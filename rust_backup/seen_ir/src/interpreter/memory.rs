//! Memory model for the IR interpreter
//!
//! This module implements a safe, inspectable memory model with:
//! - Redzone protection to detect buffer overflows
//! - Use-after-free detection
//! - Memory leak tracking
//! - Full allocation history for debugging

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for each allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationId(pub u64);

impl fmt::Display for AllocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "alloc#{}", self.0)
    }
}

/// Virtual memory address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address(pub u64);

impl Address {
    pub const NULL: Address = Address(0);

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }

    pub fn offset(&self, bytes: i64) -> Address {
        Address((self.0 as i64 + bytes) as u64)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

/// A region of allocated memory with metadata
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Unique ID for this allocation
    pub id: AllocationId,
    /// Base address of this region
    pub base: Address,
    /// Size of the allocation in bytes (not including redzones)
    pub size: usize,
    /// The actual data bytes
    data: Vec<u8>,
    /// Whether this region has been freed
    pub freed: bool,
    /// Source location that allocated this memory (for debugging)
    pub allocation_site: Option<String>,
    /// Source location that freed this memory (if freed)
    pub free_site: Option<String>,
    /// Type information for structured data
    pub type_info: Option<String>,
}

/// Redzone marker byte
const REDZONE_BYTE: u8 = 0xCD;
/// Size of redzone on each side of allocation
const REDZONE_SIZE: usize = 16;
/// Fill byte for freed memory
const FREED_BYTE: u8 = 0xDD;
/// Fill byte for uninitialized memory  
const UNINIT_BYTE: u8 = 0xAA;

impl MemoryRegion {
    pub fn new(id: AllocationId, base: Address, size: usize, type_info: Option<String>) -> Self {
        // Allocate with redzones: [REDZONE][DATA][REDZONE]
        let total_size = REDZONE_SIZE + size + REDZONE_SIZE;
        let mut data = vec![UNINIT_BYTE; total_size];
        
        // Fill redzones with marker
        for i in 0..REDZONE_SIZE {
            data[i] = REDZONE_BYTE;
            data[REDZONE_SIZE + size + i] = REDZONE_BYTE;
        }
        
        Self {
            id,
            base,
            size,
            data,
            freed: false,
            allocation_site: None,
            free_site: None,
            type_info,
        }
    }

    /// Check if redzones are intact (no buffer overflow)
    pub fn check_redzones(&self) -> Result<(), String> {
        // Check front redzone
        for i in 0..REDZONE_SIZE {
            if self.data[i] != REDZONE_BYTE {
                return Err(format!(
                    "Buffer underflow detected at {} (redzone corruption at offset -{})",
                    self.base, REDZONE_SIZE - i
                ));
            }
        }
        
        // Check back redzone
        for i in 0..REDZONE_SIZE {
            let idx = REDZONE_SIZE + self.size + i;
            if self.data[idx] != REDZONE_BYTE {
                return Err(format!(
                    "Buffer overflow detected at {} (redzone corruption at offset +{})",
                    self.base, self.size + i
                ));
            }
        }
        
        Ok(())
    }

    /// Read bytes from this region
    pub fn read(&self, offset: usize, count: usize) -> Result<&[u8], String> {
        if self.freed {
            return Err(format!(
                "Use-after-free: reading from freed allocation {} (freed at: {:?})",
                self.id, self.free_site
            ));
        }
        
        if offset + count > self.size {
            return Err(format!(
                "Out of bounds read: offset {} + size {} > allocation size {} in {}",
                offset, count, self.size, self.id
            ));
        }
        
        let start = REDZONE_SIZE + offset;
        let end = start + count;
        Ok(&self.data[start..end])
    }

    /// Write bytes to this region
    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), String> {
        if self.freed {
            return Err(format!(
                "Use-after-free: writing to freed allocation {} (freed at: {:?})",
                self.id, self.free_site
            ));
        }
        
        if offset + bytes.len() > self.size {
            return Err(format!(
                "Out of bounds write: offset {} + size {} > allocation size {} in {}",
                offset, bytes.len(), self.size, self.id
            ));
        }
        
        let start = REDZONE_SIZE + offset;
        self.data[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Mark this region as freed
    pub fn mark_freed(&mut self, free_site: Option<String>) {
        self.freed = true;
        self.free_site = free_site;
        
        // Fill data with freed marker (but keep redzones)
        for i in 0..self.size {
            self.data[REDZONE_SIZE + i] = FREED_BYTE;
        }
    }
}

/// Memory configuration
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Enable redzone checks
    pub redzones: bool,
    /// Enable use-after-free detection
    pub use_after_free_detection: bool,
    /// Log all memory operations
    pub trace: bool,
    /// Starting address for heap allocations
    pub heap_start: u64,
    /// Starting address for stack
    pub stack_start: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            redzones: true,
            use_after_free_detection: true,
            trace: false,
            heap_start: 0x1000_0000,
            stack_start: 0x7FFF_0000,
        }
    }
}

