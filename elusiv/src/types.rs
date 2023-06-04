use crate::bytes::BorshSerDeSized;
use crate::fields::{fr_to_u256_le, u256_to_big_uint, u64_to_u256_skip_mr, G1A, G2A};
use crate::macros::BorshSerDeSized;
use crate::processor::MAX_MT_COUNT;
use crate::proof::vkey::{MigrateUnaryVKey, SendQuadraVKey, VerifyingKeyInfo};
use crate::state::metadata::CommitmentMetadata;
use crate::state::proof::NullifierDuplicateAccount;
use crate::u64_array;
use ark_bn254::Fr;
use ark_ff::PrimeField;
use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use elusiv_types::{PDAAccount, SizedType};
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use std::marker::PhantomData;

/// Unsigned 256 bit integer ordered in LE ([32] is the first byte)
pub type U256 = [u8; 32];

/// A U256 in non-montgomery reduction form
#[derive(
    BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug, Default,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
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
    pub fn skip_mr(&self) -> U256 {
        self.0
    }
    pub fn skip_mr_ref(&self) -> &U256 {
        &self.0
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Eq, Clone, Copy, Debug)]
pub struct OrdU256(pub U256);

impl PartialOrd for OrdU256 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let a = u256_to_big_uint(&self.0);
        let b = u256_to_big_uint(&other.0);
        a.partial_cmp(&b)
    }
}

impl Ord for OrdU256 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = u256_to_big_uint(&self.0);
        let b = u256_to_big_uint(&other.0);
        a.cmp(&b)
    }
}

pub trait LazyField<'a>: SizedType {
    fn new(data: &'a mut [u8]) -> Self;
    fn serialize(&mut self);
}

#[derive(PartialEq, Debug)]
pub struct Lazy<'a, N: BorshSerDeSized + Copy> {
    modified: bool,
    value: Option<N>,
    data: &'a mut [u8],
}

impl<'a, N: BorshSerDeSized + Copy> SizedType for Lazy<'a, N> {
    const SIZE: usize = N::SIZE;
}

impl<'a, N: BorshSerDeSized + Copy> LazyField<'a> for Lazy<'a, N> {
    fn new(data: &'a mut [u8]) -> Self {
        Self {
            modified: false,
            value: None,
            data,
        }
    }

    fn serialize(&mut self) {
        if !self.modified {
            return;
        }

        let mut slice = &mut self.data[..];
        BorshSerialize::serialize(&self.value.unwrap(), &mut slice).unwrap();
    }
}

impl<'a, N: BorshSerDeSized + Copy> Lazy<'a, N> {
    pub fn get(&mut self) -> N {
        match &self.value {
            Some(v) => *v,
            None => {
                self.value = Some(N::try_from_slice(self.data).unwrap());
                self.value.unwrap()
            }
        }
    }

    /// Sets and serializes the value
    pub fn set(&mut self, value: N) {
        self.value = Some(value);
        self.modified = true;
        self.serialize();
    }

    pub fn set_no_serialization(&mut self, value: N) {
        self.value = Some(value);
        self.modified = true;
    }
}

#[derive(PartialEq, Debug)]
pub struct LazyArray<'a, N: BorshSerDeSized + Copy, const SIZE: usize> {
    modified: bool,
    values: Vec<Option<N>>,
    data: &'a mut [u8],
}

impl<'a, N: BorshSerDeSized + Copy, const SIZE: usize> SizedType for LazyArray<'a, N, SIZE> {
    const SIZE: usize = N::SIZE * SIZE;
}

impl<'a, N: BorshSerDeSized + Copy, const SIZE: usize> LazyField<'a> for LazyArray<'a, N, SIZE> {
    fn new(data: &'a mut [u8]) -> Self {
        Self {
            modified: false,
            values: vec![None; SIZE],
            data,
        }
    }

    fn serialize(&mut self) {
        if !self.modified {
            return;
        }

        for i in 0..SIZE {
            if let Some(value) = self.values[i] {
                let offset = i * N::SIZE;
                let mut slice = &mut self.data[offset..offset + N::SIZE];
                BorshSerialize::serialize(&value, &mut slice).unwrap();
            }
        }
    }
}

