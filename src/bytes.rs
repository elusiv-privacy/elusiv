use std::{collections::BTreeMap, hash::Hash};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::error::{ElusivError, ElusivResult};
use solana_program::pubkey::Pubkey;

pub trait BorshSerDeSized: BorshSerialize + BorshDeserialize {
    const SIZE: usize;

    fn override_slice(value: &Self, slice: &mut [u8]) -> Result<(), std::io::Error> {
        let vec = Self::try_to_vec(value)?;
        slice[..vec.len()].copy_from_slice(&vec[..]);
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
/// The advantage of `ElusivOption` over `Option` is fixed serialization length
pub enum ElusivOption<N> {
    Some(N),
    None,
}

impl<N> From<Option<N>> for ElusivOption<N> {
    fn from(o: Option<N>) -> Self {
        match o {
            Some(v) => ElusivOption::Some(v),
            None => ElusivOption::None
        }
    }
}

impl<N: Clone> ElusivOption<N> {
    pub fn option(&self) -> Option<N> {
        match self {
            ElusivOption::Some(v) => Option::Some(v.clone()),
            ElusivOption::None => Option::None
        }
    }
}

impl<T: BorshSerDeSized> BorshDeserialize for ElusivOption<T> {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        if buf[0] == 0 {
            *buf = &buf[<ElusivOption<T>>::SIZE..];
            Ok(ElusivOption::None)
        } else {
            *buf = &buf[1..];
            let v = T::deserialize(buf)?;
            Ok(ElusivOption::Some(v))
        }
    }
}

impl<T: BorshSerDeSized> BorshSerialize for ElusivOption<T> {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        match self {
            ElusivOption::Some(v) => {
                writer.write_all(&[1])?;
                v.serialize(writer)
            }
            ElusivOption::None => {
                writer.write_all(&[0])?;
                writer.write_all(&vec![0; T::SIZE])?;
                Ok(())
            }
        }
    }
}

impl<T> Default for ElusivOption<T> {
    fn default() -> Self { ElusivOption::None }
}

impl<T: BorshSerDeSized> BorshSerDeSized for ElusivOption<T> {
    const SIZE: usize = 1 + T::SIZE;
}

impl BorshSerDeSized for Pubkey {
    const SIZE: usize = 32;
}

impl BorshSerDeSized for () {
    const SIZE: usize = 0;
}

pub const fn max(a: usize, b: usize) -> usize {
    [a, b][if a < b { 1 } else { 0 }]
}

/// Rounds a integer division up
pub const fn div_ceiling(divident: u64, divisor: u64) -> u64 {
    if divisor == 0 { panic!() }
    (divident + divisor - 1) / divisor
}

pub const fn u64_as_usize_safe(u: u64) -> usize {
    u64_as_u32_safe(u) as usize
}

pub const fn u64_as_u32_safe(u: u64) -> u32 {
    if u > u32::MAX as u64 { panic!() }
    u as u32
}

/// Ensures the safety of a cast from usize to u32 on a 64-bit architecture
pub const fn usize_as_u32_safe(u: usize) -> u32 {
    if u > u32::MAX as usize { panic!() }
    u as u32
}

pub const fn usize_as_u8_safe(u: usize) -> u8 {
    if u > u8::MAX as usize { panic!() }
    u as u8
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
impl_borsh_sized!(u16, 2);
impl_borsh_sized!(u32, 4);
impl_borsh_sized!(u64, 8);
impl_borsh_sized!(u128, 16);
impl_borsh_sized!(bool, 1);

// TODO: optimize find and contains with byte alignment
pub fn contains<N: BorshSerialize + BorshSerDeSized>(v: N, data: &[u8]) -> bool {
    let length = data.len() / N::SIZE;
    find(v, data, length).is_some()
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

pub fn is_zero(s: &[u8]) -> bool {
    for i in (0..s.len()).step_by(16) {
        if s.len() - i >= 16 {
            let arr: [u8; 16] = s[i..i+16].try_into().unwrap();
            if u128::from_be_bytes(arr) != 0 { return false }
        } else {
            for &bit in s.iter().skip(i) {
                if bit != 0 { return false }
            }
        }
    }
    true
}

pub fn slice_to_array<N: Default + Copy, const SIZE: usize>(s: &[N]) -> [N; SIZE] {
    assert!(s.len() >= SIZE);
    let mut a = [N::default(); SIZE];
    a[..SIZE].copy_from_slice(&s[..SIZE]);
    a
}

/// `BTreeMap` that serializes to a fixed size (`SIZE`) with a maximum entry count (`COUNT`)
#[derive(Clone)]
pub struct ElusivBTreeMap<K, V, const COUNT: usize>(BTreeMap<K, V>) where K: Hash + Ord + BorshSerDeSized, V: BorshSerDeSized;

impl<K, V, const COUNT: usize> BorshSerDeSized for ElusivBTreeMap<K, V, COUNT>
where K: Hash + Ord + BorshSerDeSized, V: BorshSerDeSized {
    const SIZE: usize = (K::SIZE + V::SIZE) * COUNT + 4;
}

impl<K, V, const COUNT: usize> BorshDeserialize for ElusivBTreeMap<K, V, COUNT>
where K: Hash + Ord + BorshSerDeSized, V: BorshSerDeSized {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        assert_eq!(buf.len(), Self::SIZE);
        
        let len = 4 + u32::try_from_slice(&buf[..4])? as usize * (K::SIZE + V::SIZE);
        let map = <BTreeMap<K, V>>::try_from_slice(&buf[..len])?;
        *buf = &buf[Self::SIZE..];

        Ok(Self(map))
    }
}

impl<K, V, const COUNT: usize> BorshSerialize for ElusivBTreeMap<K, V, COUNT>
where K: Hash + Ord + BorshSerDeSized, V: BorshSerDeSized {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let v = self.0.try_to_vec()?;
        writer.write_all(&v)?;
        writer.write_all(&vec![0; Self::SIZE - v.len()])?;
        Ok(())
    }
}