/// The main memory manager
pub struct Memory {
    config: MemoryConfig,
    /// All allocations by their ID
    allocations: HashMap<AllocationId, MemoryRegion>,
    /// Map from address to allocation ID (for fast lookup)
    address_map: HashMap<u64, AllocationId>,
    /// Next allocation ID
    next_alloc_id: u64,
    /// Next heap address
    next_heap_addr: u64,
    /// Statistics
    pub stats: MemoryStats,
    /// Memory access log for debugging
    access_log: Vec<MemoryAccess>,
}

/// Statistics about memory usage
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub active_allocations: usize,
    pub freed_allocations: usize,
    pub peak_usage: usize,
    pub current_usage: usize,
}

/// Log entry for memory access
#[derive(Debug, Clone)]
pub struct MemoryAccess {
    pub kind: MemoryAccessKind,
    pub address: Address,
    pub size: usize,
    pub allocation_id: Option<AllocationId>,
    pub instruction_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryAccessKind {
    Allocate,
    Free,
    Read,
    Write,
}

impl Memory {
    pub fn new() -> Self {
        Self::with_config(MemoryConfig::default())
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        Self {
            next_heap_addr: config.heap_start,
            config,
            allocations: HashMap::new(),
            address_map: HashMap::new(),
            next_alloc_id: 1,
            stats: MemoryStats::default(),
            access_log: Vec::new(),
        }
    }

    /// Allocate memory with optional type info for debugging
    pub fn allocate(&mut self, size: usize, type_info: Option<String>) -> Result<Address, String> {
        if size == 0 {
            return Err("Cannot allocate zero bytes".to_string());
        }

        let id = AllocationId(self.next_alloc_id);
        self.next_alloc_id += 1;

        // Align to 8 bytes
        let aligned_size = (size + 7) & !7;
        let base = Address(self.next_heap_addr);
        self.next_heap_addr += (aligned_size + REDZONE_SIZE * 2) as u64;

        let region = MemoryRegion::new(id, base, size, type_info);
        
        self.address_map.insert(base.0, id);
        self.allocations.insert(id, region);

        // Update stats
        self.stats.total_allocated += size;
        self.stats.active_allocations += 1;
        self.stats.current_usage += size;
        if self.stats.current_usage > self.stats.peak_usage {
            self.stats.peak_usage = self.stats.current_usage;
        }

        if self.config.trace {
            self.access_log.push(MemoryAccess {
                kind: MemoryAccessKind::Allocate,
                address: base,
                size,
                allocation_id: Some(id),
                instruction_index: None,
            });
        }

        Ok(base)
    }

    /// Free a previously allocated region
    pub fn free(&mut self, address: Address, free_site: Option<String>) -> Result<(), String> {
        if address.is_null() {
            return Err("Cannot free null pointer".to_string());
        }

        let id = self.address_map.get(&address.0)
            .ok_or_else(|| format!("Invalid free: {} was never allocated", address))?;
        
        let region = self.allocations.get_mut(id)
            .ok_or_else(|| format!("Invalid free: allocation {} not found", id))?;

        if region.freed {
            return Err(format!(
                "Double free detected: {} was already freed at {:?}",
                address, region.free_site
            ));
        }

        // Check redzones before freeing
        if self.config.redzones {
            region.check_redzones()?;
        }

        let size = region.size;
        region.mark_freed(free_site);

        // Update stats
        self.stats.freed_allocations += 1;
        self.stats.active_allocations -= 1;
        self.stats.current_usage -= size;

        if self.config.trace {
            self.access_log.push(MemoryAccess {
                kind: MemoryAccessKind::Free,
                address,
                size,
                allocation_id: Some(*id),
                instruction_index: None,
            });
        }

        Ok(())
    }

    /// Read bytes from memory
    pub fn read(&mut self, address: Address, size: usize) -> Result<Vec<u8>, String> {
        if address.is_null() {
            return Err("Null pointer dereference in read".to_string());
        }

        let (id, offset) = self.find_allocation(address)?;
        let region = self.allocations.get(&id)
            .ok_or_else(|| format!("Allocation {} not found", id))?;

        if self.config.trace {
            self.access_log.push(MemoryAccess {
                kind: MemoryAccessKind::Read,
                address,
                size,
                allocation_id: Some(id),
                instruction_index: None,
            });
        }

        Ok(region.read(offset, size)?.to_vec())
    }

