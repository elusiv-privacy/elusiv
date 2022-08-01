use std::marker::PhantomData;
use ark_ec::{AffineCurve, ProjectiveCurve};
use borsh::{BorshSerialize, BorshDeserialize};
use ark_bn254::{Fq, G1Affine, G1Projective};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use crate::error::ElusivError;
use crate::proof::vkey::{VerificationKey, SendQuadraVKey, MigrateUnaryVKey};
use crate::state::program_account::{SizedAccount, PDAAccountData, MultiAccountAccount, MultiAccountAccountData};
use crate::fields::{Wrap, affine_into_projective};
use crate::macros::{elusiv_account, guard};
use crate::bytes::BorshSerDeSized;

const POINT_SIZE: usize = 64;
const PRECOMPUTED_VALUES_COUNT: usize = 32 * 255;
const PRECOMPUTED_PUBLIC_INPUT_SIZE: usize = PRECOMPUTED_VALUES_COUNT * POINT_SIZE;

pub trait PrecomutedValues<VKey: VerificationKey> {
    /// Sums all precomputed scalar products onto the acc
    /// - scalars: (byte_index, byte) -> scalar 1 would be (0, 1)
    fn sum(&self, acc: &mut G1Projective, public_input: usize, scalars: &[(usize, usize)]);
}

pub const PUBLIC_INPUTS_COUNT: usize = SendQuadraVKey::PUBLIC_INPUTS_COUNT + MigrateUnaryVKey::PUBLIC_INPUTS_COUNT;

#[elusiv_account(pda_seed = b"precomputes", multi_account = (VKEY_COUNT; 0))]
pub struct PrecomputesAccount {
    pda_data: PDAAccountData,
    multi_account_data: MultiAccountAccountData<VKEY_COUNT>,

    is_setup: bool,
    instruction: u32,
    vkey: u32,
    public_input: u32,
}

macro_rules! index_to_vkey {
    ($index: ident, $vk: ident, $e: expr) => {
        match $index {
            0 => { type $vk = crate::proof::vkey::SendQuadraVKey; $e }
            1 => { type $vk = crate::proof::vkey::MigrateUnaryVKey; $e }
            _ => panic!()
        }
    };
}

pub(crate) use index_to_vkey;

trait Index { const INDEX: usize; }

macro_rules! impl_index {
    ($vkey: ty, $index: literal) => {
        impl Index for $vkey { const INDEX: usize = $index; }
    };
}

pub const VKEY_COUNT: usize = 2;
impl_index!(SendQuadraVKey, 0);
impl_index!(MigrateUnaryVKey, 1);

pub fn precompute_account_size<VKey: VerificationKey>() -> usize {
    PRECOMPUTED_PUBLIC_INPUT_SIZE * VKey::PUBLIC_INPUTS_COUNT
}

pub const fn precompute_account_size2(vkey_index: usize) -> usize {
    index_to_vkey!(vkey_index, VKey, PRECOMPUTED_PUBLIC_INPUT_SIZE * VKey::PUBLIC_INPUTS_COUNT)
}

pub const PRECOMPUTE_INSTRUCTIONS_PER_BYTE: u32 = 1 + 2 + 15 * 15;
pub const PRECOMPUTE_INSTRUCTIONS: u32 = precompute_instructions(0) + precompute_instructions(1);

pub const fn precompute_instructions(vkey_index: usize) -> u32 {
    index_to_vkey!(vkey_index, VKey, PRECOMPUTE_INSTRUCTIONS_PER_BYTE * 32 * VKey::PUBLIC_INPUTS_COUNT as u32 + 1)
}

impl<'a, 'b, 't> PrecomputesAccount<'a, 'b, 't> {
    pub fn partial_precompute(&mut self) -> ProgramResult {
        let vkey = self.get_vkey();
        index_to_vkey!(vkey, VKey, self.partial_precompute_inner::<VKey>())
    }

