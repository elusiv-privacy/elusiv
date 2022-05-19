use crate::bytes::SerDe;
use crate::u64_array;
use crate::macros::*;
use crate::fields::{G1A, G2A, u256_to_fr};
use ark_bn254::Fr;

/// Unsigned 256 bit integer ordered in LE ([32] is the first byte)
pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub fn u256_to_le_limbs(v: U256) -> [u64; 4] {
    [
        u64::from_le_bytes(u64_array!(v, 0)),
        u64::from_le_bytes(u64_array!(v, 8)),
        u64::from_le_bytes(u64_array!(v, 16)),
        u64::from_le_bytes(u64_array!(v, 24)),
    ]
}

pub type RawProof = [u8; Proof::SIZE];

impl From<RawProof> for Proof {
    fn from(raw: RawProof) -> Proof {
        Proof {
            a: G1A::deserialize(&raw),
            b: G2A::deserialize(&raw[G1A::SIZE..]),
            c: G1A::deserialize(&raw[G1A::SIZE + G2A::SIZE..]),
        }
    }
}

#[derive(SerDe, Copy, Clone)]
/// A Groth16 proof
pub struct Proof {
    pub a: G1A,
    pub b: G2A,
    pub c: G1A,
}

/// Minimum data (without public inputs) required for our n-ary join-split based proofs
#[derive(SerDe, PartialEq)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub tree_indices: [u64; N],
}

#[derive(SerDe, PartialEq)]
pub struct JoinSplitPublicInputs<const N: usize> {
    pub nullifier_hashes: [U256; N],
    pub roots: [U256; N],
    pub commitment: U256,
}

pub trait PublicInputs {
    fn public_inputs(&self) -> Vec<Fr>;
}

pub const MAX_PUBLIC_INPUTS_COUNT: usize = 7;

#[derive(SerDe, PartialEq)]
/// Send public inputs: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/send_binary.circom
pub struct SendPublicInputs {
    pub join_split: JoinSplitPublicInputs<2>,
    pub recipient: U256,
    pub amount: u64,
    pub timestamp: u64,
}

#[derive(SerDe, PartialEq)]
// https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/merge_binary.circom
pub struct MergePublicInputs {
    pub join_split: JoinSplitPublicInputs<2>,
}

#[derive(SerDe, PartialEq)]
// https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/migrate_unary.circom
pub struct MigratePublicInputs {
    pub join_split: JoinSplitPublicInputs<1>,
    pub current_nsmt_root: U256,
    pub next_nsmt_root: U256,
}

impl PublicInputs for SendPublicInputs {
    fn public_inputs(&self) -> Vec<Fr> {
        let packed = pack_inputs(self.recipient, self.timestamp, self.amount);

        vec![
            packed[0],
            packed[1],
            self.join_split.roots[0],
            self.join_split.roots[1],
            self.join_split.nullifier_hashes[0],
            self.join_split.nullifier_hashes[1],
            self.join_split.commitment,
        ].iter().map(u256_to_fr).collect()
    }
}

impl PublicInputs for MergePublicInputs {
    fn public_inputs(&self) -> Vec<Fr> {
        vec![
            self.join_split.roots[0],
            self.join_split.roots[1],
            self.join_split.nullifier_hashes[0],
            self.join_split.nullifier_hashes[1],
            self.join_split.commitment,
        ].iter().map(u256_to_fr).collect()
    }
}

impl PublicInputs for MigratePublicInputs {
    fn public_inputs(&self) -> Vec<Fr> {
        vec![
            self.join_split.roots[0],
            self.join_split.nullifier_hashes[0],
            self.join_split.commitment,
            self.current_nsmt_root,
            self.next_nsmt_root,
        ].iter().map(u256_to_fr).collect()
    }
}

/// Packing sending details (with convention: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/send.circom#L76)
/// [[0, self.timestamp, self.amount, recipient[3]], [0, recipient[0], recipient[1], recipient[2]]]
fn pack_inputs(recipient: U256, timestamp: u64, amount: u64) -> [U256; 2] {
    let recipient = u256_to_le_limbs(recipient);

    let mut a = [0u8; 32];
    u64::serialize(timestamp, &mut a[8..]);
    u64::serialize(amount, &mut a[16..]);
    u64::serialize(recipient[3], &mut a[24..]);

    let mut b = [0u8; 32];
    u64::serialize(recipient[0], &mut a[8..]);
    u64::serialize(recipient[1], &mut a[16..]);
    u64::serialize(recipient[2], &mut a[24..]);

    [a, b]
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

        let bytes = Proof::serialize_vec(proof);
        let after = Proof::deserialize(&bytes);

        assert_eq!(proof.a.0, after.a.0);
        assert_eq!(proof.b.0, after.b.0);
        assert_eq!(proof.c.0, after.c.0);
    }

    #[test]
    fn test_max_public_inputs_count() {
        panic!()
    }
}