//! High-performance hash map implementation
//!
//! Robin Hood hashing with backward shift deletion for cache efficiency

use std::alloc::{alloc, dealloc, Layout};
use std::marker::PhantomData;
use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{Hash, Hasher, BuildHasher};
use std::mem;
use std::ptr;

const INITIAL_CAPACITY: usize = 16;
const MAX_LOAD_FACTOR: f64 = 0.85;

/// Entry in the hash table
#[derive(Clone)]
struct Entry<K, V> {
    key: K,
    value: V,
    hash: u64,
    psl: u32, // Probe sequence length (Robin Hood)
}

/// A high-performance hash map using Robin Hood hashing
#[derive(Debug)]
pub struct HashMap<K, V, S = RandomState> {
    buckets: *mut Option<Entry<K, V>>,
    capacity: usize,
    len: usize,
    hasher: S,
}

impl<K, V> HashMap<K, V, RandomState> 
where
    K: Eq + Hash,
{
    /// Creates an empty HashMap
    #[inline]
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }

    /// Creates an empty HashMap with specified capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::new())
    }
}

impl<K, V, S> HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Creates an empty HashMap with a specific hasher
    #[inline]
    pub fn with_hasher(hasher: S) -> Self {
        Self::with_capacity_and_hasher(0, hasher)
    }

    /// Creates a HashMap with specified capacity and hasher
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        let capacity = capacity.max(INITIAL_CAPACITY).next_power_of_two();
        let layout = Layout::array::<Option<Entry<K, V>>>(capacity).unwrap();
        
        let buckets = unsafe {
            let ptr = alloc(layout) as *mut Option<Entry<K, V>>;
            // Initialize all buckets to None
            for i in 0..capacity {
                ptr::write(ptr.add(i), None);
            }
            ptr
        };

        HashMap {
            buckets,
            capacity,
            len: 0,
            hasher,
        }
    }

    /// Returns the number of elements in the map
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the map contains no elements
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the map
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Inserts a key-value pair into the map
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.len as f64 >= self.capacity as f64 * MAX_LOAD_FACTOR {
            self.resize();
        }

        let hash = self.hash_key(&key);
        let mut index = (hash as usize) & (self.capacity - 1);
        let psl = 0;
        let mut entry = Entry { key, value, hash, psl };

        unsafe {
            loop {
                let bucket = &mut *self.buckets.add(index);
                
                match bucket {
                    None => {
                        *bucket = Some(entry);
                        self.len += 1;
                        return None;
                    }
                    Some(existing) => {
                        // Check for key equality
                        if existing.hash == entry.hash && existing.key == entry.key {
                            let old_value = mem::replace(&mut existing.value, entry.value);
                            return Some(old_value);
                        }

                        // Robin Hood: swap if current entry has traveled further
                        if existing.psl < entry.psl {
                            mem::swap(existing, &mut entry);
                        }

                        index = (index + 1) & (self.capacity - 1);
                        entry.psl += 1;
                    }
                }
            }
        }
    }

    /// Returns a reference to the value corresponding to the key
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        let mut index = (hash as usize) & (self.capacity - 1);
        let mut psl = 0;

        unsafe {
            loop {
                let bucket = &*self.buckets.add(index);
                
                match bucket {
                    None => return None,
                    Some(entry) => {
                        if entry.psl < psl {
                            return None; // Robin Hood: entry would be here if it existed
                        }
                        
                        if entry.hash == hash && entry.key.borrow() == key {
                            return Some(&entry.value);
                        }

                        index = (index + 1) & (self.capacity - 1);
                        psl += 1;
                    }
                }
            }
        }
    }

    /// Returns a mutable reference to the value corresponding to the key
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        let mut index = (hash as usize) & (self.capacity - 1);
        let mut psl = 0;

        unsafe {
            loop {
                let bucket = &mut *self.buckets.add(index);
                
                match bucket {
                    None => return None,
                    Some(entry) => {
                        if entry.psl < psl {
                            return None;
                        }
                        
                        if entry.hash == hash && entry.key.borrow() == key {
                            return Some(&mut entry.value);
                        }

                        index = (index + 1) & (self.capacity - 1);
                        psl += 1;
                    }
                }
            }
        }
    }

    /// Removes a key from the map, returning the value if it was present
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        let mut index = (hash as usize) & (self.capacity - 1);
        let mut psl = 0;

        unsafe {
            loop {
                let bucket = &mut *self.buckets.add(index);
                
                match bucket {
                    None => return None,
                    Some(entry) => {
                        if entry.psl < psl {
                            return None;
                        }
                        
                        if entry.hash == hash && entry.key.borrow() == key {
                            let entry = bucket.take().unwrap();
                            self.len -= 1;
                            
                            // Backward shift deletion for Robin Hood
                            self.backward_shift_deletion(index);
                            
                            return Some(entry.value);
                        }

                        index = (index + 1) & (self.capacity - 1);
                        psl += 1;
                    }
                }
            }
        }
    }

    /// Backward shift deletion to maintain Robin Hood invariant
    fn backward_shift_deletion(&mut self, start_index: usize) {
        let mut index = start_index;
        
        unsafe {
            loop {
                let next_index = (index + 1) & (self.capacity - 1);
                let next_bucket = &mut *self.buckets.add(next_index);
                
                match next_bucket {
                    None => break,
                    Some(entry) if entry.psl == 0 => break,
                    Some(entry) => {
                        entry.psl -= 1;
                        let bucket = &mut *self.buckets.add(index);
                        *bucket = next_bucket.take();
                        index = next_index;
                    }
                }
            }
        }
    }

    /// Clears the map, removing all key-value pairs
    pub fn clear(&mut self) {
        unsafe {
            for i in 0..self.capacity {
                let bucket = &mut *self.buckets.add(i);
                *bucket = None;
            }
        }
        self.len = 0;
    }

    /// Returns true if the map contains a value for the specified key
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// An iterator visiting all key-value pairs in arbitrary order
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            buckets: self.buckets,
            capacity: self.capacity,
            index: 0,
            remaining: self.len,
            _phantom: PhantomData,
        }
    }

    /// An iterator visiting all keys in arbitrary order
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }

    /// An iterator visiting all values in arbitrary order
    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }

    /// Reserves capacity for at least `additional` more elements
    pub fn reserve(&mut self, additional: usize) {
        let required = self.len + additional;
        if required > self.capacity {
            let new_capacity = required.next_power_of_two();
            self.resize_to(new_capacity);
        }
    }

    #[inline]
    fn hash_key(&self, key: &K) -> u64 {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        hasher.finish()
    }

    fn resize(&mut self) {
        self.resize_to(self.capacity * 2);
    }

    fn resize_to(&mut self, new_capacity: usize) {
        let old_buckets = self.buckets;
        let old_capacity = self.capacity;
        
        // Create new buckets
        let layout = Layout::array::<Option<Entry<K, V>>>(new_capacity).unwrap();
        let new_buckets = unsafe {
            let ptr = alloc(layout) as *mut Option<Entry<K, V>>;
            for i in 0..new_capacity {
                ptr::write(ptr.add(i), None);
            }
            ptr
        };

        self.buckets = new_buckets;
        self.capacity = new_capacity;
        self.len = 0;

        // Reinsert all entries
        unsafe {
            for i in 0..old_capacity {
                let bucket = &mut *old_buckets.add(i);
                if let Some(entry) = bucket.take() {
                    let hash = entry.hash;
                    let mut index = (hash as usize) & (self.capacity - 1);
                    let psl = 0;
                    let mut entry = Entry {
                        key: entry.key,
                        value: entry.value,
                        hash,
                        psl,
                    };

                    loop {
                        let new_bucket = &mut *self.buckets.add(index);
                        
                        match new_bucket {
                            None => {
                                *new_bucket = Some(entry);
                                self.len += 1;
                                break;
                            }
                            Some(existing) => {
                                if existing.psl < entry.psl {
                                    mem::swap(existing, &mut entry);
                                }
                                index = (index + 1) & (self.capacity - 1);
                                entry.psl += 1;
                            }
                        }
                    }
                }
            }

            // Deallocate old buckets
            let old_layout = Layout::array::<Option<Entry<K, V>>>(old_capacity).unwrap();
            dealloc(old_buckets as *mut u8, old_layout);
        }
    }
}

