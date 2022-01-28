use ark_bn254::{ Fr };
use ark_ff::*;
use std::str::FromStr;
use byteorder::{ ByteOrder, LittleEndian };

/// Bn254 scalar
/// - Circom uses `r=21888242871839275222246405745257275088548364400416034343698204186575808495617` so, we use Fr (not Fq)
pub type Scalar = Fr;

/// Little endian limbs (least signification 64 bits of 256 bits first)
pub type ScalarLimbs = [u64; 4];

// Internal field element representation is in "Montgomery form"
// - Fr::new(BigInteger256(limbs)) where limbs is in LE of 4 u64
// Human readable representation is in "representation form"
// - Fr::from_repr(BigInteger256(limbs)) where limbs is in LE of 4 u64

/// Returns a Scalar from 4 le limbs in Montgomery form
pub fn from_limbs_mont(limbs: &[u64]) -> Scalar {
    Fr::new(BigInteger256([limbs[0], limbs[1], limbs[2], limbs[3]]))
}

/// Returns a Scalar from 4 le limbs in representation form
/// - returns None if the supplied number is >= r
pub fn from_limbs_repr(limbs: &[u64]) -> Option<Scalar> {
    Fr::from_repr(BigInteger256([limbs[0], limbs[1], limbs[2], limbs[3]]))
}

/// Ruturns a Scalar from 32 le bytes in Montgomery form
pub fn from_bytes_le_mont(bytes: &[u8]) -> Scalar {
    Fr::new(BigInteger256(bytes_to_limbs(bytes)))
}

/// Ruturns a Scalar from 32 le bytes in representation form
/// - returns None if the supplied number is >= r
pub fn from_bytes_le_repr(bytes: &[u8]) -> Option<Scalar> {
    Fr::from_repr(BigInteger256(bytes_to_limbs(bytes)))
}

/// Returns 32 le bytes in Montgomery form
pub fn to_bytes_le_mont(scalar: Scalar) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    scalar.0.write(&mut writer).unwrap();
    writer
}

/// Returns 32 le bytes in representation form
pub fn to_bytes_le_repr(scalar: Scalar) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    scalar.write(&mut writer).unwrap();
    writer
}

/// Returns 4 le u64 limbs from 32 le bytes
pub fn bytes_to_limbs(bytes: &[u8]) -> [u64; 4] {
    [
        LittleEndian::read_u64(&bytes[..8]),
        LittleEndian::read_u64(&bytes[8..16]),
        LittleEndian::read_u64(&bytes[16..24]),
        LittleEndian::read_u64(&bytes[24..]),
    ]
}

/// Returns 32 bytes in little endian
pub fn limbs_to_bytes(limbs: &[u64]) -> [u8; 32] {
    let mut bytes: [u8; 32] = [0; 32];
    for i in 0..4 {
        let b = limbs[i].to_le_bytes();
        for j in 0..8 {
            bytes[(i << 3) + j] = b[j];
        }
    }
    bytes
}

/// Returns a hex string representation with leading zeros in representation form
pub fn to_hex_string(scalar: Scalar) -> String {
    let mut str = String::from("0x");
    let bytes: Vec<u8> = to_bytes_le_repr(scalar).into_iter().rev().collect();
    for byte in bytes {
        str.push_str(&format!("{:02x}", byte).to_uppercase());
    }
    str
}

/// Parses a base 10 string (in representation form) into a Scalar
pub fn from_str_10(s: &str) -> Scalar {
    Fr::from_str(s).unwrap()
}

