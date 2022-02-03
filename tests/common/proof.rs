use elusiv::groth16::*;
use ark_bn254::*;
use ark_groth16::PreparedVerifyingKey;
use ark_groth16::VerifyingKey;
use std::str::FromStr;
use ark_ff::bytes::ToBytes;

pub struct ProofString {
    pub a_x: &'static str,
    pub a_y: &'static str,
    pub a_infinity: bool,

    pub b_x_0: &'static str,
    pub b_x_1: &'static str,
    pub b_y_0: &'static str,
    pub b_y_1: &'static str,
    pub b_infinity: bool,

    pub c_x: &'static str,
    pub c_y: &'static str,
    pub c_infinity: bool,
}

impl ProofString {
    pub fn generate_proof(&self) -> Proof {
        Proof { a: self.a(), b: self.b(), c: self.c() }
    }

    pub fn generate_test_proof(&self) -> ark_groth16::Proof<Bn254> {
        ark_groth16::Proof { a: self.a(), b: self.b(), c: self.c() }
    }

    pub fn a(&self) -> G1Affine {
        G1Affine::new(
            Fq::from_str(self.a_x).unwrap(),
            Fq::from_str(self.a_y).unwrap(),
            self.a_infinity
        )
    }

    pub fn b(&self) -> G2Affine {
        G2Affine::new(
            Fq2::new(
                Fq::from_str(self.b_x_0).unwrap(),
                Fq::from_str(self.b_x_1).unwrap(),
            ),
            Fq2::new(
                Fq::from_str(self.b_y_0).unwrap(),
                Fq::from_str(self.b_y_1).unwrap(),
            ),
            self.b_infinity
        )
    }

    pub fn c(&self) -> G1Affine {
        G1Affine::new(
            Fq::from_str(self.c_x).unwrap(),
            Fq::from_str(self.c_y).unwrap(),
            self.c_infinity
        )
    }

    pub fn a_bytes(&self) -> Vec<u8> {
        let mut writer = str_to_bytes(self.a_x);
        writer.extend(str_to_bytes(self.a_y));
        writer.push(if self.a_infinity { 1 } else { 0 });
        writer
    }

    pub fn b_bytes(&self) -> Vec<u8> {
        let mut writer = str_to_bytes(self.b_x_0);
        writer.extend(str_to_bytes(self.b_x_1));
        writer.extend(str_to_bytes(self.b_y_0));
        writer.extend(str_to_bytes(self.b_y_1));
        writer.push(if self.b_infinity { 1 } else { 0 });
        writer
    }

    pub fn c_bytes(&self) -> Vec<u8> {
        let mut writer = str_to_bytes(self.c_x);
        writer.extend(str_to_bytes(self.c_y));
        writer.push(if self.c_infinity { 1 } else { 0 });
        writer
    }

    pub fn push_to_vec(&self, v: &mut Vec<u8>) {
        v.extend(self.a_bytes());
        v.extend(self.b_bytes());
        v.extend(self.c_bytes());
    }
}

pub fn ark_pvk() -> PreparedVerifyingKey<ark_bn254::Bn254> {
    let vk: VerifyingKey<ark_bn254::Bn254> = VerifyingKey {
        alpha_g1: alpha_g1(),
        beta_g2: beta_g2(),
        gamma_g2: gamma_g2(),
        delta_g2: delta_g2(),
        gamma_abc_g1: gamma_abc_g1(),
    };
    let pvk = PreparedVerifyingKey {
        vk,
        alpha_g1_beta_g2: alpha_g1_beta_g2(),
        gamma_g2_neg_pc: gamma_g2_neg_pc(),
        delta_g2_neg_pc: delta_g2_neg_pc(),
    };
    pvk
}

fn str_to_bytes(str: &str) -> Vec<u8> {
    let s = Fq::from_str(&str).unwrap();
    let mut writer: Vec<u8> = vec![];
    s.0.write(&mut writer).unwrap();
    writer
}

pub fn test_proof() -> ProofString {
    ProofString {
        a_x: "10026859857882131638516328056627849627085232677511724829502598764489185541935",
        a_y: "19685960310506634721912121951341598678325833230508240750559904196809564625591",
        a_infinity: false,

        b_x_0: "20925091368075991963132407952916453596237117852799702412141988931506241672722",
        b_x_1: "6039012589018526855429190661364232506642511499289558287989232491174672020857",
        b_y_0: "18684276579894497974780190092329868933855710870485375969907530111657029892231",
        b_y_1: "5932690455294482368858352783906317764044134926538780366070347507990829997699",
        b_infinity: false,

        c_x: "10026859857882131638516328056627849627085232677511724829502598764489185541935",
        c_y: "5810683806126530275877423137657928095712201856589324885003647168396414659782",
        c_infinity: false,
    }
}

pub fn test_inputs() -> [&'static str; 2] {
    [
        "20643720223837027367320733428836459266646763523911772324593310161284187566894",
        "19526707366532583397322534596786476145393586591811230548888354920504818678603"
    ]
}

#[cfg(test)]
mod test {
    use super::*;
    use elusiv::groth16;
    use elusiv::scalar::{ read_g1_affine, read_g2_affine };

    #[test]
    fn test_byte_conversion() {
        let mut bytes = Vec::new();
        let test = test_proof();
        test.push_to_vec(&mut bytes);
        let proof = groth16::Proof::from_bytes(&bytes).unwrap();

        assert_eq!(read_g1_affine(&test.a_bytes()), proof.a);
        assert_eq!(read_g2_affine(&test.b_bytes()), proof.b);
        assert_eq!(read_g1_affine(&test.c_bytes()), proof.c);

        assert_eq!(test.a(), proof.a);
        assert_eq!(test.b(), proof.b);
        assert_eq!(test.c(), proof.c);
    }
}