//! Hybrid generational handles used by the region runtime.
//!
//! The design follows the Vale-style handle tables referenced in the MVP plan:
//! indexes are 32-bit, generations increment on every free, and hot lookups can
//! elide runtime checks in release builds.

use std::fmt;

/// Handle that uniquely identifies a slot in a [`HybridGenerationalArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationalHandle {
    index: u32,
    generation: u32,
}

impl GenerationalHandle {
    /// Construct a handle from raw components.
    #[inline]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Return the slot index portion of the handle.
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Return the generation counter associated with the handle.
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug, Clone)]
struct Slot<T> {
    value: Option<T>,
    generation: u32,
    next_free: Option<u32>,
}

impl<T> Slot<T> {
    #[inline]
    fn new(value: T) -> Self {
        Self {
            value: Some(value),
            generation: 1,
            next_free: None,
        }
    }
}

/// Arena that mixes contiguous storage with generational handles.
#[derive(Debug, Clone)]
pub struct HybridGenerationalArena<T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    live: u32,
}

impl<T> Default for HybridGenerationalArena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            live: 0,
        }
    }
}

impl<T> HybridGenerationalArena<T> {
    /// Create an empty arena.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a value and obtain a generational handle.
    #[inline]
    pub fn insert(&mut self, value: T) -> GenerationalHandle {
        if let Some(index) = self.free_head.take() {
            let slot = &mut self.slots[index as usize];
            self.free_head = slot.next_free;
            slot.value = Some(value);
            slot.next_free = None;
            self.live += 1;
            GenerationalHandle::new(index, slot.generation)
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot::new(value));
            self.live += 1;
            GenerationalHandle::new(index, 1)
        }
    }

    /// Resolve a handle to an immutable reference, performing generation checks.
    #[inline]
    pub fn resolve(&self, handle: GenerationalHandle) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    /// Resolve a handle to a mutable reference, performing generation checks.
    #[inline]
    pub fn resolve_mut(&mut self, handle: GenerationalHandle) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation == handle.generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Unsafe variant that skips generation checks for hot paths.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that `handle` was obtained from this arena and
    /// has not been invalidated by a prior `remove` or `clear`.
    #[inline(always)]
    pub unsafe fn resolve_unchecked(&self, handle: GenerationalHandle) -> &T {
        debug_assert!(self.contains(handle), "stale handle access");
        // SAFETY: caller promises handle is valid; debug builds double-check.
        self.slots
            .get_unchecked(handle.index as usize)
            .value
            .as_ref()
            .expect("unchecked handle must be live")
    }

    /// Unsafe mutable variant mirroring [`resolve_unchecked`].
    ///
    /// # Safety
    ///
    /// Same requirements as [`resolve_unchecked`].
    #[inline(always)]
    pub unsafe fn resolve_unchecked_mut(&mut self, handle: GenerationalHandle) -> &mut T {
        debug_assert!(self.contains(handle), "stale handle access");
        self.slots
            .get_unchecked_mut(handle.index as usize)
            .value
            .as_mut()
            .expect("unchecked handle must be live")
    }

    /// Remove the value associated with `handle` and invalidate it.
    #[inline]
    pub fn remove(&mut self, handle: GenerationalHandle) -> Option<T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }

        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        slot.next_free = self.free_head;
        self.free_head = Some(handle.index);
        self.live = self.live.saturating_sub(1);
        Some(value)
    }

    /// Returns true if `handle` still points at a live value.
    #[inline]
    pub fn contains(&self, handle: GenerationalHandle) -> bool {
        self.resolve(handle).is_some()
    }

    /// Number of live entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.live as usize
    }

    /// Whether the arena is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Drop all live entries and invalidate their handles.
    pub fn clear(&mut self) {
        for index in 0..self.slots.len() {
            if self.slots[index].value.is_some() {
                let generation = self.slots[index].generation;
                let handle = GenerationalHandle::new(index as u32, generation);
                let _ = self.remove(handle);
            }
        }
    }
}

#[cfg(debug_assertions)]
impl<T> HybridGenerationalArena<T> {
    /// Safe hot-path lookup with checks retained in debug builds.
    #[inline(always)]
    pub fn resolve_fast(&self, handle: GenerationalHandle) -> Option<&T> {
        self.resolve(handle)
    }
}

#[cfg(not(debug_assertions))]
impl<T> HybridGenerationalArena<T> {
    /// Hot-path lookup that elides generation checks in release/bench profiles.
    #[inline(always)]
    pub fn resolve_fast(&self, handle: GenerationalHandle) -> Option<&T> {
        // SAFETY: release builds rely on upstream analysis to pass only valid handles.
        unsafe { Some(self.resolve_unchecked(handle)) }
    }
}

impl fmt::Display for GenerationalHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[#{} @ gen{}]", self.index, self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_increments_generation() {
        let mut arena = HybridGenerationalArena::new();
        let handle = arena.insert(5u32);
        assert!(arena.contains(handle));

        let removed = arena.remove(handle);
        assert_eq!(removed, Some(5));
        assert!(!arena.contains(handle));

        let new_handle = arena.insert(7u32);
        assert_ne!(handle.generation(), new_handle.generation());
        assert_eq!(arena.resolve(new_handle), Some(&7));
    }

    #[test]
    fn resolve_unchecked_matches_checked() {
        let mut arena = HybridGenerationalArena::new();
        let handle = arena.insert("region");
        let checked = arena.resolve(handle).unwrap();
        let unchecked = unsafe { arena.resolve_unchecked(handle) };
        assert_eq!(checked, unchecked);
    }

    #[test]
    fn clear_invalidates_handles() {
        let mut arena = HybridGenerationalArena::new();
        let h1 = arena.insert(11);
        let h2 = arena.insert(22);
        assert_eq!(arena.len(), 2);

        arena.clear();

        assert!(!arena.contains(h1));
        assert!(!arena.contains(h2));
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);

        let h3 = arena.insert(33);
        assert!(arena.contains(h3));
    }
}
