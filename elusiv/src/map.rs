use elusiv_derive::{BorshSerDeSized, BorshSerDePlaceholder, ByteBackedJIT};
use crate::macros::two_pow;
use crate::types::{LazyField, Lazy, JITArray, U256, OrdU256};
use crate::bytes::*;
use std::cmp::Ordering;
use std::fmt::Debug;

pub trait ElusivMapKey: BorshSerDeSized + Clone + PartialEq + PartialOrd + Ord + Debug {}
pub trait ElusivMapValue: BorshSerDeSized + Clone + Debug {}

macro_rules! impl_map_key {
    ($ty: ty) => { impl crate::map::ElusivMapKey for $ty {} };
}

macro_rules! impl_map_value {
    ($ty: ty) => { impl crate::map::ElusivMapValue for $ty {} };
}

impl_map_key!(());
impl_map_key!(U256);
impl_map_key!(OrdU256);
impl_map_key!(u32);
impl_map_value!(());

pub type ElusivSet<'a, K, const CAPACITY: usize> = ElusivMap<'a, K, (), CAPACITY>;

#[derive(BorshSerDeSized, BorshSerDePlaceholder, ByteBackedJIT, Debug)]
/// Write efficient, append only, JIT deserializing, insertion sorted map with a maximum capacity
/// - upper bound (inclusive) for `CAPACITY` is `2^16`
pub struct ElusivMap<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> {
    len: Lazy<'a, u32>,

    min_ptr: Lazy<'a, u16>,
    max_ptr: Lazy<'a, u16>,
    // TODO: switch to a multi-mid-ptr system (log N pointers) to drastically increase efficiency
    //mid_ptr: Lazy<'a, u16>,

    /// The map is represented as a circular, singly linked list
    /// - this means: `keys.get(next(max_ptr.get()))` is the minimum key
    next: JITArray<'a, u16, CAPACITY>,

    keys: JITArray<'a, K, CAPACITY>,
    values: JITArray<'a, V, CAPACITY>,
}

const MAX: u32 = two_pow!(16) as u32;
const fn verify_capacity(c: u32) -> u32 {
    if c > MAX { panic!() }
    c
}

