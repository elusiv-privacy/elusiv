use ark_bn254::{Fq2, Fq12, G1Affine, G2Affine, G1Projective};
use ark_ec::AffineCurve;
use ark_ff::Zero;
use borsh::BorshDeserialize;
use elusiv_proc_macros::elusiv_account;
use elusiv_types::{PDAAccountData, ElusivOption, BorshSerDeSized, MultiAccountAccountData};
use crate::{types::U256, fields::{Wrap, G1A, G2A}};

#[elusiv_account(eager_type: true)]
pub struct VKeyAccountManangerAccount {
    pda_data: PDAAccountData,
    pub active_vkey_count: u32,
}

/// Account used for storing a single immutable [`VerifyingKey`]
#[elusiv_account(multi_account: { sub_account_count: 1, sub_account_size: 1 }, eager_type: tru)]
pub struct VKeyAccount {
    pda_data: PDAAccountData,
    multi_account_data: MultiAccountAccountData<1>,
    
    pub hash: U256,
    pub public_inputs_count: u32,
    pub deploy_authority: ElusivOption<U256>,

    pub is_checked: bool,
    pub check_instruction: u32,
    pub check_hash: U256,
}
 
pub trait VerifyingKeyInfo {
    const VKEY_ID: u32;
    const PUBLIC_INPUTS_COUNT: u32;
    const HASH: U256;

    #[cfg(feature = "test-elusiv")]
    const TEST_DIRECTORY: &'static str;

    fn public_inputs_count() -> usize {
        Self::PUBLIC_INPUTS_COUNT as usize
    }

    #[cfg(feature = "test-elusiv")]
    fn verifying_key_source() -> Vec<u8> {
        use std::io::Read;

        let file = std::fs::File::open(format!("src/proof/test_vkeys/{}/elusiv_vkey", Self::TEST_DIRECTORY)).unwrap();
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).unwrap();
        buffer
    }

    #[cfg(test)]
    fn arkworks_vk() -> ark_groth16::VerifyingKey<ark_bn254::Bn254> {
        let json = std::fs::read_to_string(format!("src/proof/test_vkeys/{}/verification_key.json", Self::TEST_DIRECTORY)).unwrap();
        let vk: TestingVerifyingKeyFile = serde_json::from_str(&json).unwrap();

        ark_groth16::VerifyingKey {
            alpha_g1: vk.alpha.into(),
            beta_g2: vk.beta.into(),
            gamma_g2: vk.gamma.into(),
            delta_g2: vk.delta.into(),
            gamma_abc_g1: vk.ic.into_iter()
                .map(|a| { G1Affine::from(a) })
                .collect()
        }
    }

    #[cfg(test)]
    fn arkworks_pvk() -> ark_groth16::PreparedVerifyingKey<ark_bn254::Bn254> {
        let vk = Self::arkworks_vk();
        ark_groth16::prepare_verifying_key(&vk)
    }
}

macro_rules! verification_key_info {
    ($ident: ident, $id: expr, $public_inputs_count: expr, $hash: expr, $test_dir: expr) => {
        pub struct $ident;

        impl VerifyingKeyInfo for $ident {
            const VKEY_ID: u32 = $id;
            const PUBLIC_INPUTS_COUNT: u32 = $public_inputs_count;
            const HASH: U256 = $hash;

            #[cfg(feature = "test-elusiv")]
            const TEST_DIRECTORY: &'static str = $test_dir;
        }
    };
}

verification_key_info!(
    SendQuadraVKey,
    0,
    14,
    [137, 24, 71, 15, 128, 43, 188, 80, 166, 100, 193, 32, 100, 73, 167, 112, 40, 200, 93, 18, 248, 164, 169, 78, 34, 202, 238, 214, 96, 105, 174, 123],
    "send_quadra"
);
verification_key_info!(
    MigrateUnaryVKey,
    1,
    7,
    [193, 134, 16, 33, 191, 44, 3, 199, 83, 33, 27, 183, 211, 29, 165, 48, 198, 177, 177, 166, 224, 39, 4, 144, 220, 184, 78, 226, 167, 56, 223, 14],
    "migrate_unary"
);

#[cfg(test)]
verification_key_info!(
    TestVKey,
    2,
    14,
    [137, 24, 71, 15, 128, 43, 188, 80, 166, 100, 193, 32, 100, 73, 167, 112, 40, 200, 93, 18, 248, 164, 169, 78, 34, 202, 238, 214, 96, 105, 174, 123],
    "send_quadra"
);

/// A Groth16 verifying key with precomputed values
pub struct VerifyingKey<'a> {
    source: &'a [u8],
    pub public_inputs_count: usize,
    gamma_abc_size: usize,
}

