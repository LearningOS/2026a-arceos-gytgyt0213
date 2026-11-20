//! An open-addressed hash map using linear probing.
//!
//! The default FNV-1a hasher needs no operating-system entropy. It is
//! deterministic and is intended for trusted kernel data, rather than HashDoS
//! resistance. Callers can supply another `BuildHasher` when needed.

use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};

/// The default deterministic hasher used by [`HashMap`].
pub struct DefaultHasher(u64);

impl Default for DefaultHasher {
    fn default() -> Self {
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for DefaultHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

enum Bucket<K, V> {
    Empty,
    Deleted,
    Occupied(u64, K, V),
}

/// A hash table with amortized constant-time insertion and lookup.
pub struct HashMap<K, V, S = BuildHasherDefault<DefaultHasher>> {
    buckets: Vec<Bucket<K, V>>,
    len: usize,
    deleted: usize,
    hasher: S,
}

impl<K, V> HashMap<K, V> {
    /// Creates an empty map without allocating.
    pub fn new() -> Self {
        Self::with_hasher(BuildHasherDefault::default())
    }
}

impl<K: Eq + Hash, V> HashMap<K, V> {
    /// Creates a map that can hold at least `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut map = Self::new();
        map.reserve(capacity);
        map
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty map using a supplied hash builder.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            buckets: Vec::new(),
            len: 0,
            deleted: 0,
            hasher,
        }
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of entries that fit without reallocating.
    pub fn capacity(&self) -> usize {
        self.buckets.len() / 4 * 3
    }

    /// Removes all entries while keeping the allocated storage.
    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            *bucket = Bucket::Empty;
        }
        self.len = 0;
        self.deleted = 0;
    }

    /// Iterates over entries in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets.iter().filter_map(|bucket| match bucket {
            Bucket::Occupied(_, key, value) => Some((key, value)),
            _ => None,
        })
    }

    /// Iterates over keys and mutable values.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.buckets.iter_mut().filter_map(|bucket| match bucket {
            Bucket::Occupied(_, key, value) => Some((&*key, value)),
            _ => None,
        })
    }

    /// Iterates over keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }

    /// Iterates over values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }
}

impl<K: Eq + Hash, V, S: BuildHasher> HashMap<K, V, S> {
    fn hash<Q: Hash + ?Sized>(&self, key: &Q) -> u64 {
        self.hasher.hash_one(key)
    }

    fn find<Q: Eq + Hash + ?Sized>(&self, key: &Q, hash: u64) -> Option<usize>
    where
        K: Borrow<Q>,
    {
        if self.buckets.is_empty() {
            return None;
        }
        let mask = self.buckets.len() - 1;
        let mut index = hash as usize & mask;
        for _ in 0..self.buckets.len() {
            match &self.buckets[index] {
                Bucket::Empty => return None,
                Bucket::Occupied(h, k, _) if *h == hash && k.borrow() == key => return Some(index),
                _ => index = (index + 1) & mask,
            }
        }
        None
    }

    fn insert_unique(&mut self, hash: u64, key: K, value: V) {
        let mask = self.buckets.len() - 1;
        let mut index = hash as usize & mask;
        while matches!(self.buckets[index], Bucket::Occupied(..)) {
            index = (index + 1) & mask;
        }
        if matches!(self.buckets[index], Bucket::Deleted) {
            self.deleted -= 1;
        }
        self.buckets[index] = Bucket::Occupied(hash, key, value);
        self.len += 1;
    }

    /// Reserves space for at least `additional` more entries.
    pub fn reserve(&mut self, additional: usize) {
        let needed = self
            .len
            .checked_add(additional)
            .expect("hash map capacity overflow");
        if needed
            .checked_add(self.deleted)
            .expect("hash map capacity overflow")
            <= self.capacity()
        {
            return;
        }
        let mut size = self.buckets.len().max(8);
        while size / 4 * 3 < needed {
            size = size.checked_mul(2).expect("hash map capacity overflow");
        }
        let mut buckets = Vec::with_capacity(size);
        buckets.resize_with(size, || Bucket::Empty);
        let old = core::mem::replace(&mut self.buckets, buckets);
        self.len = 0;
        self.deleted = 0;
        for bucket in old {
            if let Bucket::Occupied(hash, key, value) = bucket {
                self.insert_unique(hash, key, value);
            }
        }
    }

    /// Inserts a value, returning the old value when the key already exists.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let hash = self.hash(&key);
        if let Some(index) = self.find(&key, hash) {
            if let Bucket::Occupied(_, _, old) = &mut self.buckets[index] {
                return Some(core::mem::replace(old, value));
            }
        }
        self.reserve(1);
        self.insert_unique(hash, key, value);
        None
    }

    /// Looks up a key, including a borrowed form such as `str` for `String`.
    pub fn get<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key, self.hash(key))?;
        match &self.buckets[index] {
            Bucket::Occupied(_, _, value) => Some(value),
            _ => None,
        }
    }

    /// Looks up a mutable value.
    pub fn get_mut<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key, self.hash(key))?;
        match &mut self.buckets[index] {
            Bucket::Occupied(_, _, value) => Some(value),
            _ => None,
        }
    }

    /// Returns whether a key exists.
    pub fn contains_key<Q: Eq + Hash + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }

    /// Removes a key while preserving probe chains through deleted buckets.
    pub fn remove<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
    {
        let index = self.find(key, self.hash(key))?;
        let old = core::mem::replace(&mut self.buckets[index], Bucket::Deleted);
        if let Bucket::Occupied(_, _, value) = old {
            self.len -= 1;
            self.deleted += 1;
            Some(value)
        } else {
            None
        }
    }
}

impl<K, V, S: Default> Default for HashMap<K, V, S> {
    fn default() -> Self {
        Self::with_hasher(S::default())
    }
}
