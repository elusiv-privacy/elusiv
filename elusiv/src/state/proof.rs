use crate::bytes::{
    usize_as_u32_safe, BorshSerDeSized, BorshSerDeSizedEnum, ElusivOption, SizedType,
};
use crate::fields::{G2HomProjective, Wrap, G1A, G2A};
use crate::processor::{ProofRequest, MAX_MT_COUNT};
use crate::proof::verifier::VerificationStep;
use crate::state::program_account::PDAAccountData;
use crate::token::Lamports;
use crate::types::{Lazy, LazyField, RawU256, U256};
use ark_bn254::{Fq, Fq12, Fq2, Fq6};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::RAM;
use elusiv_derive::{BorshSerDeSized, EnumVariantIndex};
use elusiv_proc_macros::elusiv_account;
use solana_program::entrypoint::ProgramResult;
use solana_program::pubkey::Pubkey;

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 3>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 7>;
pub type RAMG2A<'a> = LazyRAM<'a, G2A, 1>;

const MAX_PUBLIC_INPUTS_COUNT: usize = 14;
const MAX_PREPARE_INPUTS_INSTRUCTIONS: usize = MAX_PUBLIC_INPUTS_COUNT * 10;

/// Describes the state of the proof-verification initialization and finalization
#[derive(
    BorshDeserialize, BorshSerialize, BorshSerDeSized, EnumVariantIndex, Debug, Clone, PartialEq, Eq,
)]
pub enum VerificationState {
    // Init
    None,
    FeeTransferred,
    ProofSetup,

    // Finalization
    InsertNullifiers,
    Finalized,
    Closed,
}

/// Account used for verifying proofs over the span of multiple transactions
///
/// # Note
///
/// Exists only temporarily for verifying a single proof and is closed afterwards.
#[elusiv_account(partial_computation: true, eager_type: true)]
pub struct VerificationAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub(crate) instruction: u32,
    pub(crate) round: u32,

    pub prepare_inputs_instructions_count: u32,
    pub prepare_inputs_instructions: [u16; MAX_PREPARE_INPUTS_INSTRUCTIONS],

    pub vkey_id: u32,
    pub step: VerificationStep,
    pub state: VerificationState,

    // Public inputs
    pub public_input: [RawU256; MAX_PUBLIC_INPUTS_COUNT],

    // Proof
    #[lazy]
    pub a: Lazy<'a, G1A>,
    #[lazy]
    pub b: Lazy<'a, G2A>,
    #[lazy]
    pub c: Lazy<'a, G1A>,

    // Computation values
    #[lazy]
    pub(crate) prepared_inputs: Lazy<'a, G1A>,
    #[lazy]
    pub(crate) r: Lazy<'a, G2HomProjective>,
    #[lazy]
    pub(crate) f: Lazy<'a, Wrap<Fq12>>,
    #[lazy]
    pub(crate) alt_b: Lazy<'a, G2A>,
    pub(crate) coeff_index: u8,

    // RAMs for storing computation values
    #[lazy]
    pub(crate) ram_fq: RAMFq<'a>,
    #[lazy]
    pub(crate) ram_fq2: RAMFq2<'a>,
    #[lazy]
    pub(crate) ram_fq6: RAMFq6<'a>,
    #[lazy]
    pub(crate) ram_fq12: RAMFq12<'a>,

    // If true, the proof request can be finalized
    pub is_verified: ElusivOption<bool>,

    pub other_data: VerificationAccountData,
    #[no_getter]
    pub request: ProofRequest,
    pub tree_indices: [u32; MAX_MT_COUNT],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Default)]
#[cfg_attr(feature = "elusiv-client", derive(Debug))]
pub struct VerificationAccountData {
    pub fee_payer: RawU256,
    pub fee_payer_account: RawU256,
    pub recipient_wallet: ElusivOption<RawU256>,

    /// Flag that can be used to skip the renting of a nullifier_pda (if it already exists)
    pub skip_nullifier_pda: bool,

    pub min_batching_rate: u32,

    pub token_id: u16,

    /// The subvention in `token_id`-Token
    pub subvention: u64,

    /// The network-fee in `token_id`-Token
    pub network_fee: u64,

    /// The commitment-hash-fee in `Lamports`
    pub commitment_hash_fee: Lamports,

    /// The commitment-hash-fee in `token_id`-Token
    pub commitment_hash_fee_token: u64,

    /// The proof-verification-fee in `token_id`-Token
    pub proof_verification_fee: u64,

    /// The expected associated-token-account-rent in `token_id`-Token
    pub associated_token_account_rent: u64,
}