impl<'a> VerifyingKey<'a> {
    /// Creates a new [`VerifyingKey`]
    /// 
    /// # `source`
    /// 
    /// ```
    /// alpha_beta: Fq12,
    /// gamma_abc_base: G1Affine,
    /// gamma_abc: [[[G1Affine; 255]; 32]; public_inputs_count],
    /// gamma_neg: [(Fq2, Fq2, Fq2); 91],
    /// delta_neg: [(Fq2, Fq2, Fq2); 91],
    /// 
    /// alpha: G1Affine,
    /// beta: G2Affine,
    /// gamma: G2Affine,
    /// delta: G2Affine,
    /// ```
    pub fn new(source: &'a [u8], public_inputs_count: usize) -> Option<Self> {
        assert_eq!(source.len(), Self::source_size(public_inputs_count));
        if source.len() != Self::source_size(public_inputs_count) {
            return None
        }

        Some(
            Self {
                source,
                public_inputs_count,
                gamma_abc_size: Self::gamma_abc_size(public_inputs_count),
            }
        )
    }

    const COEFFS_ARRAY_SIZE: usize = 91 * 3 * Wrap::<Fq2>::SIZE;

    const fn gamma_abc_size(public_inputs_count: usize) -> usize {
        public_inputs_count * 32 * 255 * G1A::SIZE
    }

    pub const fn source_size(public_inputs_count: usize) -> usize {
        Wrap::<Fq12>::SIZE + G1A::SIZE + Self::gamma_abc_size(public_inputs_count) + 2 * Self::COEFFS_ARRAY_SIZE + G1A::SIZE + 3 * G2A::SIZE
    }

    pub fn alpha_beta(&self) -> Fq12 {
        let slice = &self.source[..Wrap::<Fq12>::SIZE];
        Wrap::try_from_slice(slice).unwrap().0
    }

    pub fn gamma_abc_base(&self) -> G1Projective {
        let offset = Wrap::<Fq12>::SIZE;
        let slice = &self.source[offset..offset + G1A::SIZE];
        G1A::try_from_slice(slice).unwrap().0.into_projective()
    }

    pub fn gamma_abc(&self, public_input: usize, window_index: usize, window: u8) -> G1Affine {
        if window == 0 {
            return G1Affine::zero()
        }

        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + ((public_input * 32 + window_index) * 255 + window as usize - 1) * G1A::SIZE;
        let slice = &self.source[offset..offset + G1A::SIZE];
        G1A::try_from_slice(slice).unwrap().0
    }

    pub fn gamma_g2_neg_pc(&self, index: usize, inner_index: usize) -> Fq2 {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + (index * 3 + inner_index) * Wrap::<Fq2>::SIZE;
        let slice = &self.source[offset..offset + Wrap::<Fq2>::SIZE];
        Wrap::try_from_slice(slice).unwrap().0
    }

    pub fn delta_g2_neg_pc(&self, index: usize, inner_index: usize) -> Fq2 {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + Self::COEFFS_ARRAY_SIZE + (index * 3 + inner_index) * Wrap::<Fq2>::SIZE;
        let slice = &self.source[offset..offset + Wrap::<Fq2>::SIZE];
        Wrap::try_from_slice(slice).unwrap().0
    }

    #[cfg(feature = "elusiv-client")]
    pub fn alpha(&self) -> G1Affine {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + 2 * Self::COEFFS_ARRAY_SIZE;
        let slice = &self.source[offset..offset + G1A::SIZE];
        G1A::try_from_slice(slice).unwrap().0
    }

    #[cfg(feature = "elusiv-client")]
    pub fn beta(&self) -> G2Affine {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + 2 * Self::COEFFS_ARRAY_SIZE + G1A::SIZE;
        let slice = &self.source[offset..offset + G2A::SIZE];
        G2A::try_from_slice(slice).unwrap().0
    }

    #[cfg(feature = "elusiv-client")]
    pub fn gamma(&self) -> G2Affine {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + 2 * Self::COEFFS_ARRAY_SIZE + G1A::SIZE + G2A::SIZE;
        let slice = &self.source[offset..offset + G2A::SIZE];
        G2A::try_from_slice(slice).unwrap().0
    }

    #[cfg(feature = "elusiv-client")]
    pub fn delta(&self) -> G2Affine {
        let offset = Wrap::<Fq12>::SIZE + G1A::SIZE + self.gamma_abc_size + 2 * Self::COEFFS_ARRAY_SIZE + G1A::SIZE + 2 * G2A::SIZE;
        let slice = &self.source[offset..offset + G2A::SIZE];
        G2A::try_from_slice(slice).unwrap().0
    }
}

