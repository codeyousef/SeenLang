//! Advanced measurement infrastructure for high-precision benchmarking
//!
//! Provides CPU cycle counting, memory allocation tracking, and other
//! low-level performance metrics for accurate benchmarking

use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};

/// High-precision timing measurement with CPU cycle counting
#[derive(Debug, Clone)]
pub struct PrecisionTimer {
    start_time: Option<Instant>,
    start_cycles: Option<u64>,
    paused_duration: Duration,
    is_paused: bool,
    pause_start: Option<Instant>,
}

impl PrecisionTimer {
    /// Create a new precision timer
    pub fn new() -> Self {
        Self {
            start_time: None,
            start_cycles: None,
            paused_duration: Duration::from_nanos(0),
            is_paused: false,
            pause_start: None,
        }
    }
    
    /// Start the timer
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        #[cfg(target_arch = "x86_64")]
        {
            self.start_cycles = Some(read_cpu_cycles());
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            self.start_cycles = None;
        }
        self.is_paused = false;
    }
    
    /// Pause the timer
    pub fn pause(&mut self) {
        if !self.is_paused && self.start_time.is_some() {
            self.is_paused = true;
            self.pause_start = Some(Instant::now());
        }
    }
    
    /// Resume the timer
    pub fn resume(&mut self) {
        if self.is_paused {
            if let Some(pause_start) = self.pause_start {
                self.paused_duration += pause_start.elapsed();
            }
            self.is_paused = false;
            self.pause_start = None;
        }
    }
    
    /// Get elapsed time since start (excluding paused periods)
    pub fn elapsed(&self) -> Duration {
        if let Some(start) = self.start_time {
            let total_elapsed = start.elapsed();
            let mut paused = self.paused_duration;
            
            // If currently paused, add the current pause time
            if self.is_paused {
                if let Some(pause_start) = self.pause_start {
                    paused += pause_start.elapsed();
                }
            }
            
            total_elapsed.saturating_sub(paused)
        } else {
            Duration::from_nanos(0)
        }
    }
    
    /// Get CPU cycles elapsed (x86_64 only)
    pub fn cycles_elapsed(&self) -> Option<u64> {
        #[cfg(target_arch = "x86_64")]
        {
            if let Some(start_cycles) = self.start_cycles {
                if !self.is_paused {
                    Some(read_cpu_cycles().saturating_sub(start_cycles))
                } else {
                    Some(0)
                }
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            None
        }
    }
}

/// CPU cycle counter using RDTSC instruction on x86_64
#[cfg(target_arch = "x86_64")]
fn read_cpu_cycles() -> u64 {
    unsafe {
        let mut high: u32;
        let mut low: u32;
        
        // Use RDTSC (Read Time-Stamp Counter) instruction
        std::arch::asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack)
        );
        
        ((high as u64) << 32) | (low as u64)
    }
}

/// Memory allocation tracker for benchmarks
#[derive(Debug)]
pub struct MemoryTracker {
    allocations: AtomicU64,
    deallocations: AtomicU64,
    bytes_allocated: AtomicU64,
    bytes_deallocated: AtomicU64,
    peak_memory: AtomicU64,
    current_memory: AtomicU64,
}

impl MemoryTracker {
    /// Create a new memory tracker
    pub fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            bytes_allocated: AtomicU64::new(0),
            bytes_deallocated: AtomicU64::new(0),
            peak_memory: AtomicU64::new(0),
            current_memory: AtomicU64::new(0),
        }
    }
    
    /// Record an allocation
    pub fn record_allocation(&self, size: u64) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_allocated.fetch_add(size, Ordering::Relaxed);
        let current = self.current_memory.fetch_add(size, Ordering::Relaxed) + size;
        
        // Update peak memory usage
        let mut peak = self.peak_memory.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_memory.compare_exchange_weak(
                peak, 
                current, 
                Ordering::Relaxed, 
                Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }
    
    /// Record a deallocation
    pub fn record_deallocation(&self, size: u64) {
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.bytes_deallocated.fetch_add(size, Ordering::Relaxed);
        self.current_memory.fetch_sub(size, Ordering::Relaxed);
    }
    
    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            allocations: self.allocations.load(Ordering::Relaxed),
            deallocations: self.deallocations.load(Ordering::Relaxed),
            bytes_allocated: self.bytes_allocated.load(Ordering::Relaxed),
            bytes_deallocated: self.bytes_deallocated.load(Ordering::Relaxed),
            peak_memory: self.peak_memory.load(Ordering::Relaxed),
            current_memory: self.current_memory.load(Ordering::Relaxed),
        }
    }
    
    /// Reset all counters to zero
    pub fn reset(&self) {
        self.allocations.store(0, Ordering::Relaxed);
        self.deallocations.store(0, Ordering::Relaxed);
        self.bytes_allocated.store(0, Ordering::Relaxed);
        self.bytes_deallocated.store(0, Ordering::Relaxed);
        self.peak_memory.store(0, Ordering::Relaxed);
        self.current_memory.store(0, Ordering::Relaxed);
    }
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub allocations: u64,
    pub deallocations: u64,
    pub bytes_allocated: u64,
    pub bytes_deallocated: u64,
    pub peak_memory: u64,
    pub current_memory: u64,
}