impl<'a> VerificationAccount<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn setup(
        &mut self,
        signer: RawU256,
        skip_nullifier_pda: bool,
        public_inputs: &[RawU256],
        instructions: &Vec<u32>,
        vkey_id: u32,
        request: ProofRequest,
        tree_indices: [u32; MAX_MT_COUNT],
    ) -> ProgramResult {
        self.set_vkey_id(&vkey_id);
        self.set_request(&request);
        for (i, tree_index) in tree_indices.iter().enumerate() {
            self.set_tree_indices(i, tree_index);
        }

        for (i, &public_input) in public_inputs.iter().enumerate() {
            let offset = i * 32;
            self.public_input[offset..(32 + offset)]
                .copy_from_slice(&public_input.skip_mr_ref()[..32]);
        }

        self.setup_public_inputs_instructions(instructions)?;

        // Remembers the authorized signer
        self.set_other_data(&VerificationAccountData {
            fee_payer: signer,
            skip_nullifier_pda,
            ..Default::default()
        });

        Ok(())
    }

    pub fn setup_public_inputs_instructions(
        &mut self,
        instructions: &Vec<u32>,
    ) -> Result<(), std::io::Error> {
        assert!(instructions.len() <= MAX_PREPARE_INPUTS_INSTRUCTIONS);

        self.set_prepare_inputs_instructions_count(&usize_as_u32_safe(instructions.len()));

        // It's guaranteed that the cast to u16 here is safe (see super::proof::vkey)
        for (i, &instruction) in instructions.iter().enumerate() {
            self.set_prepare_inputs_instructions(i, &(instruction as u16));
        }

        Ok(())
    }

    /// Only valid before public inputs have been setup
    pub fn load_raw_public_input(&self, index: usize) -> U256 {
        let offset = index * 32;
        self.public_input[offset..offset + 32].try_into().unwrap()
    }

    pub fn serialize_rams(&mut self) -> Result<(), std::io::Error> {
        self.ram_fq.serialize()?;
        self.ram_fq2.serialize()?;
        self.ram_fq6.serialize()?;
        self.ram_fq12.serialize()?;

        Ok(())
    }

    pub fn all_tree_indices(&self) -> [u32; MAX_MT_COUNT] {
        let mut m = [0; MAX_MT_COUNT];
        for (i, m) in m.iter_mut().enumerate() {
            *m = self.get_tree_indices(i);
        }
        m
    }

    pub fn get_request(&self) -> ProofRequest {
        ProofRequest::deserialize_enum_full(&mut &self.request[..]).unwrap()
    }
}

/// Stores data lazily on the heap, read requests will trigger deserialization
///
/// # Note
///
/// Heap allocation happens just-in-time.
pub struct LazyRAM<'a, N: Clone + Copy, const SIZE: usize> {
    /// Stores all serialized values.
    /// If an element has value [`None`], it has not been initialized yet.
    data: Vec<Option<N>>,
    source: &'a mut [u8],
    changes: Vec<bool>,

    /// Base-pointer for sub-function-calls.
    frame: usize,
}

impl<'a, N: Clone + Copy, const SIZE: usize> RAM<N> for LazyRAM<'a, N, SIZE>
where
    Wrap<N>: BorshSerDeSized,
{
    fn write(&mut self, value: N, index: usize) {
        self.check_vector_size(self.frame + index);
        self.data[self.frame + index] = Some(value);
        self.changes[self.frame + index] = true;
    }

    fn read(&mut self, index: usize) -> N {
        let i = self.frame + index;
        self.check_vector_size(i);

        match &self.data[i] {
            Some(v) => *v,
            None => {
                let data = &self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE];
                let v = <Wrap<N>>::try_from_slice(data).unwrap();
                self.data[i] = Some(v.0);
                self.data[i].unwrap()
            }
        }
    }

    fn set_frame(&mut self, frame: usize) {
        self.frame = frame
    }
    fn get_frame(&mut self) -> usize {
        self.frame
    }
}

impl<'a, N: Clone + Copy, const SIZE: usize> SizedType for LazyRAM<'a, N, SIZE>
where
    Wrap<N>: BorshSerDeSized,
{
    const SIZE: usize = <Wrap<N>>::SIZE * SIZE;
}

impl<'a, N: Clone + Copy, const SIZE: usize> LazyRAM<'a, N, SIZE>
where
    Wrap<N>: BorshSerDeSized,
{
    pub fn new(source: &'a mut [u8]) -> Self {
        assert!(source.len() == Self::SIZE);
        LazyRAM {
            data: vec![],
            frame: 0,
            source,
            changes: vec![],
        }
    }

    /// `check_vector_size` has to be called before every `data` access
    /// - this allows us to do jit heap allocation
    fn check_vector_size(&mut self, index: usize) {
        assert!(index < SIZE);
        if self.data.len() <= index {
            let extension = index + 1 - self.data.len();
            self.data.extend(vec![None; extension]);
            self.changes.extend(vec![false; extension]);
        }
    }

    pub fn serialize(&mut self) -> Result<(), std::io::Error> {
        for (i, &change) in self.changes.iter().enumerate() {
            if change {
                if let Some(value) = self.data[i] {
                    let mut slice =
                        &mut self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE];
                    BorshSerialize::serialize(&Wrap(value), &mut slice)?;
                }
            }
        }
        Ok(())
    }
}

