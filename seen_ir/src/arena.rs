use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Index into a 32-bit arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub struct ArenaIndex(u32);

impl ArenaIndex {
    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for ArenaIndex {
    fn from(value: usize) -> Self {
        assert!(
            value <= u32::MAX as usize,
            "arena overflow: {value} elements exceeds u32 range"
        );
        Self(value as u32)
    }
}

impl From<ArenaIndex> for usize {
    fn from(value: ArenaIndex) -> Self {
        value.as_usize()
    }
}

impl fmt::Display for ArenaIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArenaIndex({})", self.0)
    }
}

/// Compact arena that stores entries densely and exposes 32-bit indices.
#[derive(Debug, Clone, Default)]
pub struct Arena<T> {
    entries: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(&mut self, value: T) -> ArenaIndex {
        let idx = self.entries.len();
        assert!(
            idx < u32::MAX as usize,
            "arena overflow: exceeded 32-bit index space"
        );
        self.entries.push(value);
        ArenaIndex(idx as u32)
    }

    pub fn get(&self, index: ArenaIndex) -> Option<&T> {
        self.entries.get(index.as_usize())
    }

    pub fn get_mut(&mut self, index: ArenaIndex) -> Option<&mut T> {
        self.entries.get_mut(index.as_usize())
    }

    pub fn iter(&self) -> impl Iterator<Item=&T> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut T> {
        self.entries.iter_mut()
    }

    pub fn indices(&self) -> impl Iterator<Item=ArenaIndex> + '_ {
        (0..self.entries.len()).map(ArenaIndex::from)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.entries.clone()
    }

    pub fn into_vec(self) -> Vec<T> {
        self.entries
    }
}

impl<T> Deref for Arena<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl<T> DerefMut for Arena<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

impl<T: Serialize> Serialize for Arena<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.entries.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Arena<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<T>::deserialize(deserializer)?;
        Ok(Self { entries })
    }
}

impl<T> From<Vec<T>> for Arena<T> {
    fn from(entries: Vec<T>) -> Self {
        Self { entries }
    }
}

impl<T> From<Arena<T>> for Vec<T> {
    fn from(arena: Arena<T>) -> Self {
        arena.entries
    }
}

impl<'a, T> IntoIterator for &'a Arena<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Arena<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}