impl<K, V, S> Drop for HashMap<K, V, S> {
    fn drop(&mut self) {
        if self.capacity > 0 {
            unsafe {
                // Drop all entries
                for i in 0..self.capacity {
                    let bucket = &mut *self.buckets.add(i);
                    *bucket = None;
                }
                
                // Deallocate memory
                let layout = Layout::array::<Option<Entry<K, V>>>(self.capacity).unwrap();
                dealloc(self.buckets as *mut u8, layout);
            }
        }
    }
}

impl<K, V> Default for HashMap<K, V, RandomState>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

// Iterators
pub struct Iter<'a, K, V> {
    buckets: *mut Option<Entry<K, V>>,
    capacity: usize,
    index: usize,
    remaining: usize,
    _phantom: PhantomData<&'a (K, V)>,
}

impl<'a, K: 'a, V: 'a> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            while self.index < self.capacity {
                let bucket = &*self.buckets.add(self.index);
                self.index += 1;
                
                if let Some(entry) = bucket {
                    self.remaining -= 1;
                    return Some((&entry.key, &entry.value));
                }
            }
        }
        
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

pub struct Keys<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K: 'a, V: 'a> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct Values<'a, K, V> {
    inner: Iter<'a, K, V>,
}

impl<'a, K: 'a, V: 'a> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// IntoIterator implementation
impl<K, V, S> IntoIterator for HashMap<K, V, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> IntoIter<K, V> {
        let buckets = self.buckets;
        let capacity = self.capacity;
        let len = self.len;
        