impl MemoryStats {
    /// Calculate net memory usage
    pub fn net_bytes(&self) -> i64 {
        self.bytes_allocated as i64 - self.bytes_deallocated as i64
    }
    
    /// Calculate allocation efficiency (deallocations / allocations)
    pub fn efficiency(&self) -> f64 {
        if self.allocations == 0 {
            1.0
        } else {
            self.deallocations as f64 / self.allocations as f64
        }
    }
    
    /// Average allocation size
    pub fn avg_allocation_size(&self) -> f64 {
        if self.allocations == 0 {
            0.0
        } else {
            self.bytes_allocated as f64 / self.allocations as f64
        }
    }
}

/// Comprehensive benchmark measurement including advanced metrics
#[derive(Debug, Clone)]
pub struct AdvancedMeasurement {
    pub name: String,
    pub wall_time: Duration,
    pub cpu_cycles: Option<u64>,
    pub memory_stats: Option<MemoryStats>,
    pub custom_metrics: Vec<(String, f64)>,
}

impl AdvancedMeasurement {
    /// Create a new advanced measurement
    pub fn new(name: String, wall_time: Duration) -> Self {
        Self {
            name,
            wall_time,
            cpu_cycles: None,
            memory_stats: None,
            custom_metrics: Vec::new(),
        }
    }
    
    /// Add CPU cycle information
    pub fn with_cpu_cycles(mut self, cycles: u64) -> Self {
        self.cpu_cycles = Some(cycles);
        self
    }
    
    /// Add memory statistics
    pub fn with_memory_stats(mut self, stats: MemoryStats) -> Self {
        self.memory_stats = Some(stats);
        self
    }
    
    /// Add custom metric
    pub fn add_custom_metric(mut self, name: String, value: f64) -> Self {
        self.custom_metrics.push((name, value));
        self
    }
    
    /// Calculate instructions per cycle (if CPU frequency is known)
    pub fn instructions_per_cycle(&self, instructions: u64) -> Option<f64> {
        self.cpu_cycles.map(|cycles| {
            if cycles == 0 {
                0.0
            } else {
                instructions as f64 / cycles as f64
            }
        })
    }
    
    /// Calculate cycles per second (approximate CPU frequency)
    pub fn cycles_per_second(&self) -> Option<f64> {
        self.cpu_cycles.map(|cycles| {
            cycles as f64 / self.wall_time.as_secs_f64()
        })
    }
}

impl Default for PrecisionTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_precision_timer() {
        let mut timer = PrecisionTimer::new();
        timer.start();
        
        thread::sleep(Duration::from_millis(10));
        
        let elapsed = timer.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
        assert!(elapsed < Duration::from_millis(20)); // Should be close
        
        #[cfg(target_arch = "x86_64")]
        {
            let cycles = timer.cycles_elapsed();
            assert!(cycles.is_some());
            assert!(cycles.unwrap() > 0);
        }
    }
    
    #[test]
    fn test_timer_pause_resume() {
        let mut timer = PrecisionTimer::new();
        timer.start();
        
        thread::sleep(Duration::from_millis(5));
        timer.pause();
        
        thread::sleep(Duration::from_millis(10)); // This should not be counted
        timer.resume();
        
        thread::sleep(Duration::from_millis(5));
        
        let elapsed = timer.elapsed();
        // Should be approximately 10ms, not 20ms
        assert!(elapsed >= Duration::from_millis(8));
        assert!(elapsed < Duration::from_millis(15));
    }
    
    #[test]
    fn test_memory_tracker() {
        let tracker = MemoryTracker::new();
        
        tracker.record_allocation(1024);
        tracker.record_allocation(512);
        tracker.record_deallocation(256);
        
        let stats = tracker.stats();
        assert_eq!(stats.allocations, 2);
        assert_eq!(stats.deallocations, 1);
        assert_eq!(stats.bytes_allocated, 1536);
        assert_eq!(stats.bytes_deallocated, 256);
        assert_eq!(stats.current_memory, 1280);
        assert_eq!(stats.peak_memory, 1536);
        assert_eq!(stats.net_bytes(), 1280);
    }
    
    #[test]
    fn test_memory_stats_calculations() {
        let stats = MemoryStats {
            allocations: 10,
            deallocations: 8,
            bytes_allocated: 10000,
            bytes_deallocated: 8000,
            peak_memory: 3000,
            current_memory: 2000,
        };
        
        assert_eq!(stats.net_bytes(), 2000);
        assert_eq!(stats.efficiency(), 0.8);
        assert_eq!(stats.avg_allocation_size(), 1000.0);
    }
    
    #[test]
    fn test_advanced_measurement() {
        let measurement = AdvancedMeasurement::new(
            "test_benchmark".to_string(),
            Duration::from_millis(100)
        )
        .with_cpu_cycles(1000000)
        .add_custom_metric("cache_misses".to_string(), 50.0);
        
        assert_eq!(measurement.name, "test_benchmark");
        assert_eq!(measurement.wall_time, Duration::from_millis(100));
        assert_eq!(measurement.cpu_cycles, Some(1000000));
        assert_eq!(measurement.custom_metrics.len(), 1);
        
        let ipc = measurement.instructions_per_cycle(500000);
        assert_eq!(ipc, Some(0.5)); // 500k instructions / 1M cycles = 0.5 IPC
    }
}