use ark_bn254::{ Fr };
use ark_ff::*;
use std::str::FromStr;
use byteorder::{ ByteOrder, LittleEndian };

pub type Scalar = Fr;
pub type ScalarLimbs = [u64; 4];

/// Returns a new scalar from 4 u64 limbs
/// 
/// ### Arguments
/// 
/// * `limbs` - 4 u64 limbs
pub fn from_limbs(limbs: &[u64]) -> Fr {
    BigInteger256([limbs[0], limbs[1], limbs[2], limbs[3]]).into()
}

/// Ruturns a Scalar from 32 little endian bytes
/// 
/// # Arguments
/// 
/// * `bytes` - 32 little endian bytes
pub fn from_bytes_le(bytes: &[u8]) -> Scalar {
    if bytes.len() != 32 { panic!("Invalid byte amount (32 bytes required)") }
    BigInteger256(bytes_to_limbs(bytes)).into()
}

/// Returns 4 u64 limbs from 32 bytes
/// 
/// # Arguments
/// 
/// * `bytes` - 32 little endian bytes
pub fn bytes_to_limbs(bytes: &[u8]) -> [u64; 4] {
    [
        LittleEndian::read_u64(&bytes[..8]),
        LittleEndian::read_u64(&bytes[8..16]),
        LittleEndian::read_u64(&bytes[16..24]),
        LittleEndian::read_u64(&bytes[24..]),
    ]
}

/// Returns 32 bytes in little endian
/// 
/// # Arguments
/// 
/// * `limbs` - 32 little endian bytes
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

/// Returns 32 little endian bytes
/// 
/// # Arguments
/// 
/// * `scalar` - a 256 bit scalar
pub fn to_bytes_le(scalar: Scalar) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    scalar.write(&mut writer).unwrap();
    writer
}

/// Returns a hex string representation with leading zeros
/// 
/// # Arguments
/// 
/// * `scalar` - a 256 bit scalar
pub fn to_hex_string(scalar: Scalar) -> String {
    let mut str = String::from("0x");
    let bytes: Vec<u8> = to_bytes_le(scalar).into_iter().rev().collect();
    for byte in bytes {
        str.push_str(&format!("{:02x}", byte).to_uppercase());
    }
    str
}

/// Parses a base 10 string into a Scalar
pub fn from_str_10(s: &str) -> Scalar {
    Fr::from_str(s).unwrap()
}

/// Parses a base 16 string into a Scalar
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

    Some(from_bytes_le(&bytes))
}

#[cfg(test)]
mod test {
    use super::*;

    fn hex_string(scalar: Scalar) -> String {
        scalar.to_string().replace("Fp256 \"(", "").replace(")\"", "")
    }

    #[test]
    fn test_from_bytes() {
        let mut source: Vec<u8> = vec![0; 32];
        source[0] = 1;

        let f = from_bytes_le(&source);
        assert_eq!(
            "0000000000000000000000000000000000000000000000000000000000000001",
            hex_string(f)
        )
    }

    #[test]
    fn test_from_bytes2() {
        let bytes_be: Vec<u8> = vec![
            0x09, 0xC4, 0x6E, 0x9E, 0xC6, 0x8E, 0x9B, 0xD4,
            0xFE, 0x1F, 0xAA, 0xBA, 0x29, 0x4C, 0xBA, 0x38,
            0xA7, 0x1A, 0xA1, 0x77, 0x53, 0x4C, 0xDD, 0x1B,
            0x6C, 0x7D, 0xC0, 0xDB, 0xD0, 0xAB, 0xD7, 0xA7,
        ];
        let bytes_le: Vec<u8> = bytes_be.into_iter().rev().collect();

        let f = from_bytes_le(&bytes_le);
        assert_eq!(
            "09C46E9EC68E9BD4FE1FAABA294CBA38A71AA177534CDD1B6C7DC0DBD0ABD7A7",
            hex_string(f)
        )
    }

    #[test]
    fn test_to_bytes() {
        let f = from_str_10("3");
        let bytes = to_bytes_le(f);
        assert_eq!(bytes[0], 3);
        for i in 1..32 {
            assert_eq!(bytes[i], 0);
        }
    }

    #[test]
    fn test_from_string() {
        let f = from_str_10("4417881134626180770308697923359573201005643519861877412381846989312604493735");
        let bytes_be: Vec<u8> = vec![
            0x09, 0xC4, 0x6E, 0x9E, 0xC6, 0x8E, 0x9B, 0xD4,
            0xFE, 0x1F, 0xAA, 0xBA, 0x29, 0x4C, 0xBA, 0x38,
            0xA7, 0x1A, 0xA1, 0x77, 0x53, 0x4C, 0xDD, 0x1B,
            0x6C, 0x7D, 0xC0, 0xDB, 0xD0, 0xAB, 0xD7, 0xA7,
        ];
        let bytes_le: Vec<u8> = bytes_be.into_iter().rev().collect();

        assert_eq!(
            bytes_le,
            to_bytes_le(f)
        )
    }

    #[test]
    fn test_from_string_hex_valid() {
        assert_eq!(Scalar::zero(), from_str_16("0x0").unwrap());

        let n = to_bytes_le(from_str_16("0xABCDEF10").unwrap());
        println!("{:?}", n);
        assert_eq!(n[0], 0b00010000);
        assert_eq!(n[1], 0b11101111);
        assert_eq!(n[2], 0b11001101);
        assert_eq!(n[3], 0b10101011);

        assert_eq!(
            from_str_10("4417881134626180770308697923359573201005643519861877412381846989312604493735"),
            from_str_16("0x9C46E9EC68E9BD4FE1FAABA294CBA38A71AA177534CDD1B6C7DC0DBD0ABD7A7").unwrap(),
        );
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