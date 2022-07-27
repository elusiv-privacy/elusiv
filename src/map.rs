use elusiv_derive::{BorshSerDeSized, BorshSerDePlaceholder, ByteBackedJIT};
use crate::types::{LazyField, Lazy, JITArray, U256};
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

impl_map_value!(());
impl_map_key!(U256);

pub type ElusivSet<'a, K, const CAPACITY: usize> = ElusivMap<'a, K, (), CAPACITY>;

#[derive(BorshSerDeSized, BorshSerDePlaceholder, ByteBackedJIT, Debug)]
/// Write efficient, append only, JIT deserializing, insertion sorted map with a maximum capacity
/// - upper bound for `CAPACITY` is `u16::MAX`
/// - containment check: `O(log CAPACITY)`
/// - minimum/maximum key insertion: `O(1)` for search and write
/// - other value insertion: `O(log CAPACITY)` for search, `O(1)` for write
pub struct ElusivMap<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> {
    len: Lazy<'a, u16>,

    /// Points to the maximum key (entry) stored in the map
    max_ptr: Lazy<'a, u16>,

    /// The map is represented as a circular, singly linked list
    /// - this means: `keys.get(next(max_ptr.get()))` is the minimum key
    next: JITArray<'a, u16, CAPACITY>,

    keys: JITArray<'a, K, CAPACITY>,
    values: JITArray<'a, V, CAPACITY>,
}

impl<'a, K: ElusivMapKey, V: ElusivMapValue, const CAPACITY: usize> ElusivMap<'a, K, V, CAPACITY> {
    pub const CAPACITY: u16 = usize_as_u16_safe(CAPACITY);

    /// Attempts to insert a new entry into the map
    /// - duplicate keys cannot be inserted
    /// 
    /// - `Ok(None)`: the entry has been inserted
    /// - `Ok(Some(max))`: the entry has been inserted but the map is full so the maximum entry max is dropped
    /// - `Err(_)`: the entry has not been inserted (due to a duplicate key)
    pub fn try_insert(&mut self, key: K, value: &V) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        match self.binary_search(&key) {
            Ok(pointer) => self.insert_entry_after(&key, value, pointer),
            Err(ElusivMapError::KeyTooLarge) => { Ok(Some((key, value.clone()))) }
            Err(e) => Err(e),
        }
    }

    /// Inserts a key-value-pair if the key does not already exist or updates an existing pair's value
    /// - this insertion will always succeed 
    /// - returns `(dropped, old)`
    ///     - `dropped`: the dropped entry in case of the map's capacity being reached,
    ///     - `old`: the previous value for the entry
    pub fn insert_update(&mut self, key: K, value: &V) -> (Option<(K, V)>, Option<V>) {
        match self.try_insert(key, value) {
            Ok(v) => (v, None),
            Err(ElusivMapError::Duplicate(pointer, v)) => {
                self.values.set(pointer as usize, value);
                (None, Some(v))
            }
            _ => panic!()   // branch should never be reached
        }
    }

    pub fn contains(&mut self, key: &K) -> Option<V> {
        match self.binary_search(key) {
            Err(ElusivMapError::Duplicate(_, v)) => Some(v),
            _ => None,
        }
    }

    /// Finds the pointer after which an entry can be inserted 
    /// - insertion at `next(pointer)`
    #[allow(clippy::comparison_chain)]
    fn binary_search(&mut self, key: &K) -> Result<u16, ElusivMapError<V>> {
        if self.is_empty() {
            return Ok(0)
        }

        let mut len = self.len.get() as usize;
        let mut l = self.min_ptr();
        let mut prev_mid = l;

        let min = self.min();
        if *key == min {
            let min = self.min_ptr();
            return Err(
                ElusivMapError::Duplicate(
                    min,
                    self.values.get(min as usize)
                )
            )
        } else if *key < min {
            // not index zero since we insert after the max value
            return Ok(self.max_ptr.get())
        }

        let max = self.keys.get(self.max_ptr.get() as usize);
        if *key == max {
            let max = self.max_ptr.get();
            return Err(
                ElusivMapError::Duplicate(
                    max,
                    self.values.get(max as usize)
                )
            )
        } else if *key > max {
            return Ok(self.max_ptr.get())
        }
    
        loop {
            len /= 2;
            let mid = self.get_mid(l, len);
            let mid_key = self.keys.get(mid as usize);
            match key.cmp(&mid_key) {
                Ordering::Equal => {
                    return Err(
                        ElusivMapError::Duplicate(
                            mid,
                            self.values.get(mid as usize)
                        )
                    )
                }
                Ordering::Less => {
                    if len == 0 {
                        return Ok(prev_mid)
                    }
                }
                Ordering::Greater => {
                    if len == 0 {
                        return Ok(mid as u16)
                    }
                    l = self.next.get(mid as usize);
                    len -= 1;
                }
            }
            prev_mid = mid;
        }
    }

    /// Inserts an entry after a given pointer
    fn insert_entry_after(&mut self, key: &K, value: &V, pointer: u16) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        let new_index = if self.is_full() { self.max_ptr.get() } else { self.len.get() } as usize;
        let next = self.next.get(pointer as usize);

        let min_ptr = self.min_ptr();
        let min = self.min();
        let max = self.max();
        let max_value = self.values.get(self.max_ptr.get() as usize);

        self.next.set(pointer as usize, &(new_index as u16));
        self.next.set(new_index, &next);

        if *key < min { // New min
            if self.is_full() {
                let new_max = self.get_mid(min_ptr, CAPACITY - 2);
                self.max_ptr.set_serialize(&new_max);
                self.next.set(pointer as usize, &min_ptr);
            } else {
                self.next.set(self.max_ptr.get() as usize, &(new_index as u16));
            }
        } else if pointer == self.max_ptr.get() { // New max
            self.max_ptr.set_serialize(&(new_index as u16));
            let min_ptr = self.min_ptr();
            self.next.set(new_index, &min_ptr);
        } else if self.is_full() {
            let new_max = self.get_mid(min_ptr, CAPACITY - 1);
            self.max_ptr.set_serialize(&new_max);
            self.next.set(self.max_ptr.get() as usize, &min_ptr);
        }

        self.keys.set(new_index, key);
        self.values.set(new_index, value);

        if self.is_full() {
            return Ok(Some((max, max_value)))
        }

        let len = self.len.get() + 1;
        self.len.set_serialize(&len);

        Ok(None)
    }

    fn get_mid(&mut self, l: u16, len: usize) -> u16 {
        let mut ptr = l;
        for _ in 0..len {
            ptr = self.next.get(ptr as usize);
        }
        ptr
    }

    fn min_ptr(&mut self) -> u16 {
        self.next.get(self.max_ptr.get() as usize)
    }

    fn min(&mut self) -> K {
        let min_ptr = self.min_ptr() as usize;
        self.keys.get(min_ptr)
    }

    fn max(&mut self) -> K {
        self.keys.get(self.max_ptr.get() as usize)
    }

    pub fn is_empty(&mut self) -> bool {
        self.len.get() == 0
    }

    pub fn is_full(&mut self) -> bool {
        self.len.get() as usize == CAPACITY
    }

    pub fn sorted_keys(&mut self) -> Vec<K> {
        let mut k = Vec::new();
        let mut ptr = self.min_ptr() as usize;
        for _ in 0..self.len.get() {
            k.push(self.keys.get(ptr));
            ptr = self.next.get(ptr) as usize;
        }

        if cfg!(test) {
            assert_eq!(ptr, self.min_ptr() as usize);
        }

        k
    }
}

