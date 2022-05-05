use crate::types::RawProof;
use crate::macros::write_into;
use super::fields::scalar::*;
use super::proof::PROOF_BYTES_SIZE;
use super::fields::utils::*;
use super::types::U256;
//use borsh::{ BorshSerialize, BorshDeserialize };
use solana_program::program_error::{ ProgramError, ProgramError::InvalidArgument };

/// Serialization and Deserialization trait used for account fields
pub trait SerDe {
    type T;
    const SIZE: usize;
    fn deserialize(data: &[u8]) -> Self::T;
    fn serialize(value: Self::T) -> Vec<u8>;

    /// Overwrites the values in the writer slice
    fn write(value: Self::T, writer: &[u8]) {
        write_into!(writer, Self::serialize(value));
    }
}

/// This trait generates the backing store object for account fields
/// - why is this needed? Since not all top-level objects are SerDe objects (e.g. LazyStacks)
/// - so for them field would return a LazyStack and for a u64 e.g. it would just return data again
pub trait SerDeManager<T> {
    const SIZE_BYTES: usize;

    /// Returns either data or a special field handeling se/de on it's own (e.g. a LazyHeapStack)
    fn mut_backing_store<'a>(data: &'a mut [u8]) -> Result<T, ProgramError>;
}

/// SerDeManager default impl for all types that impl SerDe themselves (atomic types)
impl<T: SerDe> SerDeManager<&mut[u8]> for T {
    const SIZE_BYTES: usize = Self::SIZE;

    fn mut_backing_store<'a>(data: &'a mut [u8]) -> Result<&mut [u8], ProgramError> {
        Ok(data)
    }
}

impl SerDe for u32 {
    type T = Self;
    const SIZE: usize = 4;

    fn deserialize(data: &[u8]) -> u32 {
        u32::from_le_bytes([data[0], data[1], data[2], data[3]])
    }

    fn serialize(value: u32) -> Vec<u8> {
        u32::to_le_bytes(value).to_vec()
    }
}

impl SerDe for u64 {
    type T = Self;
    const SIZE: usize = 8;

    fn deserialize<'a>(data: &'a mut [u8]) -> Result<u64, ProgramError> {
        Ok(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]))
    }

    fn serialize(value: u64) -> Vec<u8> {
        u64::to_le_bytes(value).to_vec()
    }
}

impl SerDe for U256 {
    type T = Self;
    const SIZE: usize = 32;

    fn deserialize<'a>(data: &'a mut [u8]) -> Result<U256, ProgramError> {
        let mut u256 = [0; 32];
        write_into!(u256, data);
        Ok(u256)
    }

    fn serialize(value: U256) -> Vec<u8> {
        value.to_vec()
    }
}

pub fn contains(bytes: U256, buffer: &[u8]) -> bool {
    match find(bytes, buffer) {
        Some(_) => true,
        None => false
    }
}

pub fn not_contains(bytes: U256, buffer: &[u8]) -> bool {
    !contains(bytes, buffer)
}

pub fn find(bytes: U256, buffer: &[u8]) -> Option<usize> {
    let length = buffer.len() / 32;
    'A: for i in 0..length {
        let index = i * 32;
        if buffer[index] == bytes[0] {
            for j in 1..32 {
                if buffer[index + j] != bytes[j] { continue 'A; }
            }
            return Some(i);
        }
    }
    None
}

pub fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let a: [u8; 8] = [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]];
    u64::from_le_bytes(a)
}

pub fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let a: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    u32::from_le_bytes(a)
}

pub fn unpack_u64(data: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
    let value = data
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(InvalidArgument)?;

    Ok((value, &data[8..]))
}

pub fn unpack_32_bytes(data: &[u8]) -> Result<(&[u8], &[u8]), ProgramError> {
    let bytes = data.get(..32)
        .ok_or(ProgramError::InvalidInstructionData)?;

    Ok((bytes, &data[32..]))
}

pub fn unpack_u256(data: &[u8]) -> Result<(U256, &[u8]), ProgramError> {
    let (bytes, data) = unpack_32_bytes(&data)?;
    let word = vec_to_array_32(bytes.to_vec());

    Ok((word, &data))
}


pub fn unpack_bool(data: &[u8]) -> Result<(bool, &[u8]), ProgramError> {
    let (&byte, rest) = data.split_first().ok_or(ProgramError::InvalidInstructionData)?;

    Ok((byte == 1, rest))
}

pub fn unpack_raw_proof(data: &[u8]) -> Result<(RawProof, &[u8]), ProgramError> {
    let bytes = data.get(..PROOF_BYTES_SIZE).ok_or(ProgramError::InvalidInstructionData)?;
    let proof: [u8; PROOF_BYTES_SIZE] = bytes.try_into().unwrap();

    Ok((proof, &data[PROOF_BYTES_SIZE..]))
}

pub fn u64_to_u256(value: u64) -> U256 {
    let mut buffer = vec![0; 32];
    let bytes = value.to_le_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        buffer[i] = byte;
    }
    vec_to_array_32(buffer)
}

pub fn unpack_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_32_bytes(data)?;

    Ok((bytes_to_limbs(bytes), data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpack_u64() {
        let d: [u8; 8] = [0b00000001, 0, 0, 0, 0, 0, 0, 0b00000000];

        let (v, data) = unpack_u64(&d).unwrap();
        assert_eq!(v, 1);
        assert_eq!(data.len(), 0);
    }

    const SIZE: usize = 256;    // Max using 256 here because of u8 then there are duplicates

    fn generate_buffer() -> Vec<u8> {
        let mut buffer = Vec::new();
        for i in 0..SIZE {
            for _ in 0..32 {
                buffer.push(i as u8);
            }
        }
        buffer
    }

    #[test]
    fn test_find_contains() {
        let buffer = generate_buffer();

        // Contains & finds
        for i in 0..SIZE {
            let bytes = [i as u8; 32];

            assert!(contains(bytes, &buffer));
            assert_eq!(not_contains(bytes, &buffer), false);

            let index = find(bytes, &buffer).unwrap();
            assert_eq!(index, i);
        }

        // Doesn't contain & find
        for i in 0..32 {
            let mut bytes = [0; 32];
            bytes[i] = 1;

            assert!(not_contains(bytes, &buffer));
            assert_eq!(contains(bytes, &buffer), false);

            assert!(matches!(find(bytes, &buffer), None));
        }
    }
}