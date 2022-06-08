//! Bn254 base field modulus: `q = 21888242871839275222246405745257275088696311157297823662689037894645226208583`
//! Bn254 scalar field modulus: `r = 21888242871839275222246405745257275088548364400416034343698204186575808495617`

use ark_bn254::{Fr, Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine};
use ark_ff::{BigInteger256, PrimeField};
use borsh::{BorshSerialize, BorshDeserialize};
use crate::{types::{U256, u256_to_le_limbs}, bytes::BorshSerDeSized};
use crate::bytes::slice_to_array;
use crate::macros::BorshSerDeSized;

/// From &[u8] to [u8; 8]
#[macro_export]
macro_rules! u64_array {
    ($v: expr, $o: expr) => {
        [$v[$o], $v[$o + 1], $v[$o + 2], $v[$o + 3], $v[$o + 4], $v[$o + 5], $v[$o + 6], $v[$o + 7]]
    };
}

const BASE_MODULUS: BigInteger256 = BigInteger256([0x3c208c16d87cfd47, 0x97816a916871ca8d, 0xb85045b68181585d, 0x30644e72e131a029]);
const SCALAR_MODULUS: BigInteger256 = BigInteger256([4891460686036598785u64, 2896914383306846353u64, 13281191951274694749u64, 3486998266802970665u64]);

/// Constructs a base field element from an element in montgomery form
/// - panics if the supplied element is >= the base field modulus `q`
pub fn safe_base_montgomery(e: BigInteger256) -> Fq {
    if e < BASE_MODULUS { Fq::new(e) } else { panic!() }
}

/// Constructs a scalar field element from an element in montgomery form
/// - panics if the supplied element is >= the scalar field modulus `r`
pub fn safe_scalar_montgomery(e: BigInteger256) -> Fr {
    if e < SCALAR_MODULUS { Fr::new(e) } else { panic!() }
}

pub fn try_scalar_montgomery(e: BigInteger256) -> Option<Fr> {
    if e < SCALAR_MODULUS { Some(Fr::new(e)) } else { None }
}

/// BigInteger256 efficiently from LE buffer
/// - to increase efficiency callers should always assert that $v.len() >= $o + 32 (https://www.reddit.com/r/rust/comments/6anp0d/suggestion_for_a_new_rustc_optimization/dhfzp93/)
fn le_u256(slice: &[u8]) -> BigInteger256 {
    let l0 = u64_limb(slice, 0);
    let l1 = u64_limb(slice, 8);
    let l2 = u64_limb(slice, 16);
    let l3 = u64_limb(slice, 24);

    BigInteger256([ l0, l1, l2, l3 ])
}

pub fn u64_limb(slice: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(u64_array!(slice, offset))
}

/// Deserializes 32 bytes into a base field element
/// - panics if the serialized value is larger than the field modulus
macro_rules! fq_montgomery {
    ($v: expr) => { safe_base_montgomery(le_u256($v)) };
}

/// Deserializes 32 bytes into a scalar field element
/// - panics if the serialized value is larger than the field modulus
macro_rules! fr_montgomery {
    ($v: expr) => { safe_scalar_montgomery(le_u256($v)) };
}

/// Little-endian montgomery-form writing of a base field element
fn write_base_montgomery<W: std::io::Write>(v: Fq, writer: &mut W) -> std::io::Result<()> {
    writer.write_all(&u64::to_le_bytes(v.0.0[0])[..])?;
    writer.write_all(&u64::to_le_bytes(v.0.0[1])[..])?;
    writer.write_all(&u64::to_le_bytes(v.0.0[2])[..])?;
    writer.write_all(&u64::to_le_bytes(v.0.0[3])[..])
}

/// Wraps foreign types into the local scope
#[derive(Debug, PartialEq)]
pub struct Wrap<N>(pub N);

impl<T: Clone> Clone for Wrap<T> {
    fn clone(&self) -> Self {
        Wrap(self.0.clone())
    }
}

// BigInteger256
impl BorshSerDeSized for Wrap<BigInteger256> { const SIZE: usize = 32; }
impl BorshSerialize for Wrap<BigInteger256> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&u64::to_le_bytes(self.0.0[0])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0[1])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0[2])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0[3])[..])
    }
}
impl BorshDeserialize for Wrap<BigInteger256> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 32);
        let v = le_u256(buf);
        let res = Wrap(v);
        *buf = &buf[32..];
        Ok(res)
    }
}

// Fr
impl BorshSerDeSized for Wrap<Fr> { const SIZE: usize = 32; }
impl BorshSerialize for Wrap<Fr> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&u64::to_le_bytes(self.0.0.0[0])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0.0[1])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0.0[2])[..])?;
        writer.write_all(&u64::to_le_bytes(self.0.0.0[3])[..])
    }
}
impl BorshDeserialize for Wrap<Fr> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 32);
        let res = Wrap(fr_montgomery!(buf));
        *buf = &buf[32..];
        Ok(res)
    }
}

