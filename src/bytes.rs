use crate::macros::{ write_into };
use super::fields::scalar::*;
use super::fields::utils::*;
use super::types::U256;
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

    /// Deserializes the value using the first SIZE bytes and returns the value and remaining bytes
    fn split_at_front(data: &[u8]) -> Result<(Self::T, &[u8]), ProgramError> {
        let value = data.get(..Self::SIZE)
            .map(|s| Self::deserialize(s))
            .ok_or(InvalidArgument)?;
        Ok((value, &data[Self::SIZE..]))
    }
}

pub trait Zero {
    type T;
    const ZERO: Self::T;
}

macro_rules! impl_zero {
    ($ty: ident, $zero: expr) => {
        impl Zero for $ty {
            type T = Self;
            const ZERO: Self::T = $zero;
        }
    };
}

impl_zero!(u8, 0);
impl_zero!(u64, 0);

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

macro_rules! impl_for_uint {
    ($ty: ident, $size: expr) => {
        impl SerDe for $ty {
            type T = Self;
            const SIZE: usize = $size;
        
            #[inline]
            fn deserialize(data: &[u8]) -> Self::T {
                let mut arr = [0; Self::SIZE];
                for i in 0..Self::SIZE { arr[i] = data[i]; }
                $ty::from_le_bytes(arr)
            }
        
            #[inline]
            fn serialize(value: Self::T) -> Vec<u8> {
                $ty::to_le_bytes(value).to_vec()
            }
        }
    };
}

impl_for_uint!(u32, 4);
impl_for_uint!(u64, 8);

impl SerDe for u8 {
    type T = Self;
    const SIZE: usize = 1;

    #[inline] fn deserialize(data: &[u8]) -> Self::T { data[0] }
    #[inline] fn serialize(value: Self::T) -> Vec<u8> { vec![value] }
}

impl SerDe for bool {
    type T = Self;
    const SIZE: usize = 1;

    #[inline] fn deserialize(data: &[u8]) -> Self::T { data[0] == 1 }
    #[inline] fn serialize(value: Self::T) -> Vec<u8> { vec![if value { 1 } else { 0 }] }
}

impl<const N: usize, E: SerDe<T=E> + Zero<T=E>> SerDe for [E; N] {
    type T = Self;
    const SIZE: usize = N * E::SIZE;

    fn deserialize<'a>(data: &'a mut [u8]) -> Result<Self::T, ProgramError> {
        let mut v = [E::ZERO; N];
        for i in 0..N {
            v[i] = E::deserialize(&data[i * E::SIZE..(i + 1) * E::SIZE]);
        }
        Ok(v)
    }

    fn serialize(value: Self::T) -> Vec<u8> {
        let mut buffer = Vec::new();
        for i in 0..N {
            buffer.extend(&E::serialize(value[i]));
        }
        buffer
    }
}

impl<const X: usize, const Y: usize> SerDe for [[u8; X]; Y] {
    type T = Self;
    const SIZE: usize = X * Y;

    fn deserialize<'a>(data: &'a mut [u8]) -> Result<Self::T, ProgramError> {
        let mut v = [[0; X]; Y];
        for y in 0..Y {
            write_into!(v[y], &data[y * X..(y + 1) * X]);
        }
        Ok(v)
    }

    fn serialize(value: Self::T) -> Vec<u8> {
        let mut buffer = Vec::new();
        for y in 0..Y {
            for x in 0..X {
                buffer.push(value[y][x]);
            }
        }
        buffer
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

pub fn u64_to_u256(value: u64) -> U256 {
    let mut buffer = vec![0; 32];
    let bytes = value.to_le_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        buffer[i] = byte;
    }
    vec_to_array_32(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

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