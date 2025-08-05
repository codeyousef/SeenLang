//! Memory management utilities for the Seen language
//! 
//! This module provides Vale-style memory management primitives
//! that will be used in the self-hosted compiler implementation.

use serde::{Deserialize, Serialize};

/// Memory region identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegionId(pub u32);

/// Reference to a value in a specific region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionRef<T> {
    pub value: T,
    pub region: RegionId,
}

impl<T> RegionRef<T> {
    pub fn new(value: T, region: RegionId) -> Self {
        Self { value, region }
    }
    
    pub fn get(&self) -> &T {
        &self.value
    }
    
    pub fn region(&self) -> RegionId {
        self.region
    }
}

/// Memory allocation tracking (for the future Vale-style implementation)
pub struct MemoryTracker {
    next_region: u32,
}

impl MemoryTracker {
    pub fn new() -> Self {
        Self { next_region: 0 }
    }
    
    pub fn create_region(&mut self) -> RegionId {
        let id = RegionId(self.next_region);
        self.next_region += 1;
        id
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}