// Fq
impl BorshSerDeSized for Wrap<Fq> { const SIZE: usize = 32; }
impl BorshSerialize for Wrap<Fq> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.0, writer)
    }
}
impl BorshDeserialize for Wrap<Fq> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 32);
        let res = Wrap(fq_montgomery!(buf));
        *buf = &buf[32..];
        Ok(res)
    }
}

// Fq2
impl BorshSerDeSized for Wrap<Fq2> { const SIZE: usize = 64; }
impl BorshSerialize for Wrap<Fq2> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.0.c0, writer)?;
        write_base_montgomery(self.0.c1, writer)
    }
}
impl BorshDeserialize for Wrap<Fq2> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 64);
        let res = Fq2::new(
            fq_montgomery!(buf),
            fq_montgomery!(&buf[32..])
        );
        *buf = &buf[64..];
        Ok(Wrap(res))
    }
}

// Fq6
impl BorshSerDeSized for Wrap<Fq6> { const SIZE: usize = 192; }
impl BorshSerialize for Wrap<Fq6> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.0.c0.c0, writer)?;
        write_base_montgomery(self.0.c0.c1, writer)?;
        write_base_montgomery(self.0.c1.c0, writer)?;
        write_base_montgomery(self.0.c1.c1, writer)?;
        write_base_montgomery(self.0.c2.c0, writer)?;
        write_base_montgomery(self.0.c2.c1, writer)
    }
}
impl BorshDeserialize for Wrap<Fq6> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 192);
        let res = Wrap(Fq6::new(
            Fq2::new(fq_montgomery!(buf), fq_montgomery!(&buf[32..])),
            Fq2::new(fq_montgomery!(&buf[64..]), fq_montgomery!(&buf[96..])),
            Fq2::new(fq_montgomery!(&buf[128..]), fq_montgomery!(&buf[160..])),
        ));
        *buf = &buf[192..];
        Ok(res)
    }
}

// Fq12
impl BorshSerDeSized for Wrap<Fq12> { const SIZE: usize = 384; }
impl BorshSerialize for Wrap<Fq12> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        <Wrap<Fq6>>::serialize(&Wrap(self.0.c0), writer)?;
        <Wrap<Fq6>>::serialize(&Wrap(self.0.c1), writer)
    }
}
impl BorshDeserialize for Wrap<Fq12> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 384);
        let res = Wrap(Fq12::new(
            <Wrap<Fq6>>::deserialize(buf)?.0,
            <Wrap<Fq6>>::deserialize(buf)?.0,
        ));
        Ok(res)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct G1A(pub G1Affine);

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct G2A(pub G2Affine);

// G1A
impl BorshSerDeSized for G1A { const SIZE: usize = 65; }
impl BorshSerialize for G1A {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.0.x, writer)?;
        write_base_montgomery(self.0.y, writer)?;
        bool::serialize(&self.0.infinity, writer)?;
        Ok(())
    }
}
impl BorshDeserialize for G1A {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 65);
        let a = fq_montgomery!(buf);
        let b = fq_montgomery!(&buf[32..]);
        *buf = &buf[64..];
        Ok(G1A(G1Affine::new(a, b, bool::deserialize(buf)?)))
    }
}
impl BorshSerDeSized for Wrap<G1A> { const SIZE: usize = 65; }
impl BorshSerialize for Wrap<G1A> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> { self.0.serialize(writer) }
}
impl BorshDeserialize for Wrap<G1A> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> { Ok(Wrap(G1A::deserialize(buf)?)) }
}

// G2A
impl BorshSerDeSized for G2A { const SIZE: usize = 129; }
impl BorshSerialize for G2A {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.0.x.c0, writer)?;
        write_base_montgomery(self.0.x.c1, writer)?;
        write_base_montgomery(self.0.y.c0, writer)?;
        write_base_montgomery(self.0.y.c1, writer)?;
        bool::serialize(&self.0.infinity, writer)?;
        Ok(())
    }
}
impl BorshDeserialize for G2A {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 129);
        let x = Fq2::new(fq_montgomery!(buf), fq_montgomery!(&buf[32..]));
        let y = Fq2::new(fq_montgomery!(&buf[64..]), fq_montgomery!(&buf[96..]));
        *buf = &buf[128..];
        Ok(G2A(G2Affine::new(x, y, bool::deserialize(buf)?)))
    }
}
impl BorshSerDeSized for Wrap<G2A> { const SIZE: usize = 65; }
impl BorshSerialize for Wrap<G2A> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> { self.0.serialize(writer) }
}
impl BorshDeserialize for Wrap<G2A> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> { Ok(Wrap(G2A::deserialize(buf)?)) }
}