        // Prevent drop from running
        mem::forget(self);
        
        IntoIter {
            buckets,
            capacity,
            index: 0,
            remaining: len,
        }
    }
}

pub struct IntoIter<K, V> {
    buckets: *mut Option<Entry<K, V>>,
    capacity: usize,
    index: usize,
    remaining: usize,
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        unsafe {
            while self.index < self.capacity {
                let bucket = &mut *self.buckets.add(self.index);
                self.index += 1;
                
                if let Some(entry) = bucket.take() {
                    self.remaining -= 1;
                    return Some((entry.key, entry.value));
                }
            }
        }
        
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> Drop for IntoIter<K, V> {
    fn drop(&mut self) {
        if self.capacity > 0 {
            unsafe {
                // Drop remaining entries
                for i in self.index..self.capacity {
                    let bucket = &mut *self.buckets.add(i);
                    *bucket = None;
                }
                
                // Deallocate memory
                let layout = Layout::array::<Option<Entry<K, V>>>(self.capacity).unwrap();
                dealloc(self.buckets as *mut u8, layout);
            }
        }
    }
}

impl<K: Clone + Eq + Hash, V: Clone, S: Clone + BuildHasher> Clone for HashMap<K, V, S> {
    fn clone(&self) -> Self {
        let mut new_map = HashMap::with_capacity_and_hasher(self.capacity, self.hasher.clone());
        for (key, value) in self.iter() {
            new_map.insert(key.clone(), value.clone());
        }
        new_map
    }
}

impl<K: PartialEq + Eq + Hash, V: PartialEq> PartialEq for HashMap<K, V> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        for (key, value) in self.iter() {
            match other.get(key) {
                Some(other_value) => {
                    if value != other_value {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

impl<K, V, S> FromIterator<(K, V)> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut map = HashMap::with_capacity_and_hasher(lower, S::default());
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

impl<K, V, S> std::ops::Index<&K> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Output = V;

    fn index(&self, key: &K) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

impl<K, V, S> std::ops::IndexMut<&K> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn index_mut(&mut self, key: &K) -> &mut V {
        self.get_mut(key).expect("no entry found for key")
    }
}

impl<'a, K, V, S> IntoIterator for &'a HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

// Send and Sync implementations
unsafe impl<K: Send, V: Send, S: Send> Send for HashMap<K, V, S> {}
unsafe impl<K: Sync, V: Sync, S: Sync> Sync for HashMap<K, V, S> {}