//! High-performance hash set implementation
//!
//! Built on top of HashMap for code reuse and efficiency

use super::HashMap;
use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{Hash, BuildHasher};

/// A high-performance hash set
#[derive(Debug)]
pub struct HashSet<T, S = RandomState> {
    map: HashMap<T, (), S>,
}

impl<T> HashSet<T, RandomState>
where
    T: Eq + Hash,
{
    /// Creates an empty HashSet
    #[inline]
    pub fn new() -> Self {
        HashSet {
            map: HashMap::new(),
        }
    }

    /// Creates an empty HashSet with specified capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        HashSet {
            map: HashMap::with_capacity(capacity),
        }
    }
}

impl<T, S> HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    /// Creates an empty HashSet with a specific hasher
    #[inline]
    pub fn with_hasher(hasher: S) -> Self {
        HashSet {
            map: HashMap::with_hasher(hasher),
        }
    }

    /// Creates a HashSet with specified capacity and hasher
    #[inline]
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        HashSet {
            map: HashMap::with_capacity_and_hasher(capacity, hasher),
        }
    }

    /// Returns the number of elements in the set
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the set contains no elements
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the capacity of the set
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Adds a value to the set
    /// Returns true if the value was not present in the set
    #[inline]
    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }

    /// Returns true if the set contains a value
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.contains_key(value)
    }

    /// Removes a value from the set
    /// Returns true if the value was present in the set
    #[inline]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.remove(value).is_some()
    }

    /// Clears the set, removing all values
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear()
    }

    /// An iterator visiting all values in arbitrary order
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.map.keys(),
        }
    }

    /// Reserves capacity for at least `additional` more elements
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional)
    }

    /// Visits the values representing the union,
    /// i.e., all the values in `self` or `other`
    pub fn union<'a>(&'a self, other: &'a HashSet<T, S>) -> Union<'a, T, S> 
    where
        S: Default,
    {
        Union {
            iter: self.iter().chain(other.iter()),
            seen: HashSet::with_hasher(S::default()),
        }
    }

    /// Visits the values representing the intersection,
    /// i.e., the values that are both in `self` and `other`
    pub fn intersection<'a>(&'a self, other: &'a HashSet<T, S>) -> Intersection<'a, T, S> {
        let (smaller, larger) = if self.len() <= other.len() {
            (self, other)
        } else {
            (other, self)
        };

        Intersection {
            iter: smaller.iter(),
            other: larger,
        }
    }

    /// Visits the values representing the difference,
    /// i.e., the values that are in `self` but not in `other`
    pub fn difference<'a>(&'a self, other: &'a HashSet<T, S>) -> Difference<'a, T, S> {
        Difference {
            iter: self.iter(),
            other,
        }
    }

    /// Visits the values representing the symmetric difference,
    /// i.e., the values that are in `self` or in `other` but not in both
    pub fn symmetric_difference<'a>(&'a self, other: &'a HashSet<T, S>) -> SymmetricDifference<'a, T, S> {
        SymmetricDifference {
            iter: self.difference(other).chain(other.difference(self)),
        }
    }

    /// Returns true if `self` has no elements in common with `other`
    pub fn is_disjoint(&self, other: &HashSet<T, S>) -> bool {
        if self.len() <= other.len() {
            self.iter().all(|v| !other.contains(v))
        } else {
            other.iter().all(|v| !self.contains(v))
        }
    }

    /// Returns true if the set is a subset of another
    pub fn is_subset(&self, other: &HashSet<T, S>) -> bool {
        if self.len() > other.len() {
            return false;
        }
        self.iter().all(|v| other.contains(v))
    }

    /// Returns true if the set is a superset of another
    pub fn is_superset(&self, other: &HashSet<T, S>) -> bool {
        other.is_subset(self)
    }
}

impl<T> Default for HashSet<T, RandomState>
where
    T: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, S> Clone for HashSet<T, S>
where
    T: Clone + Eq + Hash,
    S: Clone + BuildHasher,
{
    fn clone(&self) -> Self {
        HashSet {
            map: self.map.clone(),
        }
    }
}

impl<'a, T, S> IntoIterator for &'a HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

impl<T, S> IntoIterator for HashSet<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter {
            inner: self.map.into_iter(),
        }
    }
}

impl<T, S> FromIterator<T> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut set = HashSet::with_capacity_and_hasher(lower, S::default());
        for item in iter {
            set.insert(item);
        }
        set
    }
}

// Iterators
pub struct Iter<'a, T> {
    inner: super::hashmap::Keys<'a, T, ()>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct IntoIter<T> {
    inner: super::hashmap::IntoIter<T, ()>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

pub struct Union<'a, T, S> {
    iter: std::iter::Chain<Iter<'a, T>, Iter<'a, T>>,
    seen: HashSet<&'a T, S>,
}

impl<'a, T, S> Iterator for Union<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.iter.next()?;
            if self.seen.insert(item) {
                return Some(item);
            }
        }
    }
}

pub struct Intersection<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a HashSet<T, S>,
}

impl<'a, T, S> Iterator for Intersection<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.iter.next()?;
            if self.other.contains(item) {
                return Some(item);
            }
        }
    }
}

pub struct Difference<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a HashSet<T, S>,
}

impl<'a, T, S> Iterator for Difference<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.iter.next()?;
            if !self.other.contains(item) {
                return Some(item);
            }
        }
    }
}

pub struct SymmetricDifference<'a, T, S> {
    iter: std::iter::Chain<Difference<'a, T, S>, Difference<'a, T, S>>,
}

impl<'a, T, S> Iterator for SymmetricDifference<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

// Send and Sync implementations
unsafe impl<T: Send, S: Send> Send for HashSet<T, S> {}
unsafe impl<T: Sync, S: Sync> Sync for HashSet<T, S> {}

// Helper implementation for collect
impl<T, S> HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    pub fn collect<I: IntoIterator<Item = T>>(iter: I, hasher: S) -> Self {
        let mut set = HashSet::with_hasher(hasher);
        for item in iter {
            set.insert(item);
        }
        set
    }
}

// Extension implementation is already covered by the generic FromIterator above