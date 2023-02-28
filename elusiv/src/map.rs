use crate::bytes::*;
use crate::macros::two_pow;
use crate::types::{JITArray, Lazy, LazyField, OrdU256, U256};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::{BorshSerDePlaceholder, BorshSerDeSized, ByteBackedJIT};
use std::cmp::Ordering;
use std::fmt::Debug;

pub trait ElusivMapKey: BorshSerDeSized + Clone + PartialEq + PartialOrd + Ord + Debug {}
pub trait ElusivMapValue: BorshSerDeSized + Clone + Debug {}

/// Implements [`ElusivMapKey`] for a provided type
macro_rules! impl_map_key {
    ($ty: ty) => {
        impl crate::map::ElusivMapKey for $ty {}
    };
}

/// Implements [`ElusivMapValue`] for a provided type
macro_rules! impl_map_value {
    ($ty: ty) => {
        impl crate::map::ElusivMapValue for $ty {}
    };
}

impl_map_key!(());
impl_map_key!(U256);
impl_map_key!(OrdU256);
impl_map_key!(u32);

impl_map_value!(());

#[derive(Debug)]
pub enum ElusivMapError<V: ElusivMapValue> {
    /// Value of a duplciate entry
    Duplicate(V),

    /// Key is larger than max and the map is full
    KeyTooLarge,

    /// Key is not contained in the map
    KeyNotFound,
}

const MID_PTR_HEIGHT: u32 = 3;
const MID_PTR_COUNT: usize = two_pow!(MID_PTR_HEIGHT + 1) - 1;

/// We use pointers to increase read/write efficiency in the [`ElusivMap`]
#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub struct ElusivMapPtr(pub u16);

/// A set storing values of type `K` utilizing [`ElusivMap`]
pub type ElusivSet<'a, K, const CAPACITY: usize> = ElusivMap<'a, K, (), CAPACITY>;

/// Write efficient, append only, JIT deserializing, insertion sorted map with a maximum capacity
///
/// # Note
///
/// The upper bound (inclusive) for `CAPACITY` is `2^16` (size of the pointer).
#[derive(BorshSerDeSized, BorshSerDePlaceholder, ByteBackedJIT)]
#[cfg_attr(test, derive(Debug))]
pub struct ElusivMap<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> {
    len: Lazy<'a, u32>,

    min_ptr: Lazy<'a, ElusivMapPtr>,
    max_ptr: Lazy<'a, ElusivMapPtr>,

    /// All mid-ptrs form a binary tree of height [`MID_PTR_HEIGHT`].
    mid_ptr: JITArray<'a, ElusivMapPtr, MID_PTR_COUNT>,
    mid_ptr_position: JITArray<'a, u32, MID_PTR_COUNT>,

    next: JITArray<'a, ElusivMapPtr, CAPACITY>,
    prev: JITArray<'a, ElusivMapPtr, CAPACITY>,

    keys: JITArray<'a, K, CAPACITY>,
    values: JITArray<'a, V, CAPACITY>,
}

const MAX: u32 = two_pow!(16) as u32;

