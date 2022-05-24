use borsh::{BorshDeserialize, BorshSerialize};

pub trait BorshSerDeSized: BorshSerialize + BorshDeserialize {
    const SIZE: usize;

    fn override_slice(value: &Self, slice: &mut [u8]) {
        let vec = Self::try_to_vec(value).unwrap();
        for i in 0..vec.len() {
            slice[i] = vec[i];
        }
    }
}

pub const fn max(a: usize, b: usize) -> usize {
    [a, b][(a < b) as usize]
}

macro_rules! impl_borsh_sized {
    ($ty: ty, $size: expr) => {
        impl BorshSerDeSized for $ty { const SIZE: usize = $size; }
    };
}

impl<E: BorshSerDeSized + Default + Copy, const N: usize> BorshSerDeSized for [E; N] {
    const SIZE: usize = E::SIZE * N;
}

pub(crate) use impl_borsh_sized;

impl_borsh_sized!(u8, 1);
impl_borsh_sized!(u32, 4);
impl_borsh_sized!(u64, 8);
impl_borsh_sized!(bool, 1);

pub fn contains<N: BorshSerialize + BorshSerDeSized>(v: N, data: &[u8]) -> bool {
    let length = data.len() / N::SIZE;
    match find(v, data, length) {
        Some(_) => true,
        None => false
    }
}

pub fn find<N: BorshSerialize + BorshSerDeSized>(v: N, data: &[u8], length: usize) -> Option<usize> {
    let bytes = match N::try_to_vec(&v) {
        Ok(v) => v,
        Err(_) => return None
    };

    assert!(data.len() >= length);
    'A: for i in 0..length {
        let index = i * N::SIZE;
        if data[index] == bytes[0] {
            for j in 1..N::SIZE {
                if data[index + j] != bytes[j] { continue 'A; }
            }
            return Some(i);
        }
    }
    None
}

pub fn slice_to_array<N: Default + Copy, const SIZE: usize>(s: &[N]) -> [N; SIZE] {
    assert!(s.len() >= SIZE);
    let mut a = [N::default(); SIZE];
    for i in 0..SIZE { a[i] = s[i]; }
    a
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