/// Parses a base 16 string (in representation form) into a Scalar
pub fn from_str_16(s: &str) -> Option<Scalar> {
    let s = s.trim_start_matches("0x");
    let length = s.len();
    if length > 64 { return None; }

    let s = if s.len() % 2 != 0 { String::from("0") + s } else { String::from(s) };
    let mut bytes: Vec<u8> = vec![0; 32];
    let mut val = 0;
    for (i, c) in s.chars().rev().enumerate() { 
        val += match c {
            '0' => 0, '1' => 1, '2' => 2, '3' => 3, '4' => 4, '5' => 5, '6' => 6, '7' => 7,
            '8' => 8, '9' => 9, 'A' => 10, 'B' => 11, 'C' => 12, 'D' => 13, 'E' => 14, 'F' => 15,
            _ => panic!("Wrong hex char supplied"),
        } * (if i % 2 == 0 { 1 } else { 16 });

        if (i + 1) % 2 == 0 {
            bytes[i / 2] = val;
            val = 0;
        }
    }

    from_bytes_le_repr(&bytes)
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::super::poseidon::*;

    fn hex_string(scalar: Scalar) -> String {
        scalar.to_string().replace("Fp256 \"(", "").replace(")\"", "")
    }

    #[test]
    fn test_from_bytes() {
        // value: 14744269619966411208579211824598458697587494354926760081771325075741142829156
        let mont = from_bytes_le_mont(&vec![130, 154, 1, 250, 228, 248, 226, 43, 27, 76, 165, 173, 91, 84, 165, 131, 78, 224, 152, 167, 123, 115, 91, 213, 116, 49, 167, 101, 109, 41, 161, 8]);
        let repr = from_bytes_le_repr(&vec![100, 72, 182, 70, 132, 238, 57, 168, 35, 213, 254, 95, 213, 36, 49, 220, 129, 228, 129, 123, 242, 195, 234, 60, 171, 158, 35, 158, 251, 245, 152, 32]).unwrap();
        let hash = Poseidon2::new().full_hash(Fr::zero(), Fr::zero());

        assert_eq!(mont, repr);
        assert_eq!(mont, hash);
    }

    #[test]
    fn test_from_limbs() {
        // value: 14744269619966411208579211824598458697587494354926760081771325075741142829156
        let mont = from_limbs_mont(&vec![3162363550698150530, 9486080942857866267, 15374008727889305678, 621823773387469172]);
        let repr = from_limbs_repr(&vec![12121982123933845604, 15866503461060138275, 4389536233047581825, 2348897666712444587]).unwrap();
        let hash = Poseidon2::new().full_hash(Fr::zero(), Fr::zero());

        assert_eq!(mont, repr);
        assert_eq!(mont, hash);
    }

    #[test]
    fn test_to_bytes() {
        let n = from_str_10("3");
        let repr = to_bytes_le_repr(n);
        let mont = to_bytes_le_mont(n);

        // Check for different byte representation
        assert_ne!(repr, mont);

        assert_eq!(repr[0], 3);
        for i in 1..32 {
            assert_eq!(repr[i], 0);
        }
    }

    #[test]
    fn test_from_string() {
        let dec = from_str_10("14744269619966411208579211824598458697587494354926760081771325075741142829156");
        let hex = from_str_16("2098F5FB9E239EAB3CEAC3F27B81E481DC3124D55FFED523A839EE8446B64864").unwrap();
        let hash = Poseidon2::new().full_hash(Fr::zero(), Fr::zero());

        assert_eq!(dec, hex);
        assert_eq!(dec, hash);
    }

    #[test]
    #[should_panic]
    fn test_from_string_invalid() {
        from_str_16("0G").unwrap();
    }

    #[test]
    fn test_limbs_to_bytes() {
        let limbs: [u64; 4] = [
            u64::from_le_bytes([1, 0, 0, 0, 0, 0, 0, 0]),
            0,
            0,
            0,
        ];
        let mut bytes: [u8; 32] = [0; 32]; bytes[0] = 1; 

        let f = limbs_to_bytes(&limbs);
        assert_eq!(f, bytes);
    }

    #[test]
    fn test_bytes_to_limbs() {
        let mut bytes: [u8; 32] = [0; 32]; bytes[0] = 1; 
        let limbs: [u64; 4] = [
            u64::from_le_bytes([1, 0, 0, 0, 0, 0, 0, 0]),
            0,
            0,
            0,
        ];

        let f = bytes_to_limbs(&bytes);
        assert_eq!(f, limbs);
    }
}