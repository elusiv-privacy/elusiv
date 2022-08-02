use std::marker::PhantomData;
use crate::proof::vkey::{SendQuadraVKey, VerificationKey, MigrateUnaryVKey};
use crate::u64_array;
use crate::fields::{G1A, G2A, u64_to_u256_skip_mr, u256_to_big_uint, fr_to_u256_le};
use ark_bn254::Fr;
use ark_ff::PrimeField;
use elusiv_derive::BorshSerDePlaceholder;
use solana_program::pubkey::Pubkey;
use crate::bytes::{BorshSerDeSized, max};
use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use crate::macros::BorshSerDeSized;

/// Unsigned 256 bit integer ordered in LE ([32] is the first byte)
pub type U256 = [u8; 32];

/// A U256 in non-montgomery reduction form
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
pub struct RawU256(U256);

impl RawU256 {
    pub const ZERO: Self = RawU256([0; 32]);

    pub fn new(r: U256) -> Self {
        Self(r)
    }

    /// Performs a montgomery reduction
    pub fn reduce(&self) -> U256 {
        fr_to_u256_le(&Fr::from_repr(u256_to_big_uint(&self.0)).unwrap())
    }

    /// Skips the montgomery reduction
    pub fn skip_mr(&self) -> U256 { self.0 }
    pub fn skip_mr_ref(&self) -> &U256 { &self.0 }
}

pub trait LazyField<'a>: BorshSerDeSized {
    fn new(data: &'a mut [u8]) -> Self;
    fn serialize(&mut self);
}

#[derive(BorshSerDePlaceholder, PartialEq, Debug)]
pub struct Lazy<'a, N: BorshSerDeSized + Clone> {
    modified: bool,
    value: Option<N>,
    data: &'a mut [u8],
}

impl<'a, N: BorshSerDeSized + Clone> BorshSerDeSized for Lazy<'a, N> {
    const SIZE: usize = N::SIZE;
}

impl<'a, N: BorshSerDeSized + Clone> LazyField<'a> for Lazy<'a, N> {
    fn new(data: &'a mut [u8]) -> Self {
        Self { modified: false, value: None, data }
    }

    fn serialize(&mut self) {
        if !self.modified { return }
        let v = self.value.clone().unwrap().try_to_vec().unwrap();
        assert!(self.data.len() >= v.len());
        self.data[..v.len()].copy_from_slice(&v[..]);
    }
}

impl<'a, N: BorshSerDeSized + Clone> Lazy<'a, N> {
    pub fn get(&mut self) -> N {
        match &self.value {
            Some(v) => v.clone(),
            None => {
                self.value = Some(N::try_from_slice(self.data).unwrap());
                self.value.clone().unwrap()
            }
        }
    }

    pub fn set(&mut self, value: &N) {
        self.value = Some(value.clone());
        self.modified = true;
    }

    pub fn set_serialize(&mut self, value: &N) {
        self.set(value);
        self.serialize();
    }
}