impl<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> ElusivMap<'a, K, V, CAPACITY> {
    pub const CAPACITY: u32 = verify_capacity(usize_as_u32_safe(CAPACITY));

    /// Attempts to insert a new entry into the map
    /// - duplicate keys cannot be inserted
    /// 
    /// - `Ok(None)`: the entry has been inserted
    /// - `Ok(Some(max))`: the entry has been inserted but the map is full so the maximum entry max is dropped
    /// - `Err(_)`: the entry has not been inserted (due to a duplicate key)
    pub fn try_insert(&mut self, key: K, value: &V) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        match self.binary_search(&key) {
            Ok(pointer) => self.insert_at(&key, value, pointer),
            Err(ElusivMapError::KeyTooLarge) => { Ok(Some((key, value.clone()))) }
            Err(e) => Err(e),
        }
    }

    #[cfg(test)]
    pub fn insert_multiple(&mut self, entries: &[(K, V)]) {
        for (key, value) in entries {
            self.try_insert(key.clone(), value).unwrap();
        }
    }

    /// Returns the value if a key is contained in the map
    pub fn contains(&mut self, key: &K) -> Option<V> {
        match self.binary_search(key) {
            Err(ElusivMapError::Duplicate(v)) => Some(v),
            _ => None,
        }
    }

    fn binary_search(&mut self, key: &K) -> Result<u16, ElusivMapError<V>> {
        if self.is_empty() {
            return Ok(0)
        }

        match key.cmp(&self.min()) {
            Ordering::Equal => {
                return Err(ElusivMapError::Duplicate(self.min_value()))
            }
            Ordering::Less => {
                return Ok(0)
            }
            _ => {}
        }

        match key.cmp(&self.max()) {
            Ordering::Equal => {
                return Err(ElusivMapError::Duplicate(self.max_value()))
            }
            Ordering::Greater => {
                if self.is_full() {
                    return Err(ElusivMapError::KeyTooLarge)
                }
                return Ok(self.len.get() as u16)
            }
            _ => {}
        }

        let mut mid = 0;
        let mut l = 0;
        let mut h = self.len.get();

        let mut low_ptr = self.min_ptr.get();
        let mut mid_ptr = 0;

        // 130s -> 24.35s -> 20
        // 8s -> 6
        while l < h {
            mid = l + (h - l) / 2;
            mid_ptr = self.get_next_ptr_fast(low_ptr, mid - l);
            match key.cmp(&self.keys.get(mid_ptr as usize)) {
                Ordering::Less => {
                    h = mid;
                }
                Ordering::Greater => {
                    l = mid + 1;
                    low_ptr = self.get_next_ptr_fast(mid_ptr, 1);
                }
                Ordering::Equal => {
                    return Err(ElusivMapError::Duplicate(self.values.get(mid as usize)))
                }
            }
        }

        if *key > self.keys.get(mid_ptr as usize) {
            mid += 1;
        }

        Ok(mid as u16)
    }

    fn insert_at(
        &mut self,
        key: &K,
        value: &V,
        index: u16,
    ) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        let max_key= self.max();
        let max_value = self.values.get(self.max_ptr.get() as usize);

        let new_ptr= if self.is_full() {
            self.max_ptr.get()
        } else {
            self.len.get().try_into().unwrap()
        };

        self.keys.set(new_ptr as usize, key);
        self.values.set(new_ptr as usize, value);

        if index == 0 {   // Prepend
            self.next.set(new_ptr as usize, &self.min_ptr.get());
            self.min_ptr.set(&new_ptr);
        } else if index == self.len.get() as u16 {  // Append
            self.next.set(self.max_ptr.get() as usize, &new_ptr);
            self.max_ptr.set(&new_ptr);
        } else {    // Insert at index
            let min_ptr = self.min_ptr.get();
            let prev = self.get_next_ptr_fast(min_ptr, index as u32 - 1);
            let next = self.next.get(prev as usize);
            self.next.set(prev as usize, &new_ptr);
            self.next.set(new_ptr as usize, &next);
        }

        let len = self.len.get();
        let next_len = len + if self.is_full() { 0 } else { 1 };

        /*if (index as u32) < len / 2 { // Move mid to the left
            let min_ptr = self.min_ptr.get();
            let mid_ptr = self.get_next_ptr(min_ptr, len / 2 - 1);
            self.mid_ptr.set(&mid_ptr);
        }
        
        if next_len % 2 == 0 {   // Every two insertions move mid to the right
            let mid_ptr = self.mid_ptr.get();
            self.mid_ptr.set(&self.next.get(mid_ptr as usize));
        }*/

        if self.is_full() {
            let len = self.len.get() - 1;
            let min_ptr = self.min_ptr.get();
            let prev = self.get_next_ptr_fast(min_ptr, len);
            self.max_ptr.set(&prev);
            return Ok(Some((max_key, max_value)))
        }

        self.len.set(&next_len);

        Ok(None)
    }

    fn get_next_ptr(&mut self, base_ptr: u16, offset: u32) -> u16 {
        let mut ptr = base_ptr;
        for _ in 0..offset {
            ptr = self.next.get(ptr as usize);
        }
        ptr
    }

    fn get_next_ptr_fast(&mut self, base_ptr: u16, offset: u32) -> u16 {
        /*let half = self.len.get() / 2;
        if offset >= half {
            let mid_ptr = self.mid_ptr.get();
            return self.get_next_ptr_fast(mid_ptr, offset - half)
        }*/

        self.get_next_ptr(base_ptr, offset)
    }

    pub fn min(&mut self) -> K {
        self.keys.get(self.min_ptr.get() as usize)
    }

    fn min_value(&mut self) -> V {
        self.values.get(self.min_ptr.get() as usize)
    }

    pub fn max(&mut self) -> K {
        self.keys.get(self.max_ptr.get() as usize)
    }

    fn max_value(&mut self) -> V {
        self.values.get(self.max_ptr.get() as usize)
    }

    pub fn is_empty(&mut self) -> bool {
        self.len.get() == 0
    }

    pub fn is_full(&mut self) -> bool {
        self.len.get() as usize == CAPACITY
    }

    pub fn reset(&mut self) {
        self.len.set(&0);
        self.max_ptr.set(&0);
        self.next.set(0, &0);
    }

    #[cfg(test)]
    pub fn sorted_keys(&mut self) -> Vec<K> {
        let mut k = Vec::new();
        let mut ptr = self.min_ptr.get() as usize;
        for _ in 0..self.len.get() {
            k.push(self.keys.get(ptr));
            ptr = self.next.get(ptr) as usize;
        }

        k
    }

    #[cfg(test)]
    pub fn values_sorted_by_keys(&mut self) -> Vec<V> {
        let mut v = Vec::new();
        let mut ptr = self.min_ptr.get() as usize;
        for _ in 0..self.len.get() {
            v.push(self.values.get(ptr));
            ptr = self.next.get(ptr) as usize;
        }

        v
    }

    #[cfg(test)]
    pub fn fake_min_max_len(&mut self, min: (K, V), max: (K, V), len: u32) {
        self.reset();

        self.keys.set(0, &min.0);
        self.values.set(0, &min.1);

        self.keys.set(1, &max.0);
        self.values.set(1, &max.1);

        self.min_ptr.set(&0);
        self.max_ptr.set(&1);

        self.len.set(&len);
    }
}

