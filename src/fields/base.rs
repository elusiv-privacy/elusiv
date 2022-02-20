use ark_bn254::{ Fq, Fq2, G1Affine, G2Affine, G1Projective, G2Projective };
use ark_ff::{ BigInteger256, bytes::ToBytes };
use super::scalar::*;

// Bn254 base field elements
// - `q = 21888242871839275222246405745257275088696311157297823662689037894645226208583`

pub const G1PROJECTIVE_SIZE: usize = 96;
pub const G2PROJECTIVE_SIZE: usize = 192;
pub const G1AFFINE_SIZE: usize = 65;
pub const G2AFFINE_SIZE: usize = 129;

pub fn write_g1_affine(buffer: &mut [u8], g1a: G1Affine) {
    let mut bytes: Vec<u8> = vec![];
    g1a.x.0.write(&mut bytes).unwrap();
    g1a.y.0.write(&mut bytes).unwrap();
    bytes.push(if g1a.infinity { 1 } else { 0 });

    for i in 0..G1AFFINE_SIZE {
        buffer[i] = bytes[i];
    }
}
pub fn read_g1_affine(bytes: &[u8]) -> G1Affine {
    G1Affine::new(
        read_le_montgomery(&bytes[..32]),
        read_le_montgomery(&bytes[32..64]),
        bytes[64] == 1
    )
}

pub fn write_g2_affine(buffer: &mut [u8], p: G2Affine) {
    let mut bytes = write_fq2_le_montgomery(p.x);
    bytes.extend(write_fq2_le_montgomery(p.y));
    bytes.push(if p.infinity { 1 } else { 0 });

    for i in 0..G2AFFINE_SIZE {
        buffer[i] = bytes[i];
    }
}
pub fn read_g2_affine(bytes: &[u8]) -> G2Affine {
    G2Affine::new(
        read_fq2_le_montgomery(&bytes[..64]),
        read_fq2_le_montgomery(&bytes[64..128]),
        bytes[128] == 1
    )
}

pub fn write_g1_projective(buffer: &mut [u8], p: G1Projective) {
    let mut bytes = write_le_montgomery(p.x);
    bytes.extend(write_le_montgomery(p.y));
    bytes.extend(write_le_montgomery(p.z));

    for i in 0..G1PROJECTIVE_SIZE {
        buffer[i] = bytes[i];
    }
}
pub fn read_g1_projective(bytes: &[u8]) -> G1Projective {
    G1Projective::new(
        read_le_montgomery(&bytes[..32]),
        read_le_montgomery(&bytes[32..64]),
        read_le_montgomery(&bytes[64..96]),
    )
}

pub fn write_g2_projective(buffer: &mut [u8], p: G2Projective) {
    let mut bytes = write_fq2_le_montgomery(p.x);
    bytes.extend(write_fq2_le_montgomery(p.y));
    bytes.extend(write_fq2_le_montgomery(p.z));

    for i in 0..G2PROJECTIVE_SIZE {
        buffer[i] = bytes[i];
    }
}
pub fn read_g2_projective(bytes: &[u8]) -> G2Projective {
    G2Projective::new(
        read_fq2_le_montgomery(&bytes[..64]),
        read_fq2_le_montgomery(&bytes[64..128]),
        read_fq2_le_montgomery(&bytes[128..192]),
    )
}

pub fn read_le_montgomery(bytes: &[u8]) -> Fq {
    Fq::new(BigInteger256(bytes_to_limbs(bytes)))
}

pub fn write_le_montgomery(q: Fq) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    q.0.write(&mut writer).unwrap();
    writer
}

pub fn read_fq2_le_montgomery(bytes: &[u8]) -> Fq2 {
    Fq2::new(
        read_le_montgomery(&bytes[..32]),
        read_le_montgomery(&bytes[32..64]),
    )
}

pub fn write_fq2_le_montgomery(q: Fq2) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    q.c0.0.write(&mut writer).unwrap();
    q.c1.0.write(&mut writer).unwrap();
    writer
}