#[derive(BorshSerDePlaceholder, PartialEq, Debug)]
pub struct JITArray<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> {
    pub data: &'a mut [u8],
    phantom: PhantomData<N>,
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> BorshSerDeSized for JITArray<'a, N, CAPACITY> {
    const SIZE: usize = N::SIZE * CAPACITY;
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> LazyField<'a> for JITArray<'a, N, CAPACITY> {
    fn new(data: &'a mut [u8]) -> Self {
        Self { data, phantom: PhantomData }
    }

    fn serialize(&mut self) { panic!() }    // no call to serialize required, performed after each set
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> JITArray<'a, N, CAPACITY> {
    pub fn get(&mut self, index: usize) -> N {
        N::try_from_slice(&self.data[index * N::SIZE..(index + 1) * N::SIZE]).unwrap()
    }

    pub fn set(&mut self, index: usize, value: &N) {
        let v = value.try_to_vec().unwrap();
        for (i, v) in v.iter().enumerate() {
            self.data[index * N::SIZE + i] = *v;
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
/// A Groth16 proof in affine form
pub struct Proof {
    pub a: G1A,
    pub b: G2A,
    pub c: G1A,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
/// A Groth16 proof in affine form in binary representation
pub struct RawProof(pub [u8; G1A::SIZE + G2A::SIZE + G1A::SIZE]);

impl TryFrom<Proof> for RawProof {
    type Error = std::io::Error;

    fn try_from(proof: Proof) -> Result<Self, Self::Error> {
        Ok(RawProof(proof.try_to_vec()?.try_into().unwrap()))
    }
}

impl TryFrom<RawProof> for Proof {
    type Error = std::io::Error;

    fn try_from(value: RawProof) -> Result<Self, Self::Error> {
        let mut buf = &value.0[..];
        Proof::deserialize(&mut buf)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct JoinSplitPublicInputs {
    pub commitment_count: u8,

    pub roots: Vec<Option<RawU256>>,
    pub nullifier_hashes: Vec<RawU256>,
    pub commitment: RawU256,
    pub fee_version: u64,
    pub amount: u64,
    pub fee: u64,
    pub token_id: u16,
}

impl JoinSplitPublicInputs {
    pub fn nullifier_duplicate_pda(&self) -> (Pubkey, u8) {
        let nullifier_hashes: Vec<U256> = self.nullifier_hashes.iter().map(|n| n.skip_mr()).collect();
        let nullifier_hashes: Vec<&[u8]> = nullifier_hashes.iter().map(|n| &n[..]).collect();
        Pubkey::find_program_address(&nullifier_hashes[..], &crate::ID)
    }

    pub fn total_amount(&self) -> u64 {
        self.amount + self.fee
    }
}

const JOIN_SPLIT_MAX_N_ARITY: u8 = 4;

fn deserialze_vec<N: BorshDeserialize>(buf: &mut &[u8], len: usize) -> std::io::Result<Vec<N>> {
    let mut v = Vec::new();
    for _ in 0..len { v.push(N::deserialize(buf)?); }
    Ok(v)
}

impl BorshDeserialize for JoinSplitPublicInputs {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= Self::SIZE);

        let commitment_count = u8::deserialize(buf)?;
        assert!(commitment_count <= JOIN_SPLIT_MAX_N_ARITY);

        let roots: Vec<RawU256> = deserialze_vec(buf, commitment_count as usize)?;
        let roots = roots.iter().map(|&r| if r.0 == [0; 32] { None } else { Some(r) }).collect();
        let nullifier_hashes = deserialze_vec(buf, commitment_count as usize)?;
        let commitment = RawU256::deserialize(buf)?;
        let fee_version = u64::deserialize(buf)?;
        let amount = u64::deserialize(buf)?;
        let fee = u64::deserialize(buf)?;
        let token_id = u16::deserialize(buf)?;

        let remaining = (JOIN_SPLIT_MAX_N_ARITY - commitment_count) as usize * (32 + 32);
        *buf = &buf[remaining..];

        Ok(
            JoinSplitPublicInputs {
                commitment_count,
                roots,
                nullifier_hashes,
                commitment,
                fee_version,
                amount,
                fee,
                token_id,
            }
        )
    }
}

fn serialize_vec<N: BorshSerialize, W: std::io::Write>(v: &Vec<N>, len: usize, writer: &mut W) -> std::io::Result<()> {
    assert_eq!(v.len(), len);
    for e in v { e.serialize(writer)?; }
    Ok(())
}

impl BorshSerialize for JoinSplitPublicInputs {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        assert!(self.commitment_count <= JOIN_SPLIT_MAX_N_ARITY);
        self.commitment_count.serialize(writer)?;

        let roots: Vec<RawU256> = self.roots.iter().map(|&r| r.unwrap_or(RawU256::ZERO)).collect();
        serialize_vec(&roots, self.commitment_count as usize, writer)?;
        serialize_vec(&self.nullifier_hashes, self.commitment_count as usize, writer)?;

        self.commitment.serialize(writer)?;
        self.fee_version.serialize(writer)?;
        self.amount.serialize(writer)?;
        self.fee.serialize(writer)?;
        self.token_id.serialize(writer)?;

        let remaining = (JOIN_SPLIT_MAX_N_ARITY - self.commitment_count) as usize * (32 + 32);
        writer.write_all(&vec![0; remaining])?;

        Ok(())
    }
}

impl BorshSerDeSized for JoinSplitPublicInputs {
    const SIZE: usize = 1 + JOIN_SPLIT_MAX_N_ARITY as usize * (32 + 32) + 32 + 8 + 8 + 8 + 2;
}

pub trait PublicInputs {
    const PUBLIC_INPUTS_COUNT: usize;

    /// Verifies the public inputs based on static value constraints
    fn verify_additional_constraints(&self) -> bool;
    fn join_split_inputs(&self) -> &JoinSplitPublicInputs;

    fn set_fee(&mut self, fee: u64);

    /// Returns the actual public signals used for the proof verification
    /// - no montgomery reduction is performed
    fn public_signals(&self) -> Vec<RawU256>;

    fn public_signals_skip_mr(&self) -> Vec<U256> {
        self.public_signals().iter().map(|&p| p.skip_mr()).collect() 
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
/// https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_deca.circom
pub struct SendPublicInputs {
    pub join_split: JoinSplitPublicInputs,
    pub recipient: RawU256,
    pub current_time: u64,
    pub identifier: RawU256,
    pub salt: RawU256, // only 128 bit
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
// https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
pub struct MigratePublicInputs {
    pub join_split: JoinSplitPublicInputs,
    pub current_nsmt_root: RawU256,
    pub next_nsmt_root: RawU256,
}

pub const MAX_PUBLIC_INPUTS_COUNT: usize = max(SendPublicInputs::PUBLIC_INPUTS_COUNT, MigratePublicInputs::PUBLIC_INPUTS_COUNT);

impl PublicInputs for SendPublicInputs {
    const PUBLIC_INPUTS_COUNT: usize = SendQuadraVKey::PUBLIC_INPUTS_COUNT;
    
    fn verify_additional_constraints(&self) -> bool {
        // Maximum `commitment_count` is 4
        // https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_deca.circom
        if self.join_split.commitment_count > JOIN_SPLIT_MAX_N_ARITY { return false }

        // Minimum `commitment_count` is 1
        if self.join_split.commitment_count == 0 { return false }

        // The first root has to be != `None`
        // https://github.com/elusiv-privacy/circuits/blob/dc1785ae0bf172892930548f4e1f9f1d48df6c97/circuits/send.circom#L7
        if self.join_split.roots[0].is_none() { return false }

        true
    }

    fn join_split_inputs(&self) -> &JoinSplitPublicInputs { &self.join_split }

    // Reference: https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_deca.circom
    // Ordering: https://github.com/elusiv-privacy/circuits/blob/master/circuits/send.circom
    fn public_signals(&self) -> Vec<RawU256> {
        let mut public_signals = Vec::new();

        // nullifierHash[nArity]
        for n_hash in &self.join_split.nullifier_hashes {
            public_signals.push(*n_hash);
        }
        for _ in self.join_split.nullifier_hashes.len()..JOIN_SPLIT_MAX_N_ARITY as usize {
            public_signals.push(RawU256::ZERO);
        }

        // root[nArity]
        for root in &self.join_split.roots {
            match root {
                Some(root) => public_signals.push(*root),
                None => public_signals.push(RawU256::ZERO),
            }
        }
        for _ in self.join_split.roots.len()..JOIN_SPLIT_MAX_N_ARITY as usize {
            public_signals.push(RawU256::ZERO);
        }

        // recipient[2]
        let recipient = split_u256_into_limbs(self.recipient.0);

        public_signals.extend(vec![
            RawU256(recipient[0]),
            RawU256(recipient[1]),
            RawU256(u64_to_u256_skip_mr(self.join_split.total_amount())),
            RawU256(u64_to_u256_skip_mr(self.current_time)),
            self.identifier,
            self.salt,
            self.join_split.commitment,
            RawU256(u64_to_u256_skip_mr(self.join_split.fee_version)),
            RawU256(u64_to_u256_skip_mr(self.join_split.token_id as u64)),
        ]);

        public_signals
    }

    fn set_fee(&mut self, fee: u64) {
        self.join_split.fee = fee
    }
}

impl PublicInputs for MigratePublicInputs {
    const PUBLIC_INPUTS_COUNT: usize = MigrateUnaryVKey::PUBLIC_INPUTS_COUNT;
    
    fn verify_additional_constraints(&self) -> bool {
        // `commitment_count` is 1
        // https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
        if self.join_split.commitment_count != 1 { return false }

        // The first root has to be != `None`
        if self.join_split.roots[0].is_none() { return false }

        true
    }

    fn join_split_inputs(&self) -> &JoinSplitPublicInputs { &self.join_split }

    // Reference: https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
    // Ordering: https://github.com/elusiv-privacy/circuits/blob/master/circuits/migrate.circom
    fn public_signals(&self) -> Vec<RawU256> {
        vec![
            self.join_split.nullifier_hashes[0],
            self.join_split.roots[0].unwrap(),
            self.join_split.commitment,
            self.current_nsmt_root,
            self.next_nsmt_root,
            RawU256(u64_to_u256_skip_mr(self.join_split.fee_version)),
            RawU256(u64_to_u256_skip_mr(self.join_split.total_amount())),
        ]
    }

    fn set_fee(&mut self, fee: u64) {
        self.join_split.fee = fee
    }
}

#[cfg(feature = "instruction-abi")]
pub fn compute_fee_rec<VKey: crate::proof::vkey::VerificationKey, P: PublicInputs>(
    public_inputs: &mut P,
    program_fee: &crate::state::fee::ProgramFee,
) {
    let fee = program_fee.proof_verification_fee(
        crate::proof::prepare_public_inputs_instructions::<VKey>(
            &public_inputs.public_signals_skip_mr()
        ).len(),
        0,
        public_inputs.join_split_inputs().amount
    ) - program_fee.proof_subvention;

    if fee != public_inputs.join_split_inputs().fee {
        public_inputs.set_fee(fee);
        compute_fee_rec::<VKey, P>(public_inputs, program_fee)
    }
}

pub fn u256_to_le_limbs(v: U256) -> [u64; 4] {
    [
        u64::from_le_bytes(u64_array!(v, 0)),
        u64::from_le_bytes(u64_array!(v, 8)),
        u64::from_le_bytes(u64_array!(v, 16)),
        u64::from_le_bytes(u64_array!(v, 24)),
    ]
}

/// Can be used to split a number > p into two public inputs
pub fn split_u256_into_limbs(v: U256) -> [U256; 2] {
    let mut a = v;
    for i in 0..16 { a[i + 16] = 0; }

    let mut b = [0; 32];
    b[..16].copy_from_slice(&v[16..32]);

    [a, b]
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
    use crate::{fields::{u256_from_str_skip_mr, u256_to_fr_skip_mr}, proof::proof_from_str};

    #[test]
    fn test_raw_u256() {
        // Just as info: `.0` returns the montgomery-reduced field element, `.into_repr` the actual field element
        assert_ne!(Fr::from_str("123").unwrap().0, Fr::from_str("123").unwrap().into_repr());

        assert_eq!(
            Fr::from_str("123").unwrap(),
            u256_to_fr_skip_mr(&RawU256(u256_from_str_skip_mr("123")).reduce())
        )
    }

    #[test]
    fn test_proof_bytes() {
        let proof = Proof {
            a: G1A(G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
                true
            )),
            b: G2A(G2Affine::new(
                Fq2::new(
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                    Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                ),
                false
            )),
            c: G1A(G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("5810683806126530275877423137657928095712201856589324885003647168396414659782").unwrap(),
                true
            )),
        };

        let bytes = Proof::try_to_vec(&proof).unwrap();
        let after = Proof::try_from_slice(&bytes[..]).unwrap();

        assert_eq!(proof.a.0, after.a.0);
        assert_eq!(proof.b.0, after.b.0);
        assert_eq!(proof.c.0, after.c.0);
    }

    #[test]
    fn test_proof_raw_proof_into() {
        let proof = proof_from_str(
            (
                "10026859857882131638516328056627849627085232677511724829502598764489185541935",
                "19685960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
            (
                (
                    "857882131638516328056627849627085232677511724829502598764489185541935",
                    "685960310506634721912121951341598678325833230508240750559904196809564625591",
                ),
                (
                    "837064132573119120838379738103457054645361649757131991036638108422638197362",
                    "86803555845400161937398579081414146527572885637089779856221229551142844794",
                ),
                false,
            ),
            (
                "21186803555845400161937398579081414146527572885637089779856221229551142844794",
                "85960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
        );

        let raw_proof = RawProof::try_from(proof).unwrap();
        assert_eq!(Proof::try_from(raw_proof).unwrap(), proof);
    }

    #[test]
    fn test_join_split_public_inputs_ser_de() {
        let inputs = JoinSplitPublicInputs {
            commitment_count: 1,
            commitment: RawU256([1; 32]),
            roots: vec![
                Some(RawU256([2; 32])),
            ],
            nullifier_hashes: vec![
                RawU256([5; 32]),
            ],
            fee_version: 999,
            amount: 666,
            fee: 777,
            token_id: 0,
        };

        let serialized = inputs.try_to_vec().unwrap();
        assert_eq!(serialized.len(), JoinSplitPublicInputs::SIZE);
        assert_eq!(inputs, JoinSplitPublicInputs::try_from_slice(&serialized[..]).unwrap());
    }

    #[test]
    fn test_send_public_inputs_verify() {
        let valid_inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 2,
                roots: vec![
                    Some(RawU256([0; 32])),
                    None,
                ],
                nullifier_hashes: vec![
                    RawU256([0; 32]),
                    RawU256([0; 32]),
                ],
                commitment: RawU256([0; 32]),
                fee_version: 0,
                amount: 0,
                fee: 0,
                token_id: 0,
            },
            recipient: RawU256([0; 32]),
            current_time: 0,
            identifier: RawU256([0; 32]),
            salt: RawU256([0; 32]),
        };
        assert!(valid_inputs.verify_additional_constraints());

        // Maximum `commitment_count` is 10
        let mut inputs = valid_inputs.clone();
        inputs.join_split.commitment_count = 11;
        assert!(!inputs.verify_additional_constraints());

        // Minimum `commitment_count` is 1
        inputs.join_split.commitment_count = 0;
        assert!(!inputs.verify_additional_constraints());

        // The first root has to be != `None`
        let mut inputs = valid_inputs;
        inputs.join_split.roots[0] = None;
        assert!(!inputs.verify_additional_constraints());
    }

    #[test]
    fn test_send_public_inputs_public_signals() {
        let inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(RawU256(u256_from_str_skip_mr("6191230350958560078367981107768184097462838361805930166881673322342311903752"))),
                ],
                nullifier_hashes: vec![
                    RawU256(u256_from_str_skip_mr("7889586699914970744657798935358222218486353295005298675075639741334684257960")),
                ],
                commitment: RawU256(u256_from_str_skip_mr("12986953721358354389598211912988135563583503708016608019642730042605916285029")),
                fee_version: 0,
                amount: 50000,
                fee: 1,
                token_id: 3,
            },
            recipient: RawU256(u256_from_str_skip_mr("212334656798193948954971085461110323640890639608634923090101683")),
            current_time: 1657927306,
            identifier: RawU256(u256_from_str_skip_mr("1")),
            salt: RawU256(u256_from_str_skip_mr("2")),
        };

        let expected = [
            "7889586699914970744657798935358222218486353295005298675075639741334684257960",
            "0",
            "0",
            "0",
            "6191230350958560078367981107768184097462838361805930166881673322342311903752",
            "0",
            "0",
            "0",
            "306186522190603117929438292402982536627",
            "623995473875165532486851",
            "50001",
            "1657927306",
            "1",
            "2",
            "12986953721358354389598211912988135563583503708016608019642730042605916285029",
            "0",
            "3",
        ].iter().map(|&p| RawU256(u256_from_str_skip_mr(p))).collect::<Vec<RawU256>>();

        assert_eq!(expected, inputs.public_signals());
        assert_eq!(expected.len(), SendPublicInputs::PUBLIC_INPUTS_COUNT);
    }

    #[test]
    fn test_migrate_public_inputs_verify() {
        let valid_inputs = MigratePublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![Some(RawU256([0; 32]))],
                nullifier_hashes: vec![RawU256([0; 32])],
                commitment: RawU256([0; 32]),
                fee_version: 0,
                amount: 0,
                fee: 0,
                token_id: 0,
            },
            current_nsmt_root: RawU256([0; 32]),
            next_nsmt_root: RawU256([0; 32]),
        };
        assert!(valid_inputs.verify_additional_constraints());

        // `commitment_count` is 1
        let mut inputs = valid_inputs.clone();
        inputs.join_split.commitment_count = 2;
        assert!(!inputs.verify_additional_constraints());

        // The first root has to be != `None`
        let mut inputs = valid_inputs;
        inputs.join_split.roots[0] = None;
        assert!(!inputs.verify_additional_constraints());
    }

