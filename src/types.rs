use crate::bytes::SerDe;
use crate::u64_array;
use crate::macros::*;
use ark_bn254::{ Fr, G1Affine, G2Affine };

/// Unsigned 256 bit integer ordered in LE ([32] is the first byte)
pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

impl U256 {
    pub fn to_le_limbs(&self) -> [u64; 4] {
        [
            u64::from_le_bytes(u64_array!(self, 0)),
            u64::from_le_bytes(u64_array!(self, 8)),
            u64::from_le_bytes(u64_array!(self, 16)),
            u64::from_le_bytes(u64_array!(self, 24)),
        ]
    }
}

pub type RawProof = [u8; Proof::SIZE];

#[derive(SerDe, Copy, Clone)]
/// A Groth16 proof
pub struct Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

/// Minimum data (without public inputs) required for our n-ary join-split based proofs
#[derive(SerDe)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub tree_indices: [u64; N],
}

#[derive(SerDe)]
pub struct JoinSplitPublicInputs<const N: usize> {
    pub nullifier_hashes: [U256; N],
    pub root_hashes: [U256; N],
    pub commitment: U256,
}

#[derive(SerDe)]
pub enum PublicInputs {
    Send {
        join_split: JoinSplitPublicInputs<2>,
        recipient: U256,
        amount: u64,
        timestamp: u64,
    },
    Merge {
        join_split: JoinSplitPublicInputs<2>,
    },
    Migrate {
        join_split: JoinSplitPublicInputs<1>,
        current_nsmt_root: U256,
        next_nsmt_root: U256,
    },
}

pub const MAX_PUBLIC_INPUTS_COUNT: usize = 7;

impl PublicInputs {
    pub fn get_public_inputs(&self) -> Vec<Fr> {
        match self {
            // Send public inputs: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/send_binary.circom
            Self::Send { join_split, recipient, amount, timestamp } => {
                let s = Self::pack_sending_details(recipient, amount, timestamp);

                vec![
                    s[0],
                    s[1],
                    join_split.root_hashes[0].into(),
                    join_split.root_hashes[1].into(),
                    join_split.nullifier_hashes[0].into(),
                    join_split.nullifier_hashes[1].into(),
                    join_split.commitment.into(),
                ]
            },

            // https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/merge_binary.circom
            Self::Merge { join_split } => {
                vec![
                    join_split.root_hashes[0].into(),
                    join_split.root_hashes[1].into(),
                    join_split.nullifier_hashes[0].into(),
                    join_split.nullifier_hashes[1].into(),
                    join_split.commitment.into(),
                ]
            },

            // https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/main/migrate_unary.circom
            Self::Migrate { join_split, current_nsmt_root, next_nsmt_root } => {
                vec![
                    join_split.root_hashes[0].into(),
                    join_split.nullifier_hashes[0].into(),
                    join_split.commitment.into(),
                    current_nsmt_root.into(),
                    next_nsmt_root.into(),
                ]
            }
        }
    }

    /// Convention: https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/send.circom#L76
    fn pack_sending_details(recipient: U256, amount: u64, timestamp: u64) -> [Fr; 2] {
        let recipient = recipient.to_le_limbs();
        [
            [0, timestamp, amount, recipient[3]].into(),
            [0, recipient[0], recipient[1], recipient[2]].into()
        ]
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    use ark_bn254::{ Fq, Fq2 };

    #[test]
    fn test_proof_bytes() {
        let proof = Proof {
            a: G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
                true
            ),
            b: G2Affine::new(
                Fq2::new(
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                    Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                ),
                false
            ),
            c: G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("5810683806126530275877423137657928095712201856589324885003647168396414659782").unwrap(),
                true
            ),
        };

        let bytes = proof.to_bytes();
        let after = Proof::from_bytes(&bytes).unwrap();

        assert_eq!(proof.a, after.a);
        assert_eq!(proof.b, after.b);
        assert_eq!(proof.c, after.c);
    }

    #[test]
    fn test_max_public_inputs_count() {
        panic!()
    }
}