impl<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> ElusivMap<'a, K, V, CAPACITY> {
    pub const CAPACITY: u32 = {
        assert!(usize_as_u32_safe(CAPACITY) <= MAX);
        usize_as_u32_safe(CAPACITY)
    };

    /// Attempts to insert a new entry into the map
    ///
    /// # Note
    ///
    /// Duplicate keys cannot be inserted.
    ///
    /// # Return
    ///
    /// Returns [`Ok(None)`] if the entry has been inserted successfully.
    ///
    /// Returns [`Ok(Some(max))`] if the entry has been inserted successfully but the map was already full so the maximum entry max is dropped.
    ///
    /// Returns [`Err(_)`] if the entry has not been inserted (due to a duplicate key or the key being too large for the map).
    pub fn try_insert(&mut self, key: K, value: &V) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        match self.binary_search(&key) {
            Ok(index) => self.insert_at(&key, value, index),
            Err(ElusivMapError::KeyTooLarge) => Ok(Some((key, value.clone()))),
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

    /// Searches for the [`ElusivMapPtr`] at which the key can be inserted
    ///
    /// # Return
    ///
    /// Returns [`Ok(index)`] if the key can be inserted in the map at `index`.
    ///
    /// Returns [`Err(ElusivMapError::KeyTooLarge)`] if the key cannot be inserted due to it being too large for the (already full) map.
    ///
    /// Returns [`Err(ElusivMapError::Duplicate(value)`] if the key is already contained in the map with the corresponding value.
    fn binary_search(&mut self, key: &K) -> Result<u32, ElusivMapError<V>> {
        if self.is_empty() {
            return Ok(0);
        }

        match key.cmp(&self.min()) {
            Ordering::Equal => return Err(ElusivMapError::Duplicate(self.min_value())),
            Ordering::Less => return Ok(0),
            _ => {}
        }

        match key.cmp(&self.max()) {
            Ordering::Equal => return Err(ElusivMapError::Duplicate(self.max_value())),
            Ordering::Greater => {
                if self.is_full() {
                    return Err(ElusivMapError::KeyTooLarge);
                }

                return Ok(self.len.get());
            }
            _ => {}
        }

        let mut mid = 0; // initial value is never used
        let mut low = 0;
        let mut high = self.len.get();

        let mut low_ptr = self.min_ptr.get();
        let mut mid_ptr = ElusivMapPtr(0); // initial value is never used

        while low < high {
            mid = low + (high - low) / 2;

            // Compute the `mid_ptr` by moving `mid - low` pointers forward
            mid_ptr = self.get_ptr(&low_ptr, low, mid - low);

            match key.cmp(&self.key(&mid_ptr)) {
                Ordering::Less => {
                    high = mid;
                }
                Ordering::Greater => {
                    low = mid + 1;
                    low_ptr = self.get_next(&mid_ptr);
                }
                Ordering::Equal => {
                    return Err(ElusivMapError::Duplicate(self.values.get(mid as usize)))
                }
            }
        }

        if *key > self.key(&mid_ptr) {
            mid += 1;
        }

        Ok(mid)
    }

    fn insert_at(
        &mut self,
        key: &K,
        value: &V,
        index: u32,
    ) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        assert!(index <= self.len.get());

        let max_key = self.max();
        let max_value = self.max_value();
        let is_full = self.is_full();
        let max_ptr_predecessor = {
            let max_ptr = self.max_ptr.get();
            self.get_prev(&max_ptr)
        };

        let new_ptr = if !is_full {
            // We fill the underlying data linearly
            ElusivMapPtr(self.len.get().try_into().unwrap())
        } else {
            // Once it's full, we will just override the maximum value (since this value will always be dropped)
            self.max_ptr.get()
        };

        self.set(&new_ptr, key, value);

        if index == 0 {
            // Prepend
            let ptr = self.min_ptr.get();
            self.link_ptrs(&new_ptr, &ptr);
            self.min_ptr.set(&new_ptr);
        } else if index == self.len.get() {
            // Append
            let ptr = self.max_ptr.get();
            self.link_ptrs(&ptr, &new_ptr);
            self.max_ptr.set(&new_ptr);
        } else {
            // Insert at index
            let min_ptr = self.min_ptr.get();
            let prev = self.get_ptr(&min_ptr, 0, index - 1);
            let next = self.get_next(&prev);

            // Insert `new_ptr` between `prev` and `next`
            self.link_ptrs(&prev, &new_ptr);
            self.link_ptrs(&new_ptr, &next);
        }

        self.update_mid_ptrs(index);

        if is_full {
            // Update `max_ptr`
            let prev = if index == self.len.get() {
                new_ptr
            } else {
                max_ptr_predecessor
            };

            self.max_ptr.set(&prev);

            // Return the previous max key and value
            return Ok(Some((max_key, max_value)));
        }

        let new_len = self.len.get() + u32::from(!is_full);
        self.len.set(&new_len);

        Ok(None)
    }

    fn mid_ptr_index(local_mid_ptr_index: usize, mid_ptr_depth: usize) -> usize {
        // Conversion from local index and tree depth (root has depth 0):
        // `i * 2^(h-d+1) + 2^(h-d) - 1` (with height `h`, local-index `i`, depth `d`)
        let e = two_pow!(MID_PTR_HEIGHT - mid_ptr_depth as u32);
        local_mid_ptr_index * e * 2 + e - 1
    }

    #[allow(clippy::too_many_arguments)]
    fn update_mid_ptrs(&mut self, insertion_index: u32) {
        let len = self.len.get();
        let is_full = self.is_full();
        let new_len = len + u32::from(!self.is_full());

        for i in 0..=MID_PTR_HEIGHT {
            let c = two_pow!(i) as u32;
            let c2 = c * 2;

            for j in 0..c {
                let mid_ptr_index = Self::mid_ptr_index(j as usize, i as usize);
                let mid = self.mid_ptr_position.get(mid_ptr_index);
                let new_mid = ((1 + 2 * j) * new_len) / c2;

                if mid != new_mid {
                    self.mid_ptr_position.set(mid_ptr_index, &new_mid);
                }

                if insertion_index <= mid {
                    if mid == new_mid || is_full {
                        // Adjust by decreasing by one ptr
                        let mid_ptr = self.mid_ptr.get(mid_ptr_index);
                        let next_mid_ptr = self.get_prev(&mid_ptr);
                        self.mid_ptr.set(mid_ptr_index, &next_mid_ptr);
                    }
                } else if mid != new_mid {
                    // Adjust by increasing by one ptr
                    let mid_ptr = self.mid_ptr.get(mid_ptr_index);
                    let next_mid_ptr = self.get_next(&mid_ptr);
                    self.mid_ptr.set(mid_ptr_index, &next_mid_ptr);
                }
            }
        }
    }

    /// Traverses the pointer graph and returns the pointer with a distance of `offset` from the `base_ptr`
    fn get_ptr(
        &mut self,
        base_ptr: &ElusivMapPtr,
        base_ptr_offset: u32,
        offset: u32,
    ) -> ElusivMapPtr {
        let distance = self.len.get() / MID_PTR_COUNT as u32;
        let half_distance = distance / 2;

        if offset <= half_distance || distance == 0 {
            self.get_next_ptr(base_ptr, offset)
        } else {
            let index = base_ptr_offset + offset;
            let mut step = MID_PTR_COUNT / 2;
            let mut mid_ptr_index = step;
            for i in 1..=MID_PTR_HEIGHT {
                let mid = self.mid_ptr_position.get(mid_ptr_index);
                step = two_pow!(MID_PTR_HEIGHT - i);

                match index.cmp(&mid) {
                    Ordering::Less => {
                        mid_ptr_index -= step;
                    }
                    Ordering::Equal => return self.mid_ptr.get(mid_ptr_index),
                    Ordering::Greater => {
                        mid_ptr_index += step;
                    }
                }
            }

            let mid = self.mid_ptr_position.get(mid_ptr_index);
            match index.cmp(&mid) {
                Ordering::Less => {
                    /*let len = if mid_ptr_index == 0 {
                        mid
                    } else {
                        mid - self.mid_ptr_position.get(mid_ptr_index - 1)
                    };*/
                    let x = if mid_ptr_index == 0 {
                        0
                    } else {
                        self.mid_ptr_position.get(mid_ptr_index - 1)
                    };

                    let ptr = if mid_ptr_index == 0 {
                        self.min_ptr.get()
                    } else {
                        self.mid_ptr.get(mid_ptr_index - 1)
                    };

                    self.get_next_ptr(&ptr, index - x)
                }
                Ordering::Equal => self.mid_ptr.get(mid_ptr_index),
                Ordering::Greater => {
                    /*let len = if mid_ptr_index == MID_PTR_COUNT {
                        self.len.get() - mid
                    } else {
                        self.mid_ptr_position.get(mid_ptr_index + 1) - mid
                    };*/

                    let ptr = self.mid_ptr.get(mid_ptr_index);
                    self.get_next_ptr(&ptr, index - mid)
                }
            }
        }
    }

    /*
    fn get_prev_ptr(&mut self, base_ptr: &ElusivMapPtr, offset: u32) -> ElusivMapPtr {
        let mut ptr = *base_ptr;
        for _ in 0..offset {
            ptr = self.prev.get(ptr.0 as usize);
        }
        ptr
    }
    */

    fn get_next_ptr(&mut self, base_ptr: &ElusivMapPtr, offset: u32) -> ElusivMapPtr {
        let mut ptr = *base_ptr;
        for _ in 0..offset {
            ptr = self.next.get(ptr.0 as usize);
        }
        ptr
    }

    pub fn key(&mut self, ptr: &ElusivMapPtr) -> K {
        self.keys.get(ptr.0 as usize)
    }

    pub fn value(&mut self, ptr: &ElusivMapPtr) -> V {
        self.values.get(ptr.0 as usize)
    }

    #[inline]
    fn set(&mut self, ptr: &ElusivMapPtr, key: &K, value: &V) {
        self.keys.set(ptr.0 as usize, key);
        self.values.set(ptr.0 as usize, value);
    }

    #[inline]
    fn get_prev(&mut self, ptr: &ElusivMapPtr) -> ElusivMapPtr {
        self.prev.get(ptr.0 as usize)
    }

    #[inline]
    fn get_next(&mut self, ptr: &ElusivMapPtr) -> ElusivMapPtr {
        self.next.get(ptr.0 as usize)
    }

    fn link_ptrs(&mut self, a: &ElusivMapPtr, b: &ElusivMapPtr) {
        self.next.set(a.0 as usize, b);
        self.prev.set(b.0 as usize, a);
    }

    pub fn min(&mut self) -> K {
        let ptr = self.min_ptr.get();
        self.key(&ptr)
    }

    fn min_value(&mut self) -> V {
        let ptr = self.min_ptr.get();
        self.value(&ptr)
    }

    pub fn max(&mut self) -> K {
        let ptr = self.max_ptr.get();
        self.key(&ptr)
    }

    fn max_value(&mut self) -> V {
        let ptr = self.max_ptr.get();
        self.value(&ptr)
    }

    pub fn is_empty(&mut self) -> bool {
        self.len.get() == 0
    }

    pub fn is_full(&mut self) -> bool {
        self.len.get() as usize == CAPACITY
    }

    pub fn reset(&mut self) {
        self.len.set(&0);
        self.max_ptr.set(&ElusivMapPtr(0));

        // The first ptr points to itself
        self.next.set(0, &ElusivMapPtr(0));
        self.prev.set(0, &ElusivMapPtr(0));
    }

    #[cfg(test)]
    fn mid(&mut self) -> K {
        let mid_ptr = self.mid_ptr.get(MID_PTR_COUNT / 2);
        // let mid_ptr = self.mid_ptr.get();
        self.keys.get(mid_ptr.0 as usize)
    }

    #[cfg(test)]
    pub fn sorted_keys(&mut self) -> Vec<K> {
        let mut k = Vec::with_capacity(self.len.get() as usize);
        let mut ptr = self.min_ptr.get();
        for _ in 0..self.len.get() {
            k.push(self.key(&ptr));
            ptr = self.get_next(&ptr);
        }

        k
    }

    #[cfg(test)]
    pub fn values_sorted_by_keys(&mut self) -> Vec<V> {
        let mut v = Vec::with_capacity(self.len.get() as usize);
        let mut ptr = self.min_ptr.get();
        for _ in 0..self.len.get() {
            v.push(self.value(&ptr));
            ptr = self.get_next(&ptr);
        }

        v
    }
}

impl<'a, K: ElusivMapKey, V: ElusivMapValue + Default, const CAPACITY: usize>
    ElusivMap<'a, K, V, CAPACITY>
{
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

#[cfg(test)]
/// Computes all `v.len()!` permutations
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
    fn test_link_ptrs() {
        map!(map);

        map.link_ptrs(&ElusivMapPtr(0), &ElusivMapPtr(1));

        assert_eq!(map.next.get(0).0, 1);
        assert_eq!(map.prev.get(1).0, 0);
    }

    #[test]
    fn test_mid_ptr_index() {
        assert_eq!(Map::mid_ptr_index(0, MID_PTR_HEIGHT as usize), 0);
        assert_eq!(
            Map::mid_ptr_index(MID_PTR_COUNT / 2, MID_PTR_HEIGHT as usize),
            MID_PTR_COUNT - 1
        );
        assert_eq!(Map::mid_ptr_index(0, 0), MID_PTR_COUNT / 2);
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
            assert_eq!(
                map.values_sorted_by_keys(),
                sorted.iter().map(|v| v + 100).collect::<Vec<u16>>()
            );
        }
    }

    #[test]
    fn test_try_insert_mid_even_len() {
        type Map<'a> = ElusivMap<'a, u16, u16, 4>;
        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data);

        map.try_insert_default(0).unwrap();
        map.try_insert_default(1).unwrap();
        map.try_insert_default(10).unwrap();
        map.try_insert_default(11).unwrap();

        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 10);
        assert_eq!(map.max(), 11);

        map.try_insert_default(5).unwrap();
        println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 5);
        assert_eq!(map.max(), 10);

        map.try_insert_default(4).unwrap();
        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 4);
        assert_eq!(map.max(), 5);
    }

    #[test]
    fn test_try_insert_mid_uneven_len() {
        type Map<'a> = ElusivMap<'a, u16, u16, 5>;

        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data);

        map.try_insert_default(0).unwrap();
        map.try_insert_default(1).unwrap();
        map.try_insert_default(10).unwrap();
        map.try_insert_default(11).unwrap();
        map.try_insert_default(12).unwrap();

        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 10);
        assert_eq!(map.max(), 12);

        map.try_insert_default(5).unwrap();
        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 5);
        assert_eq!(map.max(), 11);

        map.try_insert_default(4).unwrap();
        assert_eq!(map.min(), 0);
        assert_eq!(map.mid(), 4);
        assert_eq!(map.max(), 10);
    }

    #[test]
    fn test_try_insert_rev() {
        type Map<'a> = ElusivMap<'a, u32, (), 10000>;
        let mut data = vec![0; Map::SIZE];
        let mut map = Map::new(&mut data);

        for i in (0..Map::CAPACITY).rev() {
            map.try_insert_default(i).unwrap();
        }

        assert_eq!(map.sorted_keys(), (0..Map::CAPACITY).collect::<Vec<_>>());
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
        // println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 7);
        assert_eq!(map.max(), 13);

        assert_eq!(map.try_insert_default(6).unwrap().unwrap().0, 13);
        // println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 6);
        assert_eq!(map.max(), 12);

        assert_eq!(map.try_insert_default(5).unwrap().unwrap().0, 12);
        assert_eq!(map.min(), 5);
        assert_eq!(map.max(), 11);

        // Insert and min
        map!(map);
        map.try_insert_default(7).unwrap();
        assert_eq!(map.min(), 7);
        assert_eq!(map.mid(), 7);
        assert_eq!(map.max(), 7);

        map.try_insert_default(8).unwrap();
        assert_eq!(map.min(), 7);
        assert_eq!(map.mid(), 8);
        assert_eq!(map.max(), 8);

        map.try_insert_default(5).unwrap();
        assert_eq!(map.min(), 5);
        assert_eq!(map.mid(), 7);
        assert_eq!(map.max(), 8);

        map.try_insert_default(1).unwrap();
        assert_eq!(map.min(), 1);
        assert_eq!(map.mid(), 7);
        assert_eq!(map.max(), 8);

        map.try_insert_default(6).unwrap();
        map.try_insert_default(2).unwrap();
        map.try_insert_default(9).unwrap();
        assert_eq!(map.min(), 1);
        assert_eq!(map.mid(), 6);
        assert_eq!(map.max(), 9);
        assert!(map.is_full());

        assert_eq!(map.try_insert_default(3).unwrap().unwrap().0, 9);
        println!("{:?}", map.sorted_keys());
        assert_eq!(map.min(), 1);
        assert_eq!(map.mid(), 5);
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
        assert_eq!(map.mid(), m / 2 + 1);
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