    /// Write bytes to memory
    pub fn write(&mut self, address: Address, bytes: &[u8]) -> Result<(), String> {
        if address.is_null() {
            return Err("Null pointer dereference in write".to_string());
        }

        let (id, offset) = self.find_allocation(address)?;
        let region = self.allocations.get_mut(&id)
            .ok_or_else(|| format!("Allocation {} not found", id))?;

        if self.config.trace {
            self.access_log.push(MemoryAccess {
                kind: MemoryAccessKind::Write,
                address,
                size: bytes.len(),
                allocation_id: Some(id),
                instruction_index: None,
            });
        }

        region.write(offset, bytes)
    }

    /// Find which allocation contains the given address
    fn find_allocation(&self, address: Address) -> Result<(AllocationId, usize), String> {
        // Check if it's a base address
        if let Some(&id) = self.address_map.get(&address.0) {
            return Ok((id, 0));
        }

        // Search for an allocation containing this address
        for (id, region) in &self.allocations {
            let base = region.base.0;
            let end = base + region.size as u64;
            if address.0 >= base && address.0 < end {
                let offset = (address.0 - base) as usize;
                return Ok((*id, offset));
            }
        }

        Err(format!("Address {} is not in any allocation", address))
    }

    /// Read a 64-bit integer from memory
    pub fn read_i64(&mut self, address: Address) -> Result<i64, String> {
        let bytes = self.read(address, 8)?;
        Ok(i64::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Write a 64-bit integer to memory
    pub fn write_i64(&mut self, address: Address, value: i64) -> Result<(), String> {
        self.write(address, &value.to_le_bytes())
    }

    /// Read a 64-bit float from memory
    pub fn read_f64(&mut self, address: Address) -> Result<f64, String> {
        let bytes = self.read(address, 8)?;
        Ok(f64::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Write a 64-bit float to memory
    pub fn write_f64(&mut self, address: Address, value: f64) -> Result<(), String> {
        self.write(address, &value.to_le_bytes())
    }

    /// Read a pointer (address) from memory
    pub fn read_ptr(&mut self, address: Address) -> Result<Address, String> {
        let bytes = self.read(address, 8)?;
        Ok(Address(u64::from_le_bytes(bytes.try_into().unwrap())))
    }

    /// Write a pointer (address) to memory
    pub fn write_ptr(&mut self, address: Address, ptr: Address) -> Result<(), String> {
        self.write(address, &ptr.0.to_le_bytes())
    }

    /// Check all allocations for memory corruption
    pub fn verify_all(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (_, region) in &self.allocations {
            if !region.freed {
                if let Err(e) = region.check_redzones() {
                    errors.push(e);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get active allocations (for leak detection)
    pub fn get_leaks(&self) -> Vec<&MemoryRegion> {
        self.allocations
            .values()
            .filter(|r| !r.freed)
            .collect()
    }

    /// Get the memory access log
    pub fn get_access_log(&self) -> &[MemoryAccess] {
        &self.access_log
    }

    /// Clear the access log
    pub fn clear_access_log(&mut self) {
        self.access_log.clear();
    }

    /// Dump memory state for debugging
    pub fn dump(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Memory Dump ===\n");
        out.push_str(&format!("Active allocations: {}\n", self.stats.active_allocations));
        out.push_str(&format!("Peak usage: {} bytes\n", self.stats.peak_usage));
        out.push_str(&format!("Current usage: {} bytes\n\n", self.stats.current_usage));

        for (id, region) in &self.allocations {
            out.push_str(&format!(
                "{}: base={}, size={}, freed={}, type={:?}\n",
                id, region.base, region.size, region.freed, region.type_info
            ));
        }

        out
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let mut mem = Memory::new();
        let addr = mem.allocate(100, Some("test".to_string())).unwrap();
        assert!(!addr.is_null());
        assert_eq!(mem.stats.active_allocations, 1);
    }

    #[test]
    fn test_read_write() {
        let mut mem = Memory::new();
        let addr = mem.allocate(16, None).unwrap();
        
        mem.write_i64(addr, 42).unwrap();
        assert_eq!(mem.read_i64(addr).unwrap(), 42);
    }

    #[test]
    fn test_out_of_bounds() {
        let mut mem = Memory::new();
        let addr = mem.allocate(8, None).unwrap();
        
        // This should fail - writing past allocation
        let result = mem.write(addr.offset(4), &[0; 8]);
        assert!(result.is_err());
    }

    #[test]
    fn test_use_after_free() {
        let mut mem = Memory::new();
        let addr = mem.allocate(8, None).unwrap();
        mem.free(addr, None).unwrap();
        
        // This should fail - reading from freed memory
        let result = mem.read_i64(addr);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_free() {
        let mut mem = Memory::new();
        let addr = mem.allocate(8, None).unwrap();
        mem.free(addr, None).unwrap();
        
        // This should fail - double free
        let result = mem.free(addr, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_null_pointer() {
        let mut mem = Memory::new();
        
        let result = mem.read_i64(Address::NULL);
        assert!(result.is_err());
    }
}
