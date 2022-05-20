use solana_program::program_error::{ ProgramError, ProgramError::InvalidArgument };

/// Serialization and Deserialization trait used for account fields
pub trait SerDe {
    type T;
    const SIZE: usize;

    fn deserialize(data: &[u8]) -> Self::T;
    fn serialize(value: Self::T, data: &mut [u8]);

    fn serialize_vec(value: Self::T) -> Vec<u8> {
        let mut v = vec![0; Self::SIZE];
        Self::serialize(value, &mut v[..]);
        v
    }

    /// Deserializes the value using the first `SIZE` bytes and returns the value and remaining bytes
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

macro_rules! impl_for_uint {
    ($ty: ident, $size: expr) => {
        impl SerDe for $ty {
            type T = Self;
            const SIZE: usize = $size;
        
            #[inline]
            fn deserialize(data: &[u8]) -> Self::T {
                let mut arr = [0; Self::SIZE];
                assert!(data.len() >= Self::SIZE);
                for i in 0..Self::SIZE { arr[i] = data[i]; }
                $ty::from_le_bytes(arr)
            }
        
            #[inline]
            fn serialize(value: Self::T, data: &mut [u8]) {
                let a = $ty::to_le_bytes(value);
                assert!(data.len() >= Self::SIZE);
                for i in 0..Self::SIZE { data[i] = a[i]; }
            }
        }
    };
}

impl_for_uint!(u32, 4);
impl_for_uint!(u64, 8);

impl SerDe for u8 {
    type T = Self;
    const SIZE: usize = 1;

    #[inline] fn deserialize(data: &[u8]) -> u8 { data[0] }
    #[inline] fn serialize(value: u8, data: &mut [u8]) { data[0] = value; }
}

impl SerDe for bool {
    type T = Self;
    const SIZE: usize = 1;

    #[inline] fn deserialize(data: &[u8]) -> bool { data[0] == 1 }
    #[inline] fn serialize(value: bool, data: &mut [u8]) { data[0] = if value { 1 } else { 0 }; }
}

// Impl for array of serializable values
impl<const N: usize, E: SerDe<T=E> + Zero<T=E> + Clone + Copy> SerDe for [E; N] {
    type T = [E; N];
    const SIZE: usize = N * E::SIZE;

    fn deserialize(data: &[u8]) -> [E; N] {
        let mut v = [E::ZERO; N];
        assert!(data.len() >= Self::SIZE);
        for i in 0..N {
            v[i] = E::deserialize(&data[i * E::SIZE..(i + 1) * E::SIZE]);
        }
        v
    }

    fn serialize(value: [E; N], data: &mut [u8]) {
        assert!(data.len() >= Self::SIZE);
        for i in 0..N {
            E::serialize(
                value[i],
                &mut data[i * E::SIZE..(i + 1) * E::SIZE]
            );
        }
    }
}

// Impl for array of array of serializable values
impl<const X: usize, const Y: usize> SerDe for [[u8; X]; Y] {
    type T = [[u8; X]; Y];
    const SIZE: usize = X * Y;

    fn deserialize(data: &[u8]) -> [[u8; X]; Y] {
        let mut v = [[0; X]; Y];
        assert!(data.len() >= Self::SIZE);

        for y in 0..Y {
            let index = y * X;
            for x in 0..X {
                v[y][x] = data[index + x];
            }
        }
        v
    }

    fn serialize(value: [[u8; X]; Y], data: &mut [u8]) {
        for y in 0..Y {
            let index = y * X;
            for x in 0..X {
                data[index + x] = value[y][x];
            }
        }
    }
}

pub fn contains<N: SerDe<T=N>>(v: N, data: &[u8]) -> bool {
    let length = data.len() / N::SIZE;
    match find(v, data, length) {
        Some(_) => true,
        None => false
    }
}

pub fn find<N: SerDe<T=N>>(v: N, data: &[u8], length: usize) -> Option<usize> {
    let bytes = N::serialize_vec(v);

    assert!(data.len() >= length);
    'A: for i in 0..length {
        let index = i * 32;
        if data[index] == bytes[0] {
            for j in 1..32 {
                if data[index + j] != bytes[j] { continue 'A; }
            }
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_contains() {
        let length = 1000usize;
        let mut data = vec![0; length * 8];
        for i in 0..length {
            let bytes = u64::to_le_bytes(i as u64);
            for j in 0..8 {
                data[i * 8 + j] = bytes[j];
            }
        }

        for i in 0..length {
            assert_eq!(contains(i as u64, &data[..]), true);
            assert_eq!(find(i as u64, &data[..], length).unwrap(), i as usize);
        }
        for i in length..length + 20 {
            assert_eq!(contains(i as u64, &data[..]), false);
            assert!(matches!(find(i as u64, &data[..], length), None));
        }
    }
}