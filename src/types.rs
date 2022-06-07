use crate::u64_array;
use crate::fields::{G1A, G2A, u256_to_fr};
use ark_bn254::Fr;
use crate::bytes::{BorshSerDeSized, slice_to_array};
use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use crate::macros::BorshSerDeSized;

/// Unsigned 256 bit integer ordered in LE ([32] is the first byte)
pub type U256 = [u8; 32];

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Copy, Clone, PartialEq, PartialOrd)]
pub struct U256Limbed4(pub [u64; 4]);

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Copy, Clone, PartialEq, PartialOrd, std::hash::Hash, Eq, Ord)]
pub struct U256Limbed2(pub [u128; 2]);

impl From<U256> for U256Limbed4 {
    fn from(v: U256) -> Self {
        U256Limbed4([
            u64::from_le_bytes((&v[..8]).try_into().unwrap()),
            u64::from_le_bytes((&v[8..16]).try_into().unwrap()),
            u64::from_le_bytes((&v[16..24]).try_into().unwrap()),
            u64::from_le_bytes((&v[24..]).try_into().unwrap()),
        ])
    }
}

impl From<U256> for U256Limbed2 {
    fn from(v: U256) -> Self {
        U256Limbed2([
            u128::from_le_bytes((&v[..16]).try_into().unwrap()),
            u128::from_le_bytes((&v[16..]).try_into().unwrap()),
        ])
    }
}

pub struct Lazy<'a, N: BorshSerDeSized + Clone> {
    modified: bool,
    value: Option<N>,
    data: &'a mut [u8],
}

impl<'a, N: BorshSerDeSized + Clone> Lazy<'a, N> {
    pub const SIZE: usize = N::SIZE;

    pub fn new(data: &'a mut [u8]) -> Self {
        Lazy { modified: false, value: None, data }
    }

    pub fn get(&mut self) -> N {
        match &self.value {
            Some(v) => v.clone(),
            None => {
                self.value = Some(N::try_from_slice(&self.data).unwrap());
                self.value.clone().unwrap()
            }
        }
    }

    pub fn set(&mut self, value: &N) {
        self.value = Some(value.clone());
        self.modified = true;
    }

    pub fn serialize(&mut self) {
        if !self.modified { return }
        let v = self.value.clone().unwrap().try_to_vec().unwrap();
        assert!(self.data.len() >= v.len());
        for i in 0..v.len() {
            self.data[i] = v[i];
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Copy, Clone)]
/// A Groth16 proof
pub struct Proof {
    pub a: G1A,
    pub b: G2A,
    pub c: G1A,
}

pub type RawProof = [u8; 259];
impl From<RawProof> for Proof {
    fn from(raw: RawProof) -> Proof {
        let mut buf = &raw[..];
        Proof {
            a: G1A::deserialize(&mut buf).unwrap(),
            b: G2A::deserialize(&mut buf).unwrap(),
            c: G1A::deserialize(&mut buf).unwrap(),
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq)]
pub enum ProofKind {
    Send,
    Merge,
    Migrate,
}

/// Minimum data (without public inputs) required for our n-ary join-split based proofs
#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub tree_indices: [u64; N],
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
pub struct JoinSplitPublicInputs<const N: usize> {
    pub nullifier_hashes: [U256; N],
    pub roots: [U256; N],
    pub commitment: U256,
}

pub trait PublicInputs {
    fn public_inputs_raw(&self) -> Vec<U256>;