// Homogenous projective coordinates form
#[derive(Debug, Clone)]
pub struct G2HomProjective {
    pub x: Fq2,
    pub y: Fq2,
    pub z: Fq2,
}
impl BorshSerDeSized for G2HomProjective { const SIZE: usize = 192; }
impl BorshSerialize for G2HomProjective {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        write_base_montgomery(self.x.c0, writer)?;
        write_base_montgomery(self.x.c1, writer)?;
        write_base_montgomery(self.y.c0, writer)?;
        write_base_montgomery(self.y.c1, writer)?;
        write_base_montgomery(self.z.c0, writer)?;
        write_base_montgomery(self.z.c1, writer)
    }
}
impl BorshDeserialize for G2HomProjective {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert!(buf.len() >= 192);
        let res = G2HomProjective {
            x: Fq2::new(fq_montgomery!(buf), fq_montgomery!(&buf[32..])),
            y: Fq2::new(fq_montgomery!(&buf[64..]), fq_montgomery!(&buf[96..])),
            z: Fq2::new(fq_montgomery!(&buf[128..]), fq_montgomery!(&buf[160..])),
        };
        *buf = &buf[192..];
        Ok(res)
    }
}

pub fn u256_to_fr(v: &U256) -> Fr {
    safe_scalar_montgomery(BigInteger256(u256_to_le_limbs(*v)))
}

pub fn fr_to_u256_le(fr: &Fr) -> U256 {
    let s = <Wrap<Fr>>::try_to_vec(&Wrap(*fr)).unwrap();
    slice_to_array::<u8, 32>(&s)
}

pub fn u256_to_big_uint(v: &U256) -> BigInteger256 {
    BigInteger256(u256_to_le_limbs(*v))
}

pub fn u64_to_scalar(v: u64) -> Fr {
    Fr::from_repr(BigInteger256::from(v)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    macro_rules! test_ser_de {
        ($ty: ty, $v: expr) => {
            let v = $v;
            let bytes = <$ty>::try_to_vec(&v).unwrap();
            assert_eq!(bytes.len(), <$ty>::SIZE);
            let mut buf = &mut &bytes[..];
            let result = <$ty>::deserialize(&mut buf).unwrap();
            assert_eq!(v, result);
            assert_eq!(buf.len(), 0);
        };
    }

    #[test]
    fn test_ser_de_big_integer_256() {
        test_ser_de!(
            Wrap<BigInteger256>,
            Wrap(BigInteger256::from(123456789))
        );
    }

    #[test]
    fn test_ser_de_fr() {
        test_ser_de!(
            Wrap<Fr>,
            Wrap(Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap())
        );
    }

    #[test]
    fn test_ser_de_fq() {
        test_ser_de!(
            Wrap<Fq>,
            Wrap(Fq::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap())
        );
    }

    #[test]
    fn test_ser_de_fq2() {
        test_ser_de!(
            Wrap<Fq2>,
            Wrap(Fq2::new(
                Fq::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap(),
                Fq::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()
            ))
        );
    }

    #[test]
    fn test_ser_de_fq6() {
        test_ser_de!(
            Wrap<Fq6>,
            Wrap(Fq6::new(
                Fq2::new(
                    Fq::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap(),
                    Fq::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()
                ),
                Fq2::new(
                    Fq::from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362").unwrap(),
                    Fq::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap()
                ),
                Fq2::new(
                    Fq::from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794").unwrap(),
                    Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap()
                ),
            ))
        );
    }

    #[test]
    fn test_ser_de_fq12() {
        test_ser_de!(
            Wrap<Fq12>,
            Wrap(Fq12::new(
                Fq6::new(
                    Fq2::new(
                        Fq::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap(),
                        Fq::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap()
                    ),
                    Fq2::new(
                        Fq::from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362").unwrap(),
                        Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap()
                    ),
                    Fq2::new(
                        Fq::from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794").unwrap(),
                        Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap()
                    ),
                ),
                Fq6::new(
                    Fq2::new(
                        Fq::from_str("15798376151120407607995325383260410478881539926269713789760505676493608861934").unwrap(),
                        Fq::from_str("10053855256797203809243706937712819679696785488432523709871608122822392032095").unwrap()
                    ),
                    Fq2::new(
                        Fq::from_str("17221088121480185305804562315627270623879289277074607312826677888427107195721").unwrap(),
                        Fq::from_str("12873223109498890755823667267246854666756739205168367165343839421529315277098").unwrap()
                    ),
                    Fq2::new(
                        Fq::from_str("7853200120776062878684798364095072458815029376092732009249414926327459813530").unwrap(),
                        Fq::from_str("413257311912083837973810345705464536164975713199103663810842263819736").unwrap()
                    ),
                )
            ))
        );
    }

    #[test]
    fn test_ser_de_g1a() {
        test_ser_de!(
            G1A,
            G1A(G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
                false
            ))
        );
    }

    #[test]
    fn test_ser_de_g2a() {
        test_ser_de!(
            G2A,
            G2A(G2Affine::new(
                Fq2::new(
                    Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                    Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362").unwrap(),
                    Fq::from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794").unwrap(),
                ),
                false
            ))
        );
    }

    #[test]
    fn test_fr_u256_parsing() {
        let f = Fr::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap();
        let u = fr_to_u256_le(&f);
        assert_eq!(f, u256_to_fr(&u));
    }

    #[test]
    fn test_u64_to_scalar() {
        assert_eq!(u64_to_scalar(999), Fr::from_str("999").unwrap());
    }
}