    pub fn partial_precompute_inner<VKey: VerificationKey>(&mut self) -> ProgramResult {
        guard!(!self.get_is_setup(), ElusivError::InvalidAccountState);

        let instruction = self.get_instruction();
        let vkey = self.get_vkey();
        let public_input = self.get_public_input() as usize;

        if instruction == 0 {   // Init public input
            self.execute_on_sub_account(vkey as usize, |data| {
                Precomputes::<VKey>::new(data).init_public_input(public_input);
            })?;
            self.set_instruction(&1);
        } else {    // 32 * PRECOMPUTE_INSTRUCTIONS_PER_BYTE instructions
            let byte_index = ((instruction - 1) / PRECOMPUTE_INSTRUCTIONS_PER_BYTE) as usize;
            let byte_instruction = (instruction - 1) % PRECOMPUTE_INSTRUCTIONS_PER_BYTE;

            self.execute_on_sub_account(vkey as usize, |data| {
                let mut p = Precomputes::<VKey>::new(data);

                if byte_instruction == 0 {  // Init
                    p.init_byte(public_input, byte_index);
                } else if byte_instruction == 1 {   // Quads
                    p.precompute_quadruples(public_input, byte_index, 1);
                } else if byte_instruction == 2 {   // Quads
                    p.precompute_quadruples(public_input, byte_index, 16);
                } else {    // Octs
                    let l = (byte_instruction as usize - 3) / 15 + 1;
                    let h = (byte_instruction as usize - 3) % 15 + 1;
                    p.precompute_octuples(public_input, byte_index, l, h);
                }
            })?;

            let mut next_instruction = instruction + 1;
            if instruction == PRECOMPUTE_INSTRUCTIONS_PER_BYTE * 32 {
                if public_input + 1 >= VKey::PUBLIC_INPUTS_COUNT {
                    if vkey + 1 >= VKEY_COUNT as u32 {
                        self.set_is_setup(&true);
                    } else {
                        self.set_vkey(&(vkey + 1));
                        self.set_public_input(&0);
                        next_instruction = 0;
                    }
                } else {
                    self.set_public_input(&(public_input as u32 + 1));
                    next_instruction = 0;
                }
            }
            self.set_instruction(&next_instruction);
        }

        Ok(())
    }
}

impl<'a, 'b, 't, VKey: VerificationKey + Index> PrecomutedValues<VKey> for PrecomputesAccount<'a, 'b, 't> {
    fn sum(&self, acc: &mut G1Projective, public_input: usize, scalars: &[(usize, usize)]) {
        *acc = self.try_execute_on_sub_account::<_, G1Projective, ProgramError>(VKey::INDEX, |data| {
            let mut x = *acc;
            let account = Precomputes::<VKey>::new(data);
            for (byte_index, byte) in scalars {
                if *byte == 0 { continue; }
                x.add_assign_mixed(&account.get_point(public_input, *byte_index, *byte))
            }
            Ok(x)
        }).unwrap();
    }
}

/// [Public input precomputing](https://github.com/elusiv-privacy/elusiv/issues/27)
/// - we choose `k = 8` (=> `32 * 255` permutations), with `n = 32` additions, which means we achieve `O(n / log n)` instead of `O(n)` complexity
pub struct Precomputes<'a, VKey: VerificationKey> {
    pub data: &'a mut [u8],
    phantom: PhantomData<VKey>,
}

impl<'a, VKey: VerificationKey> Precomputes<'a, VKey> {
    pub fn new(data: &'a mut [u8]) -> Self {
        assert_eq!(data.len(), precompute_account_size::<VKey>());
        Self { data, phantom: PhantomData }
    }

    #[cfg(feature = "precomputing")]
    fn full_precomputation(&mut self) {
        for public_input in 0..VKey::PUBLIC_INPUTS_COUNT {
            self.precompute(public_input)
        }
    }

    #[cfg(feature = "precomputing")]
    fn precompute(&mut self, public_input: usize) {
        for byte_index in 0..32 {
            self.init_public_input(public_input);
            self.init_byte(public_input, byte_index);
            self.precompute_quadruples(public_input, byte_index, 1);
            self.precompute_quadruples(public_input, byte_index, 16);
            for l in 1..=15 {
                for h in 1..=15 {
                    self.precompute_octuples(public_input, byte_index, l, h);
                }
            }
        }
    }

    fn init_public_input(&mut self, public_input: usize) {
        self.set_point(public_input, 0, 1, VKey::gamma_abc_g1(public_input + 1));
    }