impl<'a, N: BorshSerDeSized + Copy, const SIZE: usize> LazyArray<'a, N, SIZE> {
    pub fn get(&mut self, index: usize) -> N {
        match &self.values[index] {
            Some(v) => *v,
            None => {
                let offset = index * N::SIZE;
                let v = N::try_from_slice(&self.data[offset..offset + N::SIZE]).unwrap();
                self.values[index] = Some(v);
                v
            }
        }
    }

    /// Sets and serializes the value
    pub fn set(&mut self, index: usize, value: N) {
        self.values[index] = Some(value);
        self.modified = true;
        self.serialize();
    }

    pub fn set_no_serialization(&mut self, index: usize, value: N) {
        self.values[index] = Some(value);
        self.modified = true;
    }
}

#[derive(PartialEq, Debug)]
pub struct JITArray<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> {
    pub data: &'a mut [u8],
    phantom: PhantomData<N>,
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> SizedType
    for JITArray<'a, N, CAPACITY>
{
    const SIZE: usize = N::SIZE * CAPACITY;
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> LazyField<'a>
    for JITArray<'a, N, CAPACITY>
{
    fn new(data: &'a mut [u8]) -> Self {
        Self {
            data,
            phantom: PhantomData,
        }
    }

    fn serialize(&mut self) {
        // no call to serialize required, performed directly after each set
        panic!()
    }
}

impl<'a, N: BorshSerDeSized + Clone, const CAPACITY: usize> JITArray<'a, N, CAPACITY> {
    pub fn get(&mut self, index: usize) -> N {
        N::try_from_slice(&self.data[index * N::SIZE..(index + 1) * N::SIZE]).unwrap()
    }

    pub fn set(&mut self, index: usize, value: &N) {
        let offset = index * N::SIZE;
        let mut slice = &mut self.data[offset..offset + N::SIZE];
        BorshSerialize::serialize(value, &mut slice).unwrap();
    }
}

/// A Groth16 proof in affine form
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug))]
pub struct Proof {
    pub a: G1A,
    pub b: G2A,
    pub c: G1A,
}

/// A Groth16 proof in affine form in binary representation (this construct is required for serde-json parsing in the Warden)
#[cfg(feature = "elusiv-client")]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RawProof {
    pub a: RawG1A,
    pub b: RawG2A,
    pub c: RawG1A,
}

#[cfg(feature = "elusiv-client")]
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RawG1A {
    x: U256,
    y: U256,
    infinity: bool,
}

#[cfg(feature = "elusiv-client")]
#[derive(BorshDeserialize, BorshSerialize, PartialEq, Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RawG2A {
    x: (U256, U256),
    y: (U256, U256),
    infinity: bool,
}

#[cfg(feature = "elusiv-client")]
impl BorshSerDeSized for RawG2A {
    const SIZE: usize = G2A::SIZE;
}

#[cfg(feature = "elusiv-client")]
impl TryFrom<RawProof> for Proof {
    type Error = std::io::Error;