impl<K, V, const COUNT: usize> ElusivBTreeMap<K, V, COUNT>
where K: Hash + Ord + BorshSerDeSized, V: BorshSerDeSized {
    pub const MAX_ELEMENTS_COUNT: usize = COUNT;

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn try_insert(&mut self, k: K, v: V) -> ElusivResult {
        if self.0.len() >= COUNT {
            return Err(ElusivError::NoRoomForNullifier)
        }
        self.0.insert(k, v);
        Ok(())
    }

    pub fn remove(&mut self, k: &K) {
        self.0.remove(k);
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.0.contains_key(k)
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        self.0.get(k)
    }

    #[allow(clippy::len_without_is_empty)] 
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{macros::BorshSerDeSized, types::U256};
    use assert_matches::assert_matches;

    #[test]
    fn test_max() {
        assert_eq!(max(1, 3), 3);
        assert_eq!(max(3, 1), 3);
    }

    #[test]
    fn test_div_ceiling() {
        assert_eq!(div_ceiling(3, 2), 2);
        assert_eq!(div_ceiling(4, 3), 2);
        assert_eq!(div_ceiling(7, 3), 3);
    }

    #[test]
    #[should_panic]
    fn test_div_ceiling_zero() {
        div_ceiling(0, 0);
    }

    #[test]
    fn test_pubkey_ser_de() {
        assert_eq!(Pubkey::SIZE, Pubkey::new_unique().try_to_vec().unwrap().len());
    }

    #[test]
    fn test_u64_as_usize_safe() {
        assert_eq!(u64_as_usize_safe(u32::MAX as u64), u32::MAX as usize);
    }

    #[test]
    #[should_panic]
    fn test_u64_as_usize_safe_panic() {
        assert_eq!(u64_as_usize_safe(u32::MAX as u64 + 1), u32::MAX as usize + 1);
    }

    #[test]
    fn test_u64_as_u32_safe() {
        assert_eq!(u64_as_u32_safe(u32::MAX as u64), u32::MAX);
    }

    #[test]
    #[should_panic]
    fn test_u64_as_u32_safe_panic() {
        assert_eq!(u64_as_u32_safe(u32::MAX as u64 + 1), u32::MAX);
    }

    #[test]
    fn test_usize_as_u32_safe() {
        assert_eq!(usize_as_u32_safe(u32::MAX as usize), u32::MAX);
    }

    #[test]
    #[should_panic]
    fn test_usize_as_u32_safe_panic() {
        assert_eq!(usize_as_u32_safe(u32::MAX as usize + 1), u32::MAX);
    }

    #[test]
    fn test_usize_as_u8_safe() {
        assert_eq!(usize_as_u8_safe(u8::MAX as usize), u8::MAX);
    }

    #[test]
    #[should_panic]
    fn test_usize_as_u8_safe_panic() {
        assert_eq!(usize_as_u8_safe(u8::MAX as usize + 1), u8::MAX);
    }

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
            assert!(contains(i as u64, &data[..]));
            assert_eq!(find(i as u64, &data[..], length).unwrap(), i as usize);
        }
        for i in length..length + 20 {
            assert!(!contains(i as u64, &data[..]));
            assert!(matches!(find(i as u64, &data[..], length), None));
        }
    }

    #[test]
    fn test_override_slice() {
        let mut slice = vec![0; 256];
        U256::override_slice(&[1; 32], &mut slice[32..64]).unwrap();

        for &v in slice.iter().take(64).skip(32) {
            assert_eq!(v, 1);
        }
    }

    #[derive(BorshDeserialize, BorshSerialize)]
    struct A { }
    impl_borsh_sized!(A, 11);

    #[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
    struct B { a0: A, a1: A, a2: A }

    #[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
    enum C {
        A { a: A },
        B { b: B },
        AB { a: A, b: B },
    }

    #[test]
    fn test_borsh_ser_de_sized() {
        assert_eq!(A::SIZE, 11);
        assert_eq!(B::SIZE, 33);
        assert_eq!(C::SIZE, 11 + 33 + 1);
    }

    #[test]
    fn test_elusiv_map_ser_de() {
        type T = ElusivBTreeMap<U256, u64, 100>;

        assert_eq!(T::SIZE, 100 * (32 + 8) + 4);

        let mut map = T::new();
        assert_eq!(map.try_to_vec().unwrap().len(), T::SIZE);

        map.try_insert([1; 32], 22).unwrap();
        map.try_insert([3; 32], 444).unwrap();
        let v = map.try_to_vec().unwrap();

        assert_eq!(v.len(), T::SIZE);
        assert_eq!(T::try_from_slice(&v[..]).unwrap().0, map.0);
    }

    #[test]
    fn test_elusiv_map() {
        let mut map: ElusivBTreeMap<[u32; 32], u32, 10> = ElusivBTreeMap::new();

        map.try_insert([1; 32], 123).unwrap();
        assert_eq!(*map.0.get(&[1; 32]).unwrap(), 123);

        for i in 0..10 {
            map.try_insert([i; 32], i).unwrap();
        } 

        assert_matches!(map.try_insert([10; 32], 123), Err(_));
    }
}