#[elusiv_account]
pub struct NullifierDuplicateAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
}

impl<'a> NullifierDuplicateAccount<'a> {
    pub fn associated_pubkey(nullifier_hashes: &[&RawU256]) -> Pubkey {
        let hashes: Vec<U256> = nullifier_hashes.iter().map(|n| n.skip_mr()).collect();
        let hash = solana_program::hash::hashv(&hashes.iter().map(|b| &b[..]).collect::<Vec<_>>());
        Pubkey::new_from_array(hash.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::{u256_from_str, u256_from_str_skip_mr},
        state::{metadata::CommitmentMetadata, program_account::ProgramAccount},
        types::{
            InputCommitment, JoinSplitPublicInputs, OptionalFee, PublicInputs, SendPublicInputs,
        },
    };
    use elusiv_types::SizedAccount;

    #[test]
    fn test_setup_verification_account() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();

        let public_inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![InputCommitment {
                    root: Some(RawU256::new(u256_from_str("22"))),
                    nullifier_hash: RawU256::new(u256_from_str_skip_mr("333")),
                }],
                output_commitment: RawU256::new(u256_from_str_skip_mr("44444")),
                recent_commitment_index: 456,
                fee_version: 55555,
                amount: 666666,
                fee: 123,
                optional_fee: OptionalFee::default(),
                token_id: 0,
                metadata: CommitmentMetadata::default(),
            },
            hashed_inputs: u256_from_str_skip_mr("7777777"),
            recipient_is_associated_token_account: true,
            solana_pay_transfer: false,
        };
        let request = ProofRequest::Send(public_inputs.clone());
        let data = VerificationAccountData {
            fee_payer: RawU256::new([1; 32]),
            skip_nullifier_pda: true,
            ..Default::default()
        };

        let public_inputs = public_inputs.public_signals();
        let instructions = vec![1, 2, 3];
        let vkey_id = 255;

        verification_account
            .setup(
                data.fee_payer,
                true,
                &public_inputs,
                &instructions,
                vkey_id,
                request,
                [123, 456],
            )
            .unwrap();

        assert_eq!(verification_account.get_state(), VerificationState::None);
        assert_eq!(verification_account.get_vkey_id(), vkey_id);

        assert_eq!(
            verification_account.get_prepare_inputs_instructions_count() as usize,
            instructions.len()
        );
        for (i, instruction) in instructions.iter().enumerate() {
            assert_eq!(
                verification_account.get_prepare_inputs_instructions(i),
                *instruction as u16
            );
        }

        assert_eq!(verification_account.all_tree_indices(), [123, 456]);

        assert_eq!(verification_account.get_other_data(), data);
        for (i, public_input) in public_inputs.iter().enumerate() {
            assert_eq!(
                verification_account.get_public_input(i).skip_mr(),
                public_input.skip_mr()
            );
        }
    }

    impl BorshDeserialize for Wrap<u64> {
        fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
            Ok(Wrap(u64::deserialize(buf)?))
        }
    }
    impl BorshSerialize for Wrap<u64> {
        fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
            self.0.serialize(writer)
        }
    }
    impl BorshSerDeSized for Wrap<u64> {
        const SIZE: usize = u64::SIZE;
    }

    #[test]
    fn test_lazy_ram() {
        let mut data = vec![0; u64::SIZE * 2];
        let mut ram = LazyRAM::<'_, _, 2>::new(&mut data);

        ram.write(123456789u64, 0);
        assert_eq!(ram.read(0), 123456789);

        ram.inc_frame(1);
        ram.write(u64::MAX, 0);
        ram.dec_frame(1);

        assert_eq!(ram.read(0), 123456789);
        assert_eq!(ram.read(1), u64::MAX);

        ram.serialize().unwrap();

        assert_eq!(&data[..8], &u64::to_le_bytes(123456789)[..]);
        assert_eq!(&data[8..], &u64::to_le_bytes(u64::MAX)[..]);
    }

    #[test]
    fn test_check_vector_size() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let account = VerificationAccount::new(&mut data).unwrap();
        let mut ram = account.ram_fq12;

        assert_eq!(ram.data.len(), 0);
        assert_eq!(ram.changes.len(), 0);

        ram.check_vector_size(0);
        assert_eq!(ram.data.len(), 1);
        assert_eq!(ram.changes.len(), 1);

        ram.check_vector_size(0);
        assert_eq!(ram.data.len(), 1);
        assert_eq!(ram.changes.len(), 1);

        ram.check_vector_size(2);
        assert_eq!(ram.data.len(), 3);
        assert_eq!(ram.changes.len(), 3);
    }
}