    fn try_from(proof: RawProof) -> Result<Self, Self::Error> {
        let a = G1A::try_from_slice(&proof.a.try_to_vec()?)?;
        let b = G2A::try_from_slice(&proof.b.try_to_vec()?)?;
        let c = G1A::try_from_slice(&proof.c.try_to_vec()?)?;

        Ok(Proof { a, b, c })
    }
}

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InputCommitment {
    pub root: Option<RawU256>,
    pub nullifier_hash: RawU256,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct OptionalFee {
    pub collector: Pubkey,
    pub amount: u64,
}

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct JoinSplitPublicInputs {
    pub input_commitments: Vec<InputCommitment>,
    pub output_commitment: RawU256,
    pub recent_commitment_index: u32,
    pub fee_version: u32,
    pub amount: u64,
    pub fee: u64,
    pub optional_fee: OptionalFee,
    pub token_id: u16,
    pub metadata: CommitmentMetadata,
}

impl JoinSplitPublicInputs {
    pub fn roots(&self) -> Vec<Option<RawU256>> {
        self.input_commitments.iter().map(|c| c.root).collect()
    }

    pub fn nullifier_hashes(&self) -> Vec<RawU256> {
        self.input_commitments
            .iter()
            .map(|c| c.nullifier_hash)
            .collect()
    }

    pub fn associated_nullifier_duplicate_pda_pubkey(&self) -> Pubkey {
        let nullifier_hashes: Vec<&RawU256> = self
            .input_commitments
            .iter()
            .map(|c| &c.nullifier_hash)
            .collect();

        NullifierDuplicateAccount::associated_pubkey(&nullifier_hashes)
    }

    pub fn nullifier_duplicate_pda(&self) -> (Pubkey, u8) {
        NullifierDuplicateAccount::find_with_pubkey(
            self.associated_nullifier_duplicate_pda_pubkey(),
            None,
        )
    }

    pub fn create_nullifier_duplicate_pda(
        &self,
        account: &AccountInfo,
    ) -> Result<Pubkey, ProgramError> {
        NullifierDuplicateAccount::create_with_pubkey(
            self.associated_nullifier_duplicate_pda_pubkey(),
            None,
            NullifierDuplicateAccount::get_bump(account),
        )
    }

    pub fn total_amount(&self) -> u64 {
        self.amount + self.fee
    }
}

pub const JOIN_SPLIT_MAX_N_ARITY: usize = 4;

impl BorshSerDeSized for JoinSplitPublicInputs {
    // only used as maximum size in this context
    const SIZE: usize = 4 // input_commitments length
        + JOIN_SPLIT_MAX_N_ARITY * 32 // all nullifier hashes
        + MAX_MT_COUNT * (32 + 1) // unique roots
        + (JOIN_SPLIT_MAX_N_ARITY - MAX_MT_COUNT) // roots identical to the first root
        + 32 // output_commitment
        + 4 // recent_commitment_index
        + 4 // fee_version
        + 8 // amount
        + 8 // fee
        + OptionalFee::SIZE
        + 2 // token_id
        + CommitmentMetadata::SIZE;
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

/// https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_quadra.circom
/// - IMPORTANT: depending on recipient.recipient_is_associated_token_account, a higher amount is required (that also includes the rent)
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SendPublicInputs {
    pub join_split: JoinSplitPublicInputs,
    pub recipient_is_associated_token_account: bool,
    pub solana_pay_transfer: bool,
    pub hashed_inputs: U256,
}

#[allow(clippy::too_many_arguments)]
pub fn generate_hashed_inputs(
    recipient: &U256,
    identifier: &U256,
    iv: &U256,
    encrypted_owner: &U256,
    transaction_reference: &U256,
    is_associated_token_account: bool,
    metadata: &CommitmentMetadata,
    optional_fee: &OptionalFee,
    memo: &Option<Vec<u8>>,
) -> U256 {
    let mut data = recipient.to_vec();
    data.extend(identifier);
    data.extend(iv);
    data.extend(encrypted_owner);
    data.extend(transaction_reference);
    data.extend([u8::from(is_associated_token_account)]);
    data.extend(metadata);
    data.extend(optional_fee.collector.to_bytes());
    data.extend(optional_fee.amount.to_le_bytes());

    if let Some(memo) = memo {
        data.extend(memo);
    }

    let mut hash = solana_program::hash::hash(&data).to_bytes();

    // mask the lower 253 bits
    hash[31] &= 0b11111;
    hash
}

/// https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct MigratePublicInputs {
    pub join_split: JoinSplitPublicInputs,
    pub current_nsmt_root: RawU256,
    pub next_nsmt_root: RawU256,
}

impl PublicInputs for SendPublicInputs {
    const PUBLIC_INPUTS_COUNT: usize = SendQuadraVKey::PUBLIC_INPUTS_COUNT as usize;

    fn verify_additional_constraints(&self) -> bool {
        // Maximum commitment-count is 4
        // https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_quadra.circom
        if self.join_split.input_commitments.len() > JOIN_SPLIT_MAX_N_ARITY {
            return false;
        }

        // Minimum commitment-count is 1
        if self.join_split.input_commitments.is_empty() {
            return false;
        }

        // The first root has to be != `None`
        // https://github.com/elusiv-privacy/circuits/blob/dc1785ae0bf172892930548f4e1f9f1d48df6c97/circuits/send.circom#L7
        if self.join_split.input_commitments[0].root.is_none() {
            return false;
        }

        true
    }

    fn join_split_inputs(&self) -> &JoinSplitPublicInputs {
        &self.join_split
    }

    /// Reference: https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/send_quadra.circom
    /// Ordering: https://github.com/elusiv-privacy/circuits/blob/master/circuits/send.circom
    fn public_signals(&self) -> Vec<RawU256> {
        let mut public_signals = Vec::with_capacity(Self::PUBLIC_INPUTS_COUNT);

        // nullifierHash[nArity]
        for input_commitment in &self.join_split.input_commitments {
            public_signals.push(input_commitment.nullifier_hash)
        }
        for _ in self.join_split.input_commitments.len()..JOIN_SPLIT_MAX_N_ARITY {
            public_signals.push(RawU256::ZERO);
        }

        // root[nArity]
        for input_commitment in &self.join_split.input_commitments {
            match input_commitment.root {
                Some(root) => public_signals.push(root),
                None => public_signals.push(RawU256::ZERO),
            }
        }
        for _ in self.join_split.input_commitments.len()..JOIN_SPLIT_MAX_N_ARITY {
            public_signals.push(RawU256::ZERO);
        }

        public_signals.extend(vec![
            RawU256(u64_to_u256_skip_mr(self.join_split.total_amount())),
            self.join_split.output_commitment,
            RawU256(u64_to_u256_skip_mr(
                self.join_split.recent_commitment_index as u64,
            )),
            RawU256(u64_to_u256_skip_mr(self.join_split.fee_version as u64)),
            RawU256(u64_to_u256_skip_mr(self.join_split.token_id as u64)),
            RawU256(self.hashed_inputs),
        ]);

        assert_eq!(public_signals.len(), Self::PUBLIC_INPUTS_COUNT);

        public_signals
    }

    fn set_fee(&mut self, fee: u64) {
        self.join_split.fee = fee
    }
}

impl PublicInputs for MigratePublicInputs {
    const PUBLIC_INPUTS_COUNT: usize = MigrateUnaryVKey::PUBLIC_INPUTS_COUNT as usize;

    fn verify_additional_constraints(&self) -> bool {
        // commitment-count is 1
        // https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
        if self.join_split.input_commitments.len() != 1 {
            return false;
        }

        // The first root has to be != `None`
        if self.join_split.input_commitments[0].root.is_none() {
            return false;
        }

        true
    }

    fn join_split_inputs(&self) -> &JoinSplitPublicInputs {
        &self.join_split
    }

    /// Reference: https://github.com/elusiv-privacy/circuits/blob/master/circuits/main/migrate_unary.circom
    /// Ordering: https://github.com/elusiv-privacy/circuits/blob/master/circuits/migrate.circom
    fn public_signals(&self) -> Vec<RawU256> {
        vec![
            self.join_split.input_commitments[0].nullifier_hash,
            self.join_split.input_commitments[0].root.unwrap(),
            self.join_split.output_commitment,
            RawU256(u64_to_u256_skip_mr(
                self.join_split.recent_commitment_index as u64,
            )),
            self.current_nsmt_root,
            self.next_nsmt_root,
            // RawU256(u64_to_u256_skip_mr(self.join_split.fee_version as u64)),
            RawU256(u64_to_u256_skip_mr(self.join_split.total_amount())),
        ]
    }

    fn set_fee(&mut self, fee: u64) {
        self.join_split.fee = fee
    }
}

#[cfg(feature = "elusiv-client")]
pub fn compute_fee_rec<V: crate::proof::vkey::VerifyingKeyInfo, P: PublicInputs>(
    public_inputs: &mut P,
    program_fee: &crate::state::fee::ProgramFee,
    price: &crate::token::TokenPrice,
) {
    let fee = program_fee
        .proof_verification_fee(
            crate::proof::verifier::prepare_public_inputs_instructions(
                &public_inputs.public_signals_skip_mr(),
                V::public_inputs_count(),
            )
            .len(),
            0,
            public_inputs.join_split_inputs().amount,
            public_inputs.join_split_inputs().token_id,
            price,
        )
        .unwrap()
        .amount();

    if fee != public_inputs.join_split_inputs().fee {
        public_inputs.set_fee(fee);
        compute_fee_rec::<V, P>(public_inputs, program_fee, price)
    }
}

#[cfg(feature = "elusiv-client")]
pub fn compute_fee_rec_lamports<V: crate::proof::vkey::VerifyingKeyInfo, P: PublicInputs>(
    public_inputs: &mut P,
    program_fee: &crate::state::fee::ProgramFee,
) {
    use crate::token::TokenPrice;
    compute_fee_rec::<V, P>(public_inputs, program_fee, &TokenPrice::new_lamports())
}

pub fn u256_to_le_limbs(v: U256) -> [u64; 4] {
    [
        u64::from_le_bytes(u64_array!(v, 0)),
        u64::from_le_bytes(u64_array!(v, 8)),
        u64::from_le_bytes(u64_array!(v, 16)),
        u64::from_le_bytes(u64_array!(v, 24)),
    ]
}

/// Can be used to split a number > scalar field modulus (like Curve25519 keys) into two public inputs
pub fn split_u256_into_limbs(v: U256) -> [U256; 2] {
    let mut a = v;
    for i in 0..16 {
        a[i + 16] = 0;
    }

    let mut b = [0; 32];
    b[..16].copy_from_slice(&v[16..32]);

    [a, b]
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        fields::{u256_from_str_skip_mr, u256_to_fr_skip_mr},
        processor::MAX_MT_COUNT,
        proof::verifier::proof_from_str,
    };
    use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
    use std::str::FromStr;

    #[test]
    fn test_raw_u256() {
        // Just as info: `.0` returns the montgomery-reduced field element, `.into_repr` the actual field element
        assert_ne!(
            Fr::from_str("123").unwrap().0,
            Fr::from_str("123").unwrap().into_repr()
        );

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

        let raw_proof = RawProof::try_from_slice(&proof.try_to_vec().unwrap()).unwrap();
        assert_eq!(Proof::try_from(raw_proof).unwrap(), proof);
    }

    #[test]
    fn test_join_split_public_inputs_ser_de() {
        let inputs = JoinSplitPublicInputs {
            input_commitments: vec![InputCommitment {
                root: Some(RawU256::new(u256_from_str_skip_mr("22"))),
                nullifier_hash: RawU256::new(u256_from_str_skip_mr("333")),
            }],
            output_commitment: RawU256::new(u256_from_str_skip_mr("44444")),
            recent_commitment_index: 123,
            fee_version: 999,
            amount: 666,
            fee: 777,
            optional_fee: OptionalFee::default(),
            token_id: 0,
            metadata: CommitmentMetadata::default(),
        };

        let serialized = inputs.try_to_vec().unwrap();
        assert_eq!(
            inputs,
            JoinSplitPublicInputs::try_from_slice(&serialized[..]).unwrap()
        );
    }

    #[test]
    fn test_send_public_inputs_verify() {
        let valid_inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![
                    InputCommitment {
                        root: Some(RawU256(u256_from_str_skip_mr("6191230350958560078367981107768184097462838361805930166881673322342311903752"))),
                        nullifier_hash: RawU256([0; 32]),
                    },
                    InputCommitment {
                        root: None,
                        nullifier_hash: RawU256([0; 32])
                    },
                ],
                output_commitment: RawU256([0; 32]),
                recent_commitment_index: 123,
                fee_version: 0,
                amount: 0,
                fee: 0,
                optional_fee: OptionalFee::default(),
                token_id: 0,
                metadata: CommitmentMetadata::default(),
            },
            hashed_inputs: [0; 32],
            recipient_is_associated_token_account: true,
            solana_pay_transfer: false,
        };
        assert!(valid_inputs.verify_additional_constraints());

        // Maximum commitment-count
        let mut inputs = valid_inputs.clone();
        for i in inputs.join_split.input_commitments.len()..JOIN_SPLIT_MAX_N_ARITY + 1 {
            inputs.join_split.input_commitments.push(InputCommitment {
                root: None,
                nullifier_hash: RawU256::new(u256_from_str_skip_mr(&i.to_string())),
            });
        }
        assert!(!inputs.verify_additional_constraints());

        // Minimum commitment-count is 1
        inputs.join_split.input_commitments.clear();
        assert!(!inputs.verify_additional_constraints());

        // The first root has to be != `None`
        let mut inputs = valid_inputs;
        inputs.join_split.input_commitments[0].root = None;
        assert!(!inputs.verify_additional_constraints());
    }

    #[test]
    fn test_send_public_inputs_public_signals() {
        let inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![
                    InputCommitment {
                        root: Some(RawU256(u256_from_str_skip_mr("6191230350958560078367981107768184097462838361805930166881673322342311903752"))),
                        nullifier_hash: RawU256::new(u256_from_str_skip_mr("7889586699914970744657798935358222218486353295005298675075639741334684257960")),
                    }
                ],
                output_commitment: RawU256::new(u256_from_str_skip_mr("12986953721358354389598211912988135563583503708016608019642730042605916285029")),
                recent_commitment_index: 123,
                fee_version: 0,
                amount: 50000,
                fee: 1,
                optional_fee: OptionalFee::default(),
                token_id: 3,
                metadata: CommitmentMetadata::default(),
            },
            hashed_inputs: u256_from_str_skip_mr("306186522190603117929438292402982536627"),
            recipient_is_associated_token_account: true,
            solana_pay_transfer: false,
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
            "50001",
            "12986953721358354389598211912988135563583503708016608019642730042605916285029",
            "123",
            "0",
            "3",
            "306186522190603117929438292402982536627",
        ]
        .iter()
        .map(|&p| RawU256(u256_from_str_skip_mr(p)))
        .collect::<Vec<RawU256>>();

        assert_eq!(expected, inputs.public_signals());
        assert_eq!(expected.len(), SendPublicInputs::PUBLIC_INPUTS_COUNT);
    }

    #[test]
    fn test_join_split_public_inputs_size() {
        let mut input_commitments = vec![
            InputCommitment {
                root: Some(RawU256([1; 32])),
                nullifier_hash: RawU256::new([1; 32]),
            };
            MAX_MT_COUNT
        ];
        for _ in 0..JOIN_SPLIT_MAX_N_ARITY - MAX_MT_COUNT {
            input_commitments.push(InputCommitment {
                root: None,
                nullifier_hash: RawU256::new([1; 32]),
            });
        }
        let join_split = JoinSplitPublicInputs {
            input_commitments,
            output_commitment: RawU256::new([1; 32]),
            recent_commitment_index: u32::MAX,
            fee_version: u32::MAX,
            amount: u64::MAX,
            fee: u64::MAX,
            optional_fee: OptionalFee {
                collector: Pubkey::new_unique(),
                amount: u64::MAX,
            },
            token_id: u16::MAX,
            metadata: [1; CommitmentMetadata::SIZE],
        };

        assert_eq!(
            JoinSplitPublicInputs::SIZE,
            join_split.try_to_vec().unwrap().len()
        );
    }

    #[test]
    fn test_send_public_inputs_serde() {
        let str = "
        {
            \"join_split\":
            {
                \"input_commitments\":[
                    {
                        \"root\": [220,109,75,166,42,21,212,57,27,45,247,16,115,107,121,228,172,110,162,119,166,173,100,50,196,104,230,12,112,119,15,30],
                        \"nullifier_hash\": [145,228,92,60,193,80,150,255,145,29,156,152,238,64,230,149,19,80,161,103,119,135,38,139,142,67,18,163,159,54,11,22]
                    }
                ],
                \"output_commitment\":[146,94,46,51,211,4,49,85,42,229,99,188,226,49,115,65,108,37,190,116,123,32,2,181,59,231,108,209,18,13,235,45],
                \"recent_commitment_index\":123,
                \"fee_version\":0,
                \"amount\":100000000,
                \"fee\":120000,
                \"optional_fee\": {
                    \"collector\": [220,109,75,166,42,21,212,57,27,45,247,16,115,107,121,228,172,110,162,119,166,173,100,50,196,104,230,12,112,119,15,30],
                    \"amount\": 99
                },
                \"token_id\":0,
                \"metadata\":[255,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]
            },
            \"hashed_inputs\":[239,6,63,227,53,18,117,85,172,69,192,148,3,201,244,219,177,39,64,179,204,41,240,146,189,20,177,226,231,33,176,0],
            \"recipient_is_associated_token_account\":true,
            \"solana_pay_transfer\":true
        }
        ";
        let mut str = String::from(str);
        str.retain(|c| !c.is_whitespace());

        let result: SendPublicInputs = serde_json::from_str(&str).unwrap();
        serde_json::to_string(&result).unwrap();
        result.try_to_vec().unwrap();
    }

    #[test]
    fn test_migrate_public_inputs_verify() {
        let valid_inputs = MigratePublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![InputCommitment {
                    root: Some(RawU256::new([0; 32])),
                    nullifier_hash: RawU256::new([0; 32]),
                }],
                output_commitment: RawU256::new([0; 32]),
                recent_commitment_index: 123,
                fee_version: 0,
                amount: 0,
                fee: 0,
                optional_fee: OptionalFee::default(),
                token_id: 0,
                metadata: CommitmentMetadata::default(),
            },
            current_nsmt_root: RawU256([0; 32]),
            next_nsmt_root: RawU256([0; 32]),
        };
        assert!(valid_inputs.verify_additional_constraints());

        // commitment-count is 1
        let mut inputs = valid_inputs.clone();
        inputs
            .join_split
            .input_commitments
            .push(inputs.join_split.input_commitments[0].clone());
        assert!(!inputs.verify_additional_constraints());

        // The first root has to be != `None`
        let mut inputs = valid_inputs;
        inputs.join_split.input_commitments[0].root = None;
        assert!(!inputs.verify_additional_constraints());
    }

    #[test]
    #[ignore]
    fn test_migrate_public_inputs_public_signals() {
        // TODO: update migrate to use commitment-index

        let inputs = MigratePublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![
                    InputCommitment {
                        root: Some(RawU256(u256_from_str_skip_mr("6191230350958560078367981107768184097462838361805930166881673322342311903752"))),
                        nullifier_hash: RawU256::new(u256_from_str_skip_mr("7889586699914970744657798935358222218486353295005298675075639741334684257960")),
                    }
                ],
                output_commitment: RawU256::new(u256_from_str_skip_mr("12986953721358354389598211912988135563583503708016608019642730042605916285029")),
                recent_commitment_index: 123,
                fee_version: 0,
                amount: 50000,
                fee: 1,
                optional_fee: OptionalFee::default(),
                token_id: 2,
                metadata: CommitmentMetadata::default(),
            },
            current_nsmt_root: RawU256(u256_from_str_skip_mr("21233465679819394895497108546111032364089063960863923090101683")),
            next_nsmt_root: RawU256(u256_from_str_skip_mr("409746283836180593012730668816372135835438959821191292730")),
        };

        let expected = [
            "7889586699914970744657798935358222218486353295005298675075639741334684257960",
            "6191230350958560078367981107768184097462838361805930166881673322342311903752",
            "12986953721358354389598211912988135563583503708016608019642730042605916285029",
            "123",
            "21233465679819394895497108546111032364089063960863923090101683",
            "409746283836180593012730668816372135835438959821191292730",
            "0",
            "50001",
        ]
        .iter()
        .map(|&p| RawU256(u256_from_str_skip_mr(p)))
        .collect::<Vec<RawU256>>();

        assert_eq!(expected, inputs.public_signals());
        assert_eq!(expected.len(), MigratePublicInputs::PUBLIC_INPUTS_COUNT);
    }

    #[test]
    fn test_split_u256() {
        assert_eq!(
            split_u256_into_limbs(u256_from_str_skip_mr(
                "1157920892373337907853269984665640564039457584007913129639935"
            )),
            [
                u256_from_str_skip_mr("125150045379035551642519419267248553983"),
                u256_from_str_skip_mr("3402823669209901715842"),
            ]
        );

        assert_eq!(
            split_u256_into_limbs(u256_from_str_skip_mr(
                "212334656798193948954971085461110323640890639608634923090101683"
            )),
            [
                u256_from_str_skip_mr("306186522190603117929438292402982536627"),
                u256_from_str_skip_mr("623995473875165532486851"),
            ]
        );
    }

    type TestJITArray<'a> = JITArray<'a, u64, 100>;

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

    #[test]
    fn test_compute_hashed_inputs() {
        let recipient = u256_from_str_skip_mr(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        );
        let identifier = u256_from_str_skip_mr(
            "2724519746760381068789905529616186067135764345073895624457825506068544226822",
        );
        let iv = u256_from_str_skip_mr("5683487854789");
        let encrypted_owner = u256_from_str_skip_mr("5789489458548458945478235642378");
        let solana_pay_id = u256_from_str_skip_mr("2739264834378438439");
        let is_associated_token_account = false;
        let metadata = u256_from_str_skip_mr("5806146055028818284759215385528282317313")
            [..CommitmentMetadata::SIZE]
            .try_into()
            .unwrap();
        let optional_fee = OptionalFee {
            collector: Pubkey::new_from_array(u256_from_str_skip_mr(
                "47245630098656629409689502135467576747915749142015942317587311262393467745251",
            )),
            amount: 1000000,
        };

        let expected = u256_from_str_skip_mr(
            "5593953132782974239527342909647286690390142208813910555015910557707363192433",
        );

        assert_eq!(
            generate_hashed_inputs(
                &recipient,
                &identifier,
                &iv,
                &encrypted_owner,
                &solana_pay_id,
                is_associated_token_account,
                &metadata,
                &optional_fee,
                &None
            ),
            expected
        );

        let memo = Some(vec![1, 6, 7, 88, 88, 8, 8, 8, 8, 84, 3]);
        let expected = u256_from_str_skip_mr(
            "7190753645577115026314391505244643580055580854837751025314898582887072501874",
        );

        assert_eq!(
            generate_hashed_inputs(
                &recipient,
                &identifier,
                &iv,
                &encrypted_owner,
                &solana_pay_id,
                is_associated_token_account,
                &metadata,
                &optional_fee,
                &memo
            ),
            expected
        );
    }
}
