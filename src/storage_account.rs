use solana_program::entrypoint::ProgramResult;
use super::scalar::*;

/// Checks whether a word represented by 4 u64 limbs is contained inside a byte array
/// 
/// ### Arguments
/// 
/// * `limbs` - 4 u64 limbs
/// * `buffer` - bytes to search in
pub fn contains_limbs(limbs: ScalarLimbs, buffer: &[u8]) -> bool {
    let bytes: [u8; 32] = limbs_to_bytes(&limbs);
    let length = buffer.len() >> 5;
    for i in 0..length {
        let index = i << 5;
        if buffer[index] == bytes[0] {
            for j in 1..4 {
                if buffer[index + 1] != bytes[j] { continue; }
                return true;
            }
        }
    }
    false
}

/// Overrides the bytes in the slice
/// 
/// ### Arguments
/// 
/// * `slice` - buffer to write in
/// * `from` - start index to write from
/// * `bytecount` - amount of bytes to write
/// * `bytes` - values to set
pub fn set(slice: &mut [u8], from: usize, bytecount: usize, bytes: &[u8]) -> ProgramResult {
    for i in 0..bytecount {
        slice[from + i] = bytes[i];
    }

    Ok(())
}

pub fn bytes_to_u64(bytes: &[u8]) -> u64 {
    let a: [u8; 8] = [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]];
    u64::from_le_bytes(a)
}

pub fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let a: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    u32::from_le_bytes(a)
}

pub fn bytes_to_u16(bytes: &[u8]) -> u16 {
    let a: [u8; 2] = [bytes[0], bytes[1]];
    u16::from_le_bytes(a)
}