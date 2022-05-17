use solana_program::program_error::{ ProgramError, ProgramError::InvalidArgument };

/// Serialization and Deserialization trait used for account fields
pub trait SerDe {
    type T;
    const SIZE: usize;

    fn deserialize(data: &[u8]) -> Self::T;
    fn serialize(value: Self::T, data: &mut [u8]);

    fn serialize_vec(value: Self::T, zero: Self::T) -> Vec<u8> {
        let mut v = vec![Self::SIZE; zero];
        Self::serialize(value, &mut v);
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

/// This trait generates the backing store object for account fields
/// - why is this needed?
///     - our usual jit approach for serialization is maintaining a mutable slice with the bytes and have getter/setter functions
///     - sometimes we want special data-structures, like our LazyRAM which on it's own manages the mutable slice
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

    #[inline] fn deserialize(data: &[u8]) -> Self::T { data[0] }
    #[inline] fn serialize(value: Self::T) -> Vec<u8> { vec![value] }
}

impl SerDe for bool {
    type T = Self;
    const SIZE: usize = 1;

    #[inline] fn deserialize(data: &[u8]) -> Self::T { data[0] == 1 }
    #[inline] fn serialize(value: Self::T) -> Vec<u8> { vec![if value { 1 } else { 0 }] }
}

// Impl for array of serializable values
impl<const N: usize, E: SerDe<T=E> + Zero<T=E>> SerDe for [E; N] {
    type T = [E; N];
    const SIZE: usize = N * E::SIZE;

    fn deserialize<'a>(data: &'a mut [u8]) -> [E; N] {
        let mut v = [E::ZERO; N];
        assert!(data.len() >= Self::SIZE);
        for i in 0..N {
            v[i] = E::deserialize(&data[i * E::SIZE..(i + 1) * E::SIZE]);
        }
        v
    }

    fn serialize(value: Self::T, data: &mut [u8]) {
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

    fn deserialize<'a>(data: &'a mut [u8]) -> [[u8; X]; Y] {
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

pub fn contains<N: SerDe<T=N> + Zero>(v: N, data: &[u8]) -> bool {
    let length = data.len() / N::SIZE;
    match find(v, data, length) {
        Some(_) => true,
        None => false
    }
}

pub fn not_contains<N: SerDe<T=N> + Zero>(v: N, data: &[u8]) -> bool {
    !contains(v, data)
}

pub fn find<N: SerDe<T=N> + Zero>(v: N, data: &[u8], length: usize) -> Option<usize> {
    let bytes = N::serialize_vec(v, N::ZERO);

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
        let length = u64::MAX / 8;
        let data = vec![0; length];
        for i in 0..length { data[i] = i; }

        for i in 0..length {
            assert_eq!(contains(i, &data[..]), true);
            assert_eq!(find(i, &data[..], length).unwrap(), i as usize);
        }
        for i in length..length + 20 {
            assert_eq!(not_contains(i, &data[..]), true);
            assert_eq!(contains(i, &data[..]), false);
            assert!(matches!(find(i, &data[..], length).unwrap(), None));
        }
    }
}