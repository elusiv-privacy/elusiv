use {
    elusiv::groth16::*,
    ark_bn254::*,
    ark_groth16::PreparedVerifyingKey,
    ark_groth16::VerifyingKey,
    super::utils::*,
};

pub struct ProofString {
    pub ax: &'static str,
    pub ay: &'static str,
    pub az: &'static str,

    pub bx0: &'static str,
    pub bx1: &'static str,
    pub by0: &'static str,
    pub by1: &'static str,
    pub bz0: &'static str,
    pub bz1: &'static str,

    pub cx: &'static str,
    pub cy: &'static str,
    pub cz: &'static str,
}

impl ProofString {
    pub fn generate_proof(&self) -> Proof {
        Proof { a: self.a(), b: self.b(), c: self.c() }
    }

    pub fn generate_test_proof(&self) -> ark_groth16::Proof<Bn254> {
        ark_groth16::Proof { a: self.a(), b: self.b(), c: self.c() }
    }

    pub fn a(&self) -> G1Affine {
        G1Affine::from(G1Projective::new(str_to_bigint(self.ax).into(), str_to_bigint(self.ay).into(), str_to_bigint(self.az).into()))
    }

    pub fn b(&self) -> G2Affine {
        G2Affine::from(
            G2Projective::new(
                Fq2::new(str_to_bigint(self.bx0).into(), str_to_bigint(self.bx1).into()),
                Fq2::new(str_to_bigint(self.by0).into(), str_to_bigint(self.by1).into()),
                Fq2::new(str_to_bigint(self.bz0).into(), str_to_bigint(self.bz1).into()),
            )
        )
    }

    pub fn c(&self) -> G1Affine {
        G1Affine::from(G1Projective::new(str_to_bigint(self.cx).into(), str_to_bigint(self.cy).into(), str_to_bigint(self.cz).into()))
    }

    pub fn push_to_vec(&self, v: &mut Vec<u8>) {
        v.extend(str_to_bytes(self.ax));
        v.extend(str_to_bytes(self.ay));
        v.push(if self.az == "0" { 0 } else { 1 });

        v.extend(str_to_bytes(self.bx0));
        v.extend(str_to_bytes(self.bx1));
        v.extend(str_to_bytes(self.by0));
        v.extend(str_to_bytes(self.by1));
        v.push(if self.bz0 == "0" { 0 } else { 1 });
        v.push(if self.bz1 == "0" { 0 } else { 1 });

        v.extend(str_to_bytes(self.cx));
        v.extend(str_to_bytes(self.cy));
        v.push(if self.cz == "0" { 0 } else { 1 });
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