    #[test]
    fn test_migrate_public_inputs_public_signals() {
        let inputs = MigratePublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(RawU256(u256_from_str_skip_mr("6191230350958560078367981107768184097462838361805930166881673322342311903752"))),
                ],
                nullifier_hashes: vec![
                    RawU256(u256_from_str_skip_mr("7889586699914970744657798935358222218486353295005298675075639741334684257960")),
                ],
                commitment: RawU256(u256_from_str_skip_mr("12986953721358354389598211912988135563583503708016608019642730042605916285029")),
                fee_version: 0,
                amount: 50000,
                fee: 1,
                token_id: 2,
            },
            current_nsmt_root: RawU256(u256_from_str_skip_mr("21233465679819394895497108546111032364089063960863923090101683")),
            next_nsmt_root: RawU256(u256_from_str_skip_mr("409746283836180593012730668816372135835438959821191292730")),
        };

        let expected = [
            "7889586699914970744657798935358222218486353295005298675075639741334684257960",
            "6191230350958560078367981107768184097462838361805930166881673322342311903752",
            "12986953721358354389598211912988135563583503708016608019642730042605916285029",
            "21233465679819394895497108546111032364089063960863923090101683",
            "409746283836180593012730668816372135835438959821191292730",
            "0",
            "50001",
        ].iter().map(|&p| RawU256(u256_from_str_skip_mr(p))).collect::<Vec<RawU256>>();

        assert_eq!(expected, inputs.public_signals());
        assert_eq!(expected.len(), MigratePublicInputs::PUBLIC_INPUTS_COUNT);
    }

    #[test]
    fn test_split_u256() {
        assert_eq!(
            split_u256_into_limbs(u256_from_str_skip_mr("1157920892373337907853269984665640564039457584007913129639935")),
            [
                u256_from_str_skip_mr("125150045379035551642519419267248553983"),
                u256_from_str_skip_mr("3402823669209901715842"),
            ]
        );

        assert_eq!(
            split_u256_into_limbs(u256_from_str_skip_mr("212334656798193948954971085461110323640890639608634923090101683")),
            [
                u256_from_str_skip_mr("306186522190603117929438292402982536627"),
                u256_from_str_skip_mr("623995473875165532486851"),
            ]
        );
    }

    type TestJITArray<'a> = JITArray<'a, u64, 100>;

    #[test]
    #[should_panic]
    fn test_jit_array_ser() {
        let v = vec![0; TestJITArray::SIZE];
        _ = TestJITArray::deserialize(&mut &v[..]);
    }

    #[test]
    #[should_panic]
    fn test_jit_array_de() {
        let mut v = vec![0; TestJITArray::SIZE];
        let a = TestJITArray::new(&mut v);
        _ = a.try_to_vec();
    }

    #[test]
    fn test_jit_array() {
        let mut v = vec![0; TestJITArray::SIZE];
        let mut a = TestJITArray::new(&mut v);
        for i in 0..100 {
            a.set(i, &(i as u64));
        }

        for i in 0..100 {
            assert_eq!(a.get(i), i as u64);
        }

        for i in 0..100 {
            let v = u64::try_from_slice(&v[i * u64::SIZE..(i + 1) * u64::SIZE]).unwrap();
            assert_eq!(v, i as u64);
        }
    }
}