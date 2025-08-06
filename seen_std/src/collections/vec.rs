//! High-performance vector implementation
//!
//! Optimized for cache efficiency and SIMD operations

use std::alloc::{alloc, dealloc, realloc, Layout};
use std::mem;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::ptr;
use std::slice;

/// A growable array with cache-efficient memory layout
#[derive(Debug)]
pub struct Vec<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

impl<T> Vec<T> {
    /// Creates a new empty vector
    #[inline]
    pub const fn new() -> Self {
        Vec {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        }
    }

    /// Creates a vector with specified capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::new();
        }

        let layout = Layout::array::<T>(capacity).unwrap();
        let ptr = unsafe { alloc(layout) as *mut T };

        Vec {
            ptr,
            len: 0,
            cap: capacity,
        }
    }

    /// Returns the length of the vector
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the vector is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the vector
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Pushes an element to the back of the vector
    #[inline]
    pub fn push(&mut self, value: T) {
        if self.len == self.cap {
            self.grow();
        }

        unsafe {
            ptr::write(self.ptr.add(self.len), value);
            self.len += 1;
        }
    }

    /// Removes and returns the last element
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(ptr::read(self.ptr.add(self.len))) }
        }
    }

    /// Returns a reference to the element at the given index
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            unsafe { Some(&*self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Returns a mutable reference to the element at the given index
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            unsafe { Some(&mut *self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Clears the vector, removing all values
    #[inline]
    pub fn clear(&mut self) {
        unsafe {
            // Drop all elements
            for i in 0..self.len {
                ptr::drop_in_place(self.ptr.add(i));
            }
        }
        self.len = 0;
    }

    /// Reserves capacity for at least `additional` more elements
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let required_cap = self.len.saturating_add(additional);
        if required_cap > self.cap {
            self.grow_to(required_cap);
        }
    }

    /// Inserts an element at position `index`
    #[inline]
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len, "index out of bounds");

        if self.len == self.cap {
            self.grow();
        }

        unsafe {
            if index < self.len {
                // Shift elements to the right
                ptr::copy(
                    self.ptr.add(index),
                    self.ptr.add(index + 1),
                    self.len - index,
                );
            }
            ptr::write(self.ptr.add(index), value);
            self.len += 1;
        }
    }

    /// Removes and returns the element at position `index`
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "index out of bounds");

        unsafe {
            self.len -= 1;
            let result = ptr::read(self.ptr.add(index));
            if index < self.len {
                // Shift elements to the left
                ptr::copy(
                    self.ptr.add(index + 1),
                    self.ptr.add(index),
                    self.len - index,
                );
            }
            result
        }
    }

    /// Extends the vector with contents of a slice
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[T])
    where
        T: Clone,
    {
        self.reserve(slice.len());
        for item in slice {
            self.push(item.clone());
        }
    }

    /// Returns a slice of the entire vector
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    /// Returns a mutable slice of the entire vector
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0 {
            &mut []
        } else {
            unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    }

    /// Returns an iterator over the vector
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Returns true if the vector contains an element equal to the given value
    #[inline]
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq,
    {
        self.as_slice().contains(x)
    }

    // Growth strategy: double capacity or at least 4 elements
    #[cold]
    #[inline(never)]
    fn grow(&mut self) {
        let new_cap = if self.cap == 0 {
            4
        } else {
            self.cap * 2
        };
        self.grow_to(new_cap);
    }

    fn grow_to(&mut self, new_cap: usize) {
        unsafe {
            let new_layout = Layout::array::<T>(new_cap).unwrap();

            let new_ptr = if self.cap == 0 {
                alloc(new_layout) as *mut T
            } else {
                let old_layout = Layout::array::<T>(self.cap).unwrap();
                realloc(self.ptr as *mut u8, old_layout, new_layout.size()) as *mut T
            };

            self.ptr = new_ptr;
            self.cap = new_cap;
        }
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            unsafe {
                // Drop all elements
                for i in 0..self.len {
                    ptr::drop_in_place(self.ptr.add(i));
                }
                // Deallocate memory
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}

impl<T> Deref for Vec<T> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for Vec<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> Index<usize> for Vec<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.as_slice()[index]
    }
}

impl<T> IndexMut<usize> for Vec<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.as_mut_slice()[index]
    }
}

impl<T: Clone> Clone for Vec<T> {
    fn clone(&self) -> Self {
        let mut new_vec = Vec::with_capacity(self.len);
        new_vec.extend_from_slice(self.as_slice());
        new_vec
    }
}

impl<T: PartialEq> PartialEq for Vec<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        for i in 0..self.len {
            if self[i] != other[i] {
                return false;
            }
        }
        true
    }
}

impl<T> Default for Vec<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Iterator support
impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        let ptr = self.ptr;
        let len = self.len;
        let cap = self.cap;
        
        // Prevent drop from running
        mem::forget(self);
        
        IntoIter {
            ptr,
            len,
            cap,
            pos: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> std::slice::Iter<'a, T> {
        self.as_slice().iter()
    }
}

pub struct IntoIter<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
    pos: usize,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.pos < self.len {
            unsafe {
                let item = ptr::read(self.ptr.add(self.pos));
                self.pos += 1;
                Some(item)
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.pos;
        (remaining, Some(remaining))
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        if self.cap != 0 {
            unsafe {
                // Drop remaining elements
                for i in self.pos..self.len {
                    ptr::drop_in_place(self.ptr.add(i));
                }
                // Deallocate memory
                let layout = Layout::array::<T>(self.cap).unwrap();
                dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}

impl<T> std::iter::FromIterator<T> for Vec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut vec = Vec::new();
        for item in iter {
            vec.push(item);
        }
        vec
    }
}

// Send and Sync implementations
unsafe impl<T: Send> Send for Vec<T> {}
unsafe impl<T: Sync> Sync for Vec<T> {}