/// Groth16 verifying key used for testing purposes
/// Reference: https://github.com/elusiv-privacy/elusiv-verifying-key/blob/main/src/lib.rs#L13
#[cfg(feature = "test-elusiv")]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TestingVerifyingKeyFile {
    #[serde(rename = "nPublic")]
    npublic: u32,

    #[serde(rename = "vk_alpha_1")]
    alpha: G1PRep,

    #[serde(rename = "vk_beta_2")]
    beta: G2PRep,

    #[serde(rename = "vk_gamma_2")]
    gamma: G2PRep,

    #[serde(rename = "vk_delta_2")]
    delta: G2PRep,

    #[serde(rename = "IC")]
    ic: Vec<G1PRep>,
}

#[cfg(feature = "test-elusiv")]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct G1PRep([String; 3]);

#[cfg(feature = "test-elusiv")]
impl From<G1PRep> for G1Affine {
    fn from(value: G1PRep) -> Self {
        use std::str::FromStr;

        G1Projective::new(
            ark_bn254::Fq::from_str(&value.0[0]).unwrap(),
            ark_bn254::Fq::from_str(&value.0[1]).unwrap(),
            ark_bn254::Fq::from_str(&value.0[2]).unwrap(),
        ).into()
    }
}

#[cfg(feature = "test-elusiv")]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct G2PRep([[String; 2]; 3]);

#[cfg(feature = "test-elusiv")]
impl From<G2PRep> for G2Affine {
    fn from(value: G2PRep) -> Self {
        use std::str::FromStr;

        ark_bn254::G2Projective::new(
            ark_bn254::Fq2::new(
                ark_bn254::Fq::from_str(&value.0[0][0]).unwrap(),
                ark_bn254::Fq::from_str(&value.0[0][1]).unwrap(),
            ),
            ark_bn254::Fq2::new(
                ark_bn254::Fq::from_str(&value.0[1][0]).unwrap(),
                ark_bn254::Fq::from_str(&value.0[1][1]).unwrap(),
            ),
            ark_bn254::Fq2::new(
                ark_bn254::Fq::from_str(&value.0[2][0]).unwrap(),
                ark_bn254::Fq::from_str(&value.0[2][1]).unwrap(),
            ),
        ).into()
    }
}

#[cfg(test)]
mod test {
    use crate::fields::u256_to_big_uint;
    use super::*;

    fn test_vkey<VKey: VerifyingKeyInfo>() {
        let source = VKey::verifying_key_source();
        let vkey = VerifyingKey::new(&source, VKey::public_inputs_count()).unwrap();
        let pvk = VKey::arkworks_pvk();

        assert_eq!(vkey.alpha(), pvk.vk.alpha_g1);
        assert_eq!(vkey.beta(), pvk.vk.beta_g2);
        assert_eq!(vkey.gamma(), pvk.vk.gamma_g2);
        assert_eq!(vkey.delta(), pvk.vk.delta_g2);

        assert_eq!(vkey.alpha_beta(), pvk.alpha_g1_beta_g2);
        assert_eq!(vkey.gamma_abc_base(), pvk.vk.gamma_abc_g1[0]);

        for i in 0..VKey::PUBLIC_INPUTS_COUNT as usize {
            assert_eq!(vkey.gamma_abc(i, 0, 0), G1Affine::zero());

            let one = pvk.vk.gamma_abc_g1[i + 1];
            assert_eq!(vkey.gamma_abc(i, 0, 1), one);

            for j in 0..32 {
                for k in 1..=255 {
                    let mut scalar = [0u8; 32];
                    scalar[j] = k;
                    let s = u256_to_big_uint(&scalar);
                    assert_eq!(vkey.gamma_abc(i, j, k), one.mul(s));
                }
            }
        }

        for (i, coeffs) in pvk.gamma_g2_neg_pc.ell_coeffs.iter().enumerate() {
            assert_eq!(vkey.gamma_g2_neg_pc(i, 0), coeffs.0);
            assert_eq!(vkey.gamma_g2_neg_pc(i, 1), coeffs.1);
            assert_eq!(vkey.gamma_g2_neg_pc(i, 2), coeffs.2);
        }

        for (i, coeffs) in pvk.delta_g2_neg_pc.ell_coeffs.iter().enumerate() {
            assert_eq!(vkey.delta_g2_neg_pc(i, 0), coeffs.0);
            assert_eq!(vkey.delta_g2_neg_pc(i, 1), coeffs.1);
            assert_eq!(vkey.delta_g2_neg_pc(i, 2), coeffs.2);
        }
    }

    #[test]
    fn test_send_quadra_vkey() {
        test_vkey::<SendQuadraVKey>()
    }

    #[test]
    fn test_migrate_unary_vkey() {
        test_vkey::<MigrateUnaryVKey>()
    }
}