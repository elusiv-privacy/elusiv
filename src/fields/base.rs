use ark_bn254::{ Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine };
use ark_ff::{ BigInteger256, bytes::ToBytes, bytes::FromBytes };
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

pub fn serialize_fq(f: Fq, data: &mut [u8]) {
    save_fq(f, data, 0);
}

pub fn deserialize_fq(data: &[u8]) -> Fq {
    Fq::new(BigInteger256::read(data).unwrap())
}

pub fn serialize_fq2(f: Fq2, data: &mut [u8]) {
    save_fq(f.c0, data, 0);
    save_fq(f.c1, data, 32);
}

pub fn deserialize_fq2(data: &[u8]) -> Fq2 {
    Fq2::new(
        Fq::new(BigInteger256::read(&data[0..32]).unwrap()),
        Fq::new(BigInteger256::read(&data[32..64]).unwrap()),
    )
}

pub fn serialize_fq6(f: Fq6, data: &mut [u8]) {
    save_fq(f.c0.c0, data, 0);
    save_fq(f.c0.c1, data, 32);
    save_fq(f.c1.c0, data, 64);
    save_fq(f.c1.c1, data, 96);
    save_fq(f.c2.c0, data, 128);
    save_fq(f.c2.c1, data, 160);
}

pub fn deserialize_fq6(data: &[u8]) -> Fq6 {
    Fq6::new(
        Fq2::new(
            Fq::new(BigInteger256::read(&data[0..32]).unwrap()),
            Fq::new(BigInteger256::read(&data[32..64]).unwrap()),
        ),
        Fq2::new(
            Fq::new(BigInteger256::read(&data[64..96]).unwrap()),
            Fq::new(BigInteger256::read(&data[96..128]).unwrap()),
        ),
        Fq2::new(
            Fq::new(BigInteger256::read(&data[128..160]).unwrap()),
            Fq::new(BigInteger256::read(&data[160..192]).unwrap()),
        ),
    )
}

pub fn serialize_fq12(f: Fq12, data: &mut [u8]) {
    save_fq(f.c0.c0.c0, data, 0);
    save_fq(f.c0.c0.c1, data, 32);
    save_fq(f.c0.c1.c0, data, 64);
    save_fq(f.c0.c1.c1, data, 96);
    save_fq(f.c0.c2.c0, data, 128);
    save_fq(f.c0.c2.c1, data, 160);
    save_fq(f.c1.c0.c0, data, 192);
    save_fq(f.c1.c0.c1, data, 224);
    save_fq(f.c1.c1.c0, data, 256);
    save_fq(f.c1.c1.c1, data, 288);
    save_fq(f.c1.c2.c0, data, 320);
    save_fq(f.c1.c2.c1, data, 352);
}

pub fn deserialize_fq12(data: &[u8]) -> Fq12 {
    Fq12::new(
        deserialize_fq6(&data[0..192]),
        deserialize_fq6(&data[192..384]),
    )
}

#[inline(always)]
fn save_fq(v: Fq, buffer: &mut [u8], offset: usize) {
    save_limb(v.0.0[0], buffer, 0 + offset);
    save_limb(v.0.0[1], buffer, 8 + offset);
    save_limb(v.0.0[2], buffer, 16 + offset);
    save_limb(v.0.0[3], buffer, 24 + offset);
}

#[inline(never)]
fn save_limb(v: u64, buffer: &mut [u8], offset: usize) {
    let a = u64::to_le_bytes(v);
    buffer[offset + 0] = a[0];
    buffer[offset + 1] = a[1];
    buffer[offset + 2] = a[2];
    buffer[offset + 3] = a[3];
    buffer[offset + 4] = a[4];
    buffer[offset + 5] = a[5];
    buffer[offset + 6] = a[6];
    buffer[offset + 7] = a[7];
}