    fn init_byte(&mut self, public_input: usize, byte_index: usize) {
        self.precompute_singles(public_input, byte_index);
        self.precompute_tuples(public_input, byte_index);
    }

    /// Precomputes 8 identities
    fn precompute_singles(&mut self, public_input: usize, byte_index: usize) {
        let mut ls = self.get_point(public_input, byte_index, 1).into_projective();
        let h = if byte_index < 31 { 8 } else { 6 };
        for i in 1..=h {
            ls.double_in_place();
            self.set_point(public_input, byte_index, 1 << i, ls.into_affine());
        }
    }

    /// Precomputes 2 * 2 permutations (of singles)
    fn precompute_tuples(&mut self, public_input: usize, byte_index: usize) {
        for (a, b) in [(1, 2), (4, 8), (16, 32), (64, 128)] {
            let l = self.get_point(public_input, byte_index, a).into_projective();
            let r = self.get_point(public_input, byte_index, b).into_projective();
            let s = (l + r).into_affine();
            self.set_point(public_input, byte_index, a + b, s);
        }
    }

    /// Precomputes 3 * 3 permutations (of tuples) for `i in {1, 16}`
    fn precompute_quadruples(&mut self, public_input: usize, byte_index: usize, i: usize) {
        assert!(i == 1 || i == 16);

        for j in 1..=3 {
            let l = i * j;
            let a = self.get_point(public_input, byte_index, l).into_projective();
            for k in 1..=3 {
                let h = i * 4 * k;
                let b = self.get_point(public_input, byte_index, h).into_projective();
                self.set_point(public_input, byte_index, l + h, (a + b).into_affine());
            }
        }
    }

    /// Precomputes 1 permutation per `l, h in [1; 15]` (of quadruples)
    fn precompute_octuples(&mut self, public_input: usize, byte_index: usize, l: usize, h: usize) {
        assert!((1..=15).contains(&l));
        assert!((1..=15).contains(&h));
        
        let h = h * 16;
        let a = affine_into_projective(&self.get_point(public_input, byte_index, l));
        let b = affine_into_projective(&self.get_point(public_input, byte_index, h));
        self.set_point(public_input, byte_index, l + h, (a + b).into_affine());
    }

    fn get_point(&self, public_input: usize, byte_index: usize, byte: usize) -> G1Affine {
        let slice = &self.data[memory_range(public_input, byte_index, byte)];
        G1Affine::new(
            <Wrap<Fq>>::try_from_slice(&slice[..32]).unwrap().0,
            <Wrap<Fq>>::try_from_slice(&slice[32..64]).unwrap().0,
            false
        )
    }

    fn set_point(&mut self, public_input: usize, byte_index: usize, byte: usize, point: G1Affine) {
        let slice = &mut self.data[memory_range(public_input, byte_index, byte)];
        let x = Wrap(point.x).try_to_vec().unwrap();
        let y = Wrap(point.y).try_to_vec().unwrap();
        slice[..32].copy_from_slice(&x[..32]);
        slice[32..64].copy_from_slice(&y[..32]);
    }
}

#[cfg(feature = "precomputing")]
pub struct VirtualPrecomputes<'a, VKey: VerificationKey>(pub Precomputes<'a, VKey>);

#[cfg(feature = "precomputing")]
impl<'a, VKey: VerificationKey> VirtualPrecomputes<'a, VKey> {
    pub fn data() -> Vec<u8> {
        vec![0; precompute_account_size::<VKey>()]
    }

    pub fn new(data: &'a mut [u8]) -> Self {
        let mut p = Precomputes::<VKey>::new(data);
        p.full_precomputation();
        Self(p)
    }

    pub fn new_skip_precompute(data: &'a mut [u8]) -> Self {
        Self(Precomputes::<VKey>::new(data))
    }
}

#[cfg(feature = "precomputing")]
impl<'a, VKey: VerificationKey> PrecomutedValues<VKey> for VirtualPrecomputes<'a, VKey> {
    fn sum(&self, acc: &mut G1Projective, public_input: usize, scalars: &[(usize, usize)]) {
        for (byte_index, byte) in scalars {
            if *byte == 0 { continue; }
            acc.add_assign_mixed(&self.0.get_point(public_input, *byte_index, *byte))
        }
    }
}

