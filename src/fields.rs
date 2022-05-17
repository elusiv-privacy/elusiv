//! Bn254 base field modulus: `q = 21888242871839275222246405745257275088696311157297823662689037894645226208583`
//! Bn254 scalar field modulus: `r = 21888242871839275222246405745257275088548364400416034343698204186575808495617`

use ark_bn254::{ Fr, Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine, FqParameters, FrParameters };
use ark_ff::BigInteger256;
use crate::bytes::SerDe;

/// From &[u8] to [u8; 8]
macro_rules! u64_array {
    ($v: ident, $o: expr) => {
        [$v[$o + 0], $v[$o + 1], $v[$o + 2], $v[$o + 3], $v[$o + 4], $v[$o + 5], $v[$o + 6], $v[$o + 7]]
    };
}

/// Constructs a base field element from an element in montgomery form
/// - panics if the supplied element is >= the base field modulus `q`
fn safe_base_montgomery(e: BigInteger256) -> Fq {
    if e < FqParameters::MODULUS { Fq::new(e) } else { panic!() }
}

/// Constructs a scalar field element from an element in montgomery form
/// - panics if the supplied element is >= the scalar field modulus `r`
fn safe_scalar_montgomery(e: BigInteger256) -> Fr {
    if e < FrParameters::MODULUS { Fr::new(e) } else { panic!() }
}

/// BigInteger256 efficiently from LE buffer
/// - to increase efficiency callers should always assert that $v.len() >= $o + 32 (https://www.reddit.com/r/rust/comments/6anp0d/suggestion_for_a_new_rustc_optimization/dhfzp93/)
macro_rules! le_u256 {
    ($v: ident, $o: literal) => {
        BigInteger256([
            u64::from_le_bytes(u64_array!($v, 0 + $o)),
            u64::from_le_bytes(u64_array!($v, 8 + $o)),
            u64::from_le_bytes(u64_array!($v, 16 + $o)),
            u64::from_le_bytes(u64_array!($v, 24 + $o)),
        ])
    };
}

/// Deserializes 32 bytes into a base field element
/// - panics if the serialized value is larger than the field modulus
macro_rules! fq_montgomery {
    ($v: ident, $o: literal) => { safe_base_montgomery(le_u256!($v, $o)) };
}

/// Deserializes 32 bytes into a scalar field element
/// - panics if the serialized value is larger than the field modulus
macro_rules! fr_montgomery {
    ($v: ident, $o: literal) => { safe_scalar_montgomery(le_u256!($v, $o)) };
}

fn write_limb(l: u64, data: &mut [u8], offset: usize) {
    let a: [u8; 8] = u64::to_le_bytes(l);

    assert!(data.len() >= offset + 8);
    for i in 0..8 {
        data[offset + i] = a[i];
    }
}

/// Little-endian montgomery-form writing of a base field element
fn write_base_montgomery(v: Fq, data: &mut [u8], offset: usize) {
    assert!(data.len() >= offset + 32);
    write_limb(v.0.0[0], data, 0 + offset);
    write_limb(v.0.0[1], data, 8 + offset);
    write_limb(v.0.0[2], data, 16 + offset);
    write_limb(v.0.0[3], data, 24 + offset);
}

/// Little-endian montgomery-form writing of a scalar field element
fn write_scalar_montgomery(v: Fr, data: &mut [u8], offset: usize) {
    assert!(data.len() >= offset + 32);
    write_limb(v.0.0[0], data, 0 + offset);
    write_limb(v.0.0[1], data, 8 + offset);
    write_limb(v.0.0[2], data, 16 + offset);
    write_limb(v.0.0[3], data, 24 + offset);
}

impl SerDe for Fr {
    type T = Fr;
    const SIZE: usize = 32;

    fn deserialize(data: &[u8]) -> Fr {
        assert!(data.len() >= 32);
        fr_montgomery!(data, 0)
    }

    fn serialize(value: Fr, data: &mut [u8]) {
        assert!(data.len() >= 32);
        write_scalar_montgomery(value, data, 0);
    }
}