    fn public_inputs_fr(&self) -> Vec<Fr> {
        self.public_inputs_raw().iter().map(u256_to_fr).collect()
    }
}

pub const MAX_PUBLIC_INPUTS_COUNT: usize = 7;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
/// Send public inputs: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/send_binary.circom
pub struct SendPublicInputs {
    pub join_split: JoinSplitPublicInputs<2>,
    pub recipient: U256,
    pub amount: u64,
    pub timestamp: u64,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
// https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/merge_binary.circom
pub struct MergePublicInputs {
    pub join_split: JoinSplitPublicInputs<2>,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
// https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/migrate_unary.circom
pub struct MigratePublicInputs {
    pub join_split: JoinSplitPublicInputs<1>,
    pub current_nsmt_root: U256,
    pub next_nsmt_root: U256,
}

impl PublicInputs for SendPublicInputs {
    fn public_inputs_raw(&self) -> Vec<U256> {
        let packed = pack_inputs(self.recipient, self.timestamp, self.amount).unwrap();

        vec![
            packed[0],
            packed[1],
            self.join_split.roots[0],
            self.join_split.roots[1],
            self.join_split.nullifier_hashes[0],
            self.join_split.nullifier_hashes[1],
            self.join_split.commitment,
        ]
    }
}

impl PublicInputs for MergePublicInputs {
    fn public_inputs_raw(&self) -> Vec<U256> {
        vec![
            self.join_split.roots[0],
            self.join_split.roots[1],
            self.join_split.nullifier_hashes[0],
            self.join_split.nullifier_hashes[1],
            self.join_split.commitment,
        ]
    }
}

impl PublicInputs for MigratePublicInputs {
    fn public_inputs_raw(&self) -> Vec<U256> {
        vec![
            self.join_split.roots[0],
            self.join_split.nullifier_hashes[0],
            self.join_split.commitment,
            self.current_nsmt_root,
            self.next_nsmt_root,
        ]
    }
}

/// Packing sending details (with convention: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/send.circom#L76)
/// [[0, self.timestamp, self.amount, recipient[3]], [0, recipient[0], recipient[1], recipient[2]]]
fn pack_inputs(
    recipient: U256,
    timestamp: u64,
    amount: u64
) -> Result<[U256; 2], std::io::Error> {
    let recipient = u256_to_le_limbs(recipient);

    let mut a = Vec::new();
    a.extend(vec![0; 8]);
    u64::serialize(&timestamp, &mut a)?;
    u64::serialize(&amount, &mut a)?;
    u64::serialize(&recipient[3], &mut a)?;

    let mut b = Vec::new();
    b.extend(vec![0; 8]);
    u64::serialize(&recipient[0], &mut b)?;
    u64::serialize(&recipient[1], &mut b)?;
    u64::serialize(&recipient[2], &mut b)?;

    Ok(
        [
            slice_to_array::<u8, 32>(&a[..]),
            slice_to_array::<u8, 32>(&b[..]),
        ]
    )
}

#[cfg(test)]
pub fn generate_transaction_identifier(identifier: Fr, salt: Fr) -> Fr {
    use crate::commitment::poseidon_hash::full_poseidon2_hash;
    full_poseidon2_hash(identifier, salt)
}

pub fn u256_to_le_limbs(v: U256) -> [u64; 4] {
    [
        u64::from_le_bytes(u64_array!(v, 0)),
        u64::from_le_bytes(u64_array!(v, 8)),
        u64::from_le_bytes(u64_array!(v, 16)),
        u64::from_le_bytes(u64_array!(v, 24)),
    ]
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};

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

    const ZERO: U256 = [0; 32];

    #[test]
    fn test_max_public_inputs_count() {
        assert!(
            SendPublicInputs {
                join_split: JoinSplitPublicInputs { nullifier_hashes: [ ZERO, ZERO ], roots: [ ZERO, ZERO ], commitment: ZERO },
                recipient: ZERO,
                amount: 0,
                timestamp: 0,
            }.public_inputs_raw().len() <= MAX_PUBLIC_INPUTS_COUNT
        );

        assert!(
            MergePublicInputs {
                join_split: JoinSplitPublicInputs { nullifier_hashes: [ ZERO, ZERO ], roots: [ ZERO, ZERO ], commitment: ZERO },
            }.public_inputs_raw().len() <= MAX_PUBLIC_INPUTS_COUNT
        );

        assert!(
            MigratePublicInputs {
                join_split: JoinSplitPublicInputs { nullifier_hashes: [ ZERO ], roots: [ ZERO ], commitment: ZERO },
                current_nsmt_root: ZERO,
                next_nsmt_root: ZERO,
            }.public_inputs_raw().len() <= MAX_PUBLIC_INPUTS_COUNT
        );
    }
}