#[inline]
fn memory_range(public_input: usize, byte_index: usize, byte: usize) -> std::ops::Range<usize> {
    assert!(byte > 0);
    let offset = public_input * PRECOMPUTED_PUBLIC_INPUT_SIZE + (byte_index * 255 + byte as usize - 1) * POINT_SIZE;
    offset..offset + POINT_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{str::FromStr, collections::HashMap};
    use ark_bn254::Fr;
    use ark_ff::{PrimeField, Zero};
    use crate::{fields::{u256_to_big_uint, big_uint_to_u256}, proof::vkey::TestVKey, macros::account, state::program_account::{SUB_ACCOUNT_ADDITIONAL_SIZE, MultiAccountProgramAccount}};

    fn test_full_precompute<VKey: VerificationKey>() {
        let mut data = vec![0; precompute_account_size::<VKey>()];
        let mut account = VirtualPrecomputes::<VKey>::new_skip_precompute(&mut data);

        for public_input in 0..VKey::PUBLIC_INPUTS_COUNT {
            let l = VKey::gamma_abc_g1(public_input + 1);
            account.0.precompute(public_input);

            for byte_index in 0..31 {
                for byte in 1..255 {
                    let mut scalar = [0u8; 32];
                    scalar[byte_index] = byte as u8;
                    let s = u256_to_big_uint(&scalar);
                    let v = l.mul(s).into_affine();

                    assert!(!v.infinity);
                    assert_eq!(account.0.get_point(public_input, byte_index, byte), v);
                }
            }
        }
    }

    #[test]
    fn test_send_quadra_precompute() {
        test_full_precompute::<SendQuadraVKey>();
        test_full_precompute::<MigrateUnaryVKey>();
    }

    #[test]
    fn test_partial_precompute() {
        let pk = solana_program::pubkey::Pubkey::new_unique();
        let mut data = vec![0; PrecomputesAccount::SIZE];
        let mut sub_accounts = HashMap::new();
        account!(acc0, pk, vec![0; precompute_account_size2(0) + SUB_ACCOUNT_ADDITIONAL_SIZE]);
        account!(acc1, pk, vec![0; precompute_account_size2(1) + SUB_ACCOUNT_ADDITIONAL_SIZE]);
        sub_accounts.insert(0, &acc0);
        sub_accounts.insert(1, &acc1);
        let mut precompute_account = PrecomputesAccount::new(&mut data, sub_accounts).unwrap();

        for _ in 0..PUBLIC_INPUTS_COUNT {
            // Init public input
            precompute_account.partial_precompute().unwrap();
    
            for _ in 0..32 {
                // Tuples
                precompute_account.partial_precompute().unwrap();
    
                // Quads
                precompute_account.partial_precompute().unwrap();
                precompute_account.partial_precompute().unwrap();
                
                // Octs
                for _ in 0..225 {
                    precompute_account.partial_precompute().unwrap();
                }
            }
        }

        let mut data = vec![0; precompute_account_size2(0)];
        assert_eq!(&acc0.data.borrow()[1..], VirtualPrecomputes::<SendQuadraVKey>::new(&mut data).0.data);

        let mut data = vec![0; precompute_account_size2(1)];
        assert_eq!(&acc1.data.borrow()[1..], VirtualPrecomputes::<MigrateUnaryVKey>::new(&mut data).0.data);

        assert!(precompute_account.get_is_setup());
    }

    #[test]
    fn test_mul() {
        let mut data = vec![0; precompute_account_size::<TestVKey>()];
        let account = VirtualPrecomputes::<TestVKey>::new(&mut data);
        let g = TestVKey::gamma_abc_g1(1);
        let s = Fr::from_str("123456789").unwrap();

        let mut acc = G1Projective::zero();
        for (byte_index, byte) in big_uint_to_u256(&s.into_repr()).iter().enumerate() {
            account.sum(&mut acc, 0, &[(byte_index, *byte as usize)]);
        }

        let expected = g.mul(s.into_repr());
        assert_eq!(acc, expected);
    }

    #[test]
    #[should_panic]
    fn test_memory_range() {
        memory_range(0, 0, 0);
    }
}