impl<'a, K: ElusivMapKey, V: ElusivMapValue + Default, const CAPACITY: usize> ElusivMap<'a, K, V, CAPACITY> {
    pub fn try_insert_default(&mut self, key: K) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        self.try_insert(key, &V::default())
    }

    #[cfg(test)]
    pub fn insert_multiple_default(&mut self, keys: &[K]) {
        for key in keys {
            self.try_insert_default(key.clone()).unwrap();
        }
    }
}

#[derive(Debug)]
pub enum ElusivMapError<V: ElusivMapValue> {
    /// Value of a duplciate entry
    Duplicate(V),

    /// Key is larger than max and the map is full
    KeyTooLarge,

    /// Key is not contained in the map
    KeyNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    impl_map_key!(u16);
    impl_map_value!(u16);

    type Map<'a> = ElusivMap<'a, u16, u16, 7>;

    macro_rules! map {
        ($id: ident) => {
            let mut data = vec![0; Map::SIZE];
            let mut $id = Map::new(&mut data);
        };
    }

    #[test]
    fn test_try_insert() {
        type Map<'a> = ElusivMap<'a, u16, u16, 8>;

        for permutation in permute(&(0..8).collect::<Vec<u16>>()) {
            let mut sorted = permutation.clone();
            sorted.sort_unstable();

            let mut data = vec![0; Map::SIZE];
            let mut map = Map::new(&mut data);

            for v in permutation.clone() {
                map.try_insert(v, &(v + 100)).unwrap();
            }

            for v in permutation.clone() {
                map.contains(&v).unwrap();
            }

            assert_eq!(map.sorted_keys(), sorted);
            assert_eq!(map.min(), *sorted.first().unwrap());
            assert_eq!(map.max(), *sorted.last().unwrap());
            assert_eq!(map.values_sorted_by_keys(), sorted.iter().map(|v| v + 100).collect::<Vec<u16>>());
        }
    }

    #[test]
    fn test_try_insert_rev() {
        type Map<'a> = ElusivMap<'a, u16, u16, 10000>;
        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data);

        for i in (0..10000).rev() {
            map.try_insert_default(i).unwrap();
        }

        assert_eq!(map.sorted_keys(), (0..10000).collect::<Vec<u16>>());
    }

    #[test]
    fn test_try_insert_too_large() {
        map!(map);

        map.insert_multiple(&(0..7).zip(0..7).collect::<Vec<(u16, u16)>>());
        assert_eq!(map.sorted_keys(), (0..7).collect::<Vec<u16>>());
        assert!(map.is_full());

        assert_eq!(map.try_insert(7, &8).unwrap().unwrap(), (7, 8));
        assert_eq!(map.try_insert(8, &9).unwrap().unwrap(), (8, 9));
    }

    #[test]
    #[allow(unused_variables)]
    fn test_try_insert_duplicate() {
        map!(map);

        map.insert_multiple(&(1..=7).zip(8..=14).collect::<Vec<(u16, u16)>>());
        for i in 0..7u16 {
            let k = i + 1;
            let v = i + 8;
            assert_matches!(map.try_insert_default(k), Err(ElusivMapError::Duplicate(v)));
        }
    }

    #[test]
    fn test_try_insert_drop_last() {
        // Prepend new mins
        map!(map);
        map.insert_multiple_default(&(7..14).rev().collect::<Vec<u16>>());
        println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 7);
        assert_eq!(map.max(), 13);

        assert_eq!(map.try_insert_default(6).unwrap().unwrap().0, 13);
        println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 6);
        assert_eq!(map.max(), 12);

        assert_eq!(map.try_insert_default(5).unwrap().unwrap().0, 12);
        assert_eq!(map.min(), 5);
        assert_eq!(map.max(), 11);

        // Insert and min
        map!(map);
        map.try_insert_default(7).unwrap();
        map.try_insert_default(8).unwrap();
        map.try_insert_default(5).unwrap();
        map.try_insert_default(1).unwrap();
        map.try_insert_default(6).unwrap();
        map.try_insert_default(2).unwrap();
        map.try_insert_default(9).unwrap();
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), 9);

        assert_eq!(map.try_insert_default(3).unwrap().unwrap().0, 9);
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), 8);

        assert_eq!(map.try_insert_default(4).unwrap().unwrap().0, 8);
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), 7);

        assert_eq!(map.try_insert_default(0).unwrap().unwrap().0, 7);
        assert_eq!(map.min(), 0);
        assert_eq!(map.max(), 6);
    }

    #[test]
    fn test_contains() {
        map!(map);

        assert!(map.contains(&0).is_none());
        map.try_insert_default(0).unwrap();
        assert!(map.contains(&0).is_some());

        map.try_insert(1, &1).unwrap();
        assert!(map.contains(&1).is_some());

        map.try_insert(8, &6).unwrap();
        assert!(map.contains(&8).is_some());
    }

    #[test]
    fn test_reset() {
        map!(map);

        map.try_insert_default(0).unwrap();
        map.try_insert_default(1).unwrap();
        map.try_insert_default(2).unwrap();

        map.reset();

        assert!(map.is_empty());
        assert_eq!(map.len.get(), 0);

        map.try_insert_default(0).unwrap();
        map.try_insert_default(1).unwrap();
        map.try_insert_default(2).unwrap();

        assert_eq!(map.sorted_keys(), [0, 1, 2]);
    }

    const M: usize = MAX as usize;

    #[test]
    fn test_map_max_size() {
        type Map<'a> = ElusivMap<'a, u32, (), M>;
        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data);
        let m = M as u32;

        for i in (1..=m).rev() {
            map.try_insert_default(i).unwrap();
        }

        assert!(map.is_full());
        assert_eq!(map.len.get(), two_pow!(16) as u32);

        assert_eq!(map.try_insert_default(m + 1).unwrap().unwrap().0, m + 1);
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), m);

        assert_eq!(map.try_insert_default(0).unwrap().unwrap().0, m);
        assert_eq!(map.min(), 0);
        assert_eq!(map.max(), m - 1);

        // Tests correct serialization
        let mut map = Map::new(&mut data);
        assert_eq!(map.min(), 0);
        assert_eq!(map.max(), m - 1);
        for i in 1..=m {
            map.contains(&i);
        }
    }
}

#[cfg(test)]
/// Computes all v.len()! permutations
pub fn permute<T: Clone + Sized>(v: &[T]) -> Vec<Vec<T>> {
    fn permute<T: Clone>(values: &mut Vec<T>, l: usize) -> Vec<Vec<T>> {
        let mut v = Vec::new();
        if l <= 1 {
            v.push(values.clone());
        } else {
            v.append(&mut permute(values, l - 1));
            for i in 0..(l - 1) {
                if l % 2 == 0 {
                    values.swap(i, l - 1);
                } else {
                    values.swap(0, l - 1);
                }
                v.append(&mut permute(values, l - 1));
            }
        }
        v
    }

    permute(&mut v.to_vec(), v.len())
}