//! Generational references for memory safety
//! 
//! Implements Vale-style generational references to prevent use-after-free
//! errors while maintaining high performance.

use crate::regions::RegionId;
use crate::runtime::RuntimeManager;
use serde::{Serialize, Deserialize};

/// A generational reference that tracks object validity
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerationalRef<T> {
    region_id: RegionId,
    object_id: u32,
    generation: u32,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: 'static> GenerationalRef<T> {
    /// Create a new generational reference
    pub fn new(region_id: RegionId, object_id: u32, generation: u32) -> Self {
        Self {
            region_id,
            object_id,
            generation,
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Check if this reference is still valid
    pub fn is_valid(&self, runtime: &RuntimeManager) -> bool {
        runtime.is_reference_valid(self.region_id, self.object_id, self.generation)
    }
    
    /// Get the referenced object if still valid
    pub fn get<'a>(&self, runtime: &'a RuntimeManager) -> Option<&'a T> {
        if self.is_valid(runtime) {
            runtime.get_object(self.region_id, self.object_id)
        } else {
            None
        }
    }
    
    /// Get mutable reference to the object if still valid
    pub fn get_mut<'a>(&self, runtime: &'a mut RuntimeManager) -> Option<&'a mut T> {
        if self.is_valid(runtime) {
            runtime.get_object_mut(self.region_id, self.object_id)
        } else {
            None
        }
    }
    
    /// Get the region ID this reference points to
    pub fn region_id(&self) -> RegionId {
        self.region_id
    }
    
    /// Get the object ID within the region
    pub fn object_id(&self) -> u32 {
        self.object_id
    }
    
    /// Get the generation of this reference
    pub fn generation(&self) -> u32 {
        self.generation
    }
}

impl<T> Clone for GenerationalRef<T> {
    fn clone(&self) -> Self {
        Self {
            region_id: self.region_id,
            object_id: self.object_id,
            generation: self.generation,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Reference counting for shared regions
#[derive(Debug, Clone)]
pub struct SharedRef<T> {
    inner: GenerationalRef<T>,
    ref_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl<T: 'static> SharedRef<T> {
    pub fn new(gen_ref: GenerationalRef<T>) -> Self {
        Self {
            inner: gen_ref,
            ref_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        }
    }
    
    pub fn clone(&self) -> Self {
        self.ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            inner: self.inner.clone(),
            ref_count: self.ref_count.clone(),
        }
    }
    
    pub fn get<'a>(&self, runtime: &'a RuntimeManager) -> Option<&'a T> {
        self.inner.get(runtime)
    }
    
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl<T> Drop for SharedRef<T> {
    fn drop(&mut self) {
        let old_count = self.ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        if old_count == 1 {
            // Last reference dropped, cleanup can happen
            // In a full implementation, this would notify the runtime
        }
    }
}

/// Weak reference that doesn't prevent cleanup
#[derive(Debug, Clone)]
pub struct WeakRef<T> {
    inner: GenerationalRef<T>,
}

impl<T: 'static> WeakRef<T> {
    pub fn new(gen_ref: GenerationalRef<T>) -> Self {
        Self { inner: gen_ref }
    }
    
    pub fn upgrade(&self, runtime: &RuntimeManager) -> Option<GenerationalRef<T>> {
        if self.inner.is_valid(runtime) {
            Some(self.inner.clone())
        } else {
            None
        }
    }
    
    pub fn is_valid(&self, runtime: &RuntimeManager) -> bool {
        self.inner.is_valid(runtime)
    }
}

/// Type-erased generational reference for runtime use
#[derive(Debug, Clone, PartialEq)]
pub struct TypeErasedRef {
    region_id: RegionId,
    object_id: u32,
    generation: u32,
}

impl TypeErasedRef {
    pub fn new(region_id: RegionId, object_id: u32, generation: u32) -> Self {
        Self {
            region_id,
            object_id,
            generation,
        }
    }
    
    pub fn region_id(&self) -> RegionId {
        self.region_id
    }
    
    pub fn object_id(&self) -> u32 {
        self.object_id
    }
    
    pub fn generation(&self) -> u32 {
        self.generation
    }
    
    /// Convert to typed reference
    pub fn typed<T: 'static>(self) -> GenerationalRef<T> {
        GenerationalRef::new(self.region_id, self.object_id, self.generation)
    }
}

impl<T> From<GenerationalRef<T>> for TypeErasedRef {
    fn from(gen_ref: GenerationalRef<T>) -> Self {
        Self::new(gen_ref.region_id, gen_ref.object_id, gen_ref.generation)
    }
}