impl<'a, K: ElusivMapKey, V: ElusivMapValue + Default, const CAPACITY: usize> ElusivMap<'a, K, V, CAPACITY> {
    pub fn try_insert_default(&mut self, key: K) -> Result<Option<(K, V)>, ElusivMapError<V>> {
        self.try_insert(key, &V::default())
    }
}

#[derive(Debug)]
pub enum ElusivMapError<V: ElusivMapValue> {
    // Points to the element after which a value can be inserted
    /// Position and value of a duplciate entry
    Duplicate(u16, V),

    /// Key is larger than max and the map is full
    KeyTooLarge,

    /// Key is not contained in the map
    KeyNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        map!(map);

        map.try_insert(2, &2).unwrap();
        map.try_insert(7, &7).unwrap();
        map.try_insert(4, &4).unwrap();
        map.try_insert(3, &3).unwrap();
        map.try_insert(5, &5).unwrap();
        map.try_insert(1, &1).unwrap();
        map.try_insert(8, &6).unwrap();

        assert_eq!(map.sorted_keys(), vec![1, 2, 3, 4, 5, 7, 8]);
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), 8);

        // Should drop the 8
        map.try_insert(6, &6).unwrap();
        assert_eq!(map.sorted_keys(), vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(map.min(), 1);
        assert_eq!(map.max(), 7);

        map.try_insert(0, &0).unwrap();

        // Should drop the 7
        assert_eq!(map.sorted_keys(), vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(map.min(), 0);
        assert_eq!(map.max(), 6);

        map!(map);
        map.try_insert_default(2).unwrap();
        map.try_insert_default(14).unwrap();
        map.try_insert_default(4).unwrap();
        map.try_insert_default(12).unwrap();

        assert_eq!(map.sorted_keys(), vec![2, 4, 12, 14]);
        assert_eq!(map.min(), 2);
        assert_eq!(map.max(), 14);
    }

    #[test]
    fn test_try_insert_update() {
        map!(map);

        map.try_insert(0, &0).unwrap();
        assert_eq!(map.values.get(0), 0);

        map.insert_update(0, &1);
        assert_eq!(map.values.get(0), 1);
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
}