impl SerDe for Fq {
    type T = Fq;
    const SIZE: usize = 32;

    fn deserialize(data: &[u8]) -> Fq {
        assert!(data.len() >= 32);
        fq_montgomery!(data, 0)
    }

    fn serialize(value: Fq, data: &mut [u8]) {
        assert!(data.len() >= 32);
        write_base_montgomery(value, data, 0);
    }
}

impl SerDe for Fq2 {
    type T = Fq2;
    const SIZE: usize = 64;

    fn deserialize(data: &[u8]) -> Fq2 {
        assert!(data.len() >= 64);

        Fq2::new(fq_montgomery!(data, 0), fq_montgomery!(data, 32))
    }

    fn serialize(value: Fq2, data: &mut [u8]) {
        assert!(data.len() >= 64);

        write_base_montgomery(value.c0.c0, data, 0);
        write_base_montgomery(value.c0.c1, data, 32);
    }
}

impl SerDe for Fq6 {
    type T = Fq6;
    const SIZE: usize = 192;

    fn deserialize(data: &[u8]) -> Fq6 {
        assert!(data.len() >= 192);

        Fq6::new(
            Fq2::new(fq_montgomery!(data, 0), fq_montgomery!(data, 32)),
            Fq2::new(fq_montgomery!(data, 64), fq_montgomery!(data, 96)),
            Fq2::new(fq_montgomery!(data, 128), fq_montgomery!(data, 160)),
        )
    }

    fn serialize(value: Fq6, data: &mut [u8]) {
        assert!(data.len() >= 192);

        write_base_montgomery(value.c0.c0, data, 0);
        write_base_montgomery(value.c0.c1, data, 32);
        write_base_montgomery(value.c1.c0, data, 64);
        write_base_montgomery(value.c1.c1, data, 96);
        write_base_montgomery(value.c2.c0, data, 128);
        write_base_montgomery(value.c2.c1, data, 160);
    }
}

impl SerDe for Fq12 {
    type T = Fq12;
    const SIZE: usize = 192;

    fn deserialize(data: &[u8]) -> Fq12 {
        assert!(data.len() >= 384);

        Fq12::new(
            Fq6::deserialize(data),
            Fq6::deserialize(&data[192..384]),
        )
    }

    fn serialize(value: Fq12, data: &mut [u8]) {
        assert!(data.len() >= 384);

        Fq6::serialize(value.c0, data);
        Fq6::serialize(value.c0, &mut data[192..384]);
    }
}

impl SerDe for G1Affine {
    type T = G1Affine;
    const SIZE: usize = 65;

    fn deserialize(data: &[u8]) -> G1Affine {
        assert!(data.len() >= 65);

        G1Affine::new(
            fq_montgomery!(data, 0),
            fq_montgomery!(data, 32),
            bool::deserialize(&data[64]),
        )
    }

    fn serialize(value: G1Affine, data: &mut [u8]) {
        assert!(data.len() >= 65);

        write_base_montgomery(value.x, data, 0);
        write_base_montgomery(value.y, data, 32);
        bool::serialize(value.infinity, &mut data[64]);
    }
}

impl SerDe for G2Affine {
    type T = G2Affine;
    const SIZE: usize = 129;

    fn deserialize(data: &[u8]) -> G2Affine {
        assert!(data.len() >= 129);

        G2Affine::new(
            Fq2::new(fq_montgomery!(data, 0), fq_montgomery!(data, 32)),
            Fq2::new(fq_montgomery!(data, 64), fq_montgomery!(data, 96)),
            bool::deserialize(&data[128]),
        )
    }

    fn serialize(value: G2Affine, data: &mut [u8]) {
        assert!(data.len() >= 129);

        write_base_montgomery(value.x, data, 0);
        write_base_montgomery(value.y, data, 32);
        write_base_montgomery(value.c1.c0, data, 64);
        write_base_montgomery(value.c1.c1, data, 96);
        bool::serialize(value.infinity, &mut data[128]);
    }
}