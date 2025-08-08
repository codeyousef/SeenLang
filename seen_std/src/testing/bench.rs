//! Benchmarking infrastructure for performance testing
//!
//! High-precision timing and statistical analysis for Seen programs

use crate::string::String;
use crate::collections::Vec;
use std::time::{Duration, Instant};

/// Bencher type for micro-benchmarking with iteration control
/// Provides the familiar `b.iter { ... }` API from Rust benchmarking
#[derive(Debug)]
pub struct Bencher {
    measurement_time: Duration,
    warmup_iterations: usize,
    sample_size: usize,
    bytes_processed: Option<u64>,
    timer_paused: bool,
    elapsed_time: Duration,
}

impl Bencher {
    /// Create a new Bencher with default configuration
    pub fn new() -> Self {
        Self {
            measurement_time: Duration::from_secs(5),
            warmup_iterations: 10,
            sample_size: 100,
            bytes_processed: None,
            timer_paused: false,
            elapsed_time: Duration::from_nanos(0),
        }
    }
    
    /// Set the number of bytes processed per iteration (for throughput measurement)
    pub fn bytes(&mut self, n: u64) {
        self.bytes_processed = Some(n);
    }
    
    /// Pause the timer (useful for setup/teardown that shouldn't be measured)
    pub fn pause_timer(&mut self) {
        self.timer_paused = true;
    }
    
    /// Resume the timer
    pub fn resume_timer(&mut self) {
        self.timer_paused = false;
    }
    
    /// Benchmark a closure with automatic iteration control
    pub fn iter<F, T>(&mut self, mut routine: F) -> BenchMeasurement
    where
        F: FnMut() -> T,
    {
        // Warmup phase
        for _ in 0..self.warmup_iterations {
            let _ = routine();
        }
        
        let mut samples = Vec::new();
        let bench_start = Instant::now();
        
        while samples.len() < self.sample_size && bench_start.elapsed() < self.measurement_time {
            let start = Instant::now();
            let _ = routine();
            let elapsed = start.elapsed();
            samples.push(elapsed);
        }
        
        let mut measurement = BenchMeasurement::new(String::from("benchmark"), samples);
        
        // Add throughput information if bytes were specified
        if let Some(bytes) = self.bytes_processed {
            measurement = measurement.with_throughput(bytes);
        }
        
        measurement
    }
    
    /// Benchmark with custom setup and teardown for each iteration
    pub fn iter_batched<S, F, T>(&mut self, mut setup: S, mut routine: F) -> BenchMeasurement
    where
        S: FnMut() -> T,
        F: FnMut(T),
    {
        // Warmup phase
        for _ in 0..self.warmup_iterations {
            let input = setup();
            routine(input);
        }
        
        let mut samples = Vec::new();
        let bench_start = Instant::now();
        
        while samples.len() < self.sample_size && bench_start.elapsed() < self.measurement_time {
            let input = setup();
            
            let start = Instant::now();
            routine(input);
            let elapsed = start.elapsed();
            samples.push(elapsed);
        }
        
        let mut measurement = BenchMeasurement::new(String::from("benchmark"), samples);
        
        if let Some(bytes) = self.bytes_processed {
            measurement = measurement.with_throughput(bytes);
        }
        
        measurement
    }
    
    /// Configure measurement time
    pub fn with_measurement_time(mut self, duration: Duration) -> Self {
        self.measurement_time = duration;
        self
    }
    
    /// Configure warmup iterations
    pub fn with_warmup_iterations(mut self, iterations: usize) -> Self {
        self.warmup_iterations = iterations;
        self
    }
    
    /// Configure sample size
    pub fn with_sample_size(mut self, size: usize) -> Self {
        self.sample_size = size;
        self
    }
}

impl Default for Bencher {
    fn default() -> Self {
        Self::new()
    }
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchConfig {
    pub sample_size: usize,
    pub warmup_iterations: usize,
    pub measurement_time: Duration,
    pub confidence_level: f64, // 0.95 for 95% confidence
    pub significance_level: f64, // 0.05 for 5% significance
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            sample_size: 100,
            warmup_iterations: 10,
            measurement_time: Duration::from_secs(5),
            confidence_level: 0.95,
            significance_level: 0.05,
        }
    }
}

/// Benchmark measurement result
#[derive(Debug, Clone)]
pub struct BenchMeasurement {
    pub name: String,
    pub samples: Vec<Duration>,
    pub mean: Duration,
    pub std_dev: Duration,
    pub min: Duration,
    pub max: Duration,
    pub percentiles: BenchPercentiles,
    pub throughput: Option<f64>, // Operations per second
}

/// Statistical percentiles for benchmark results
#[derive(Debug, Clone)]
pub struct BenchPercentiles {
    pub p50: Duration, // Median
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p99_9: Duration,
}

impl BenchMeasurement {
    pub fn new(name: String, mut samples: Vec<Duration>) -> Self {
        if samples.is_empty() {
            panic!("Cannot create benchmark measurement with no samples");
        }
        
        samples.sort();
        
        let mean = calculate_mean(&samples);
        let std_dev = calculate_std_dev(&samples, mean);
        let min = *samples.first().unwrap();
        let max = *samples.last().unwrap();
        let percentiles = calculate_percentiles(&samples);
        
        Self {
            name,
            samples,
            mean,
            std_dev,
            min,
            max,
            percentiles,
            throughput: None,
        }
    }
    
    pub fn with_throughput(mut self, operations_per_iteration: u64) -> Self {
        let ops_per_second = operations_per_iteration as f64 / self.mean.as_secs_f64();
        self.throughput = Some(ops_per_second);
        self
    }
    
    pub fn coefficient_of_variation(&self) -> f64 {
        self.std_dev.as_secs_f64() / self.mean.as_secs_f64()
    }
    
    pub fn is_stable(&self) -> bool {
        // Consider stable if coefficient of variation is less than 10%
        self.coefficient_of_variation() < 0.1
    }
}

/// Benchmark runner for performance testing
pub struct BenchRunner {
    config: BenchConfig,
}

impl BenchRunner {
    pub fn new(config: BenchConfig) -> Self {
        Self { config }
    }
    
    pub fn with_default_config() -> Self {
        Self::new(BenchConfig::default())
    }
    
    /// Run a benchmark function multiple times and collect statistics
    pub fn bench<F>(&self, name: &str, mut bench_fn: F) -> BenchMeasurement
    where
        F: FnMut(),
    {
        // Warmup iterations
        for _ in 0..self.config.warmup_iterations {
            bench_fn();
        }
        
        let mut samples = Vec::new();
        let start_time = Instant::now();
        
        // Collect samples until we have enough or time runs out
        while samples.len() < self.config.sample_size && start_time.elapsed() < self.config.measurement_time {
            let sample_start = Instant::now();
            bench_fn();
            let sample_duration = sample_start.elapsed();
            samples.push(sample_duration);
        }
        
        BenchMeasurement::new(String::from(name), samples)
    }
    
    /// Run a throughput benchmark (measures operations per second)
    pub fn bench_throughput<F>(&self, name: &str, operations_per_iteration: u64, bench_fn: F) -> BenchMeasurement
    where
        F: FnMut(),
    {
        self.bench(name, bench_fn)
            .with_throughput(operations_per_iteration)
    }
    
    /// Compare two benchmark results for regression detection
    pub fn compare_benchmarks(&self, baseline: &BenchMeasurement, current: &BenchMeasurement) -> BenchComparison {
        let mean_change = (current.mean.as_secs_f64() - baseline.mean.as_secs_f64()) / baseline.mean.as_secs_f64();
        let is_regression = mean_change > self.config.significance_level;
        let is_improvement = mean_change < -self.config.significance_level;
        
        let throughput_change = if let (Some(baseline_tput), Some(current_tput)) = 
            (baseline.throughput, current.throughput) {
            Some((current_tput - baseline_tput) / baseline_tput)
        } else {
            None
        };
        
        BenchComparison {
            baseline_name: baseline.name.clone(),
            current_name: current.name.clone(),
            mean_change,
            throughput_change,
            is_regression,
            is_improvement,
            confidence_interval: self.calculate_confidence_interval(baseline, current),
        }
    }
    
    /// Calculate confidence interval for performance difference
    fn calculate_confidence_interval(&self, baseline: &BenchMeasurement, current: &BenchMeasurement) -> (f64, f64) {
        // Simplified confidence interval calculation
        // In a full implementation, this would use t-distribution
        let pooled_std_dev = ((baseline.std_dev.as_secs_f64().powi(2) + current.std_dev.as_secs_f64().powi(2)) / 2.0).sqrt();
        let std_error = pooled_std_dev * (2.0 / baseline.samples.len() as f64).sqrt();
        let margin_of_error = 1.96 * std_error; // 95% confidence assuming normal distribution
        
        let mean_diff = current.mean.as_secs_f64() - baseline.mean.as_secs_f64();
        (mean_diff - margin_of_error, mean_diff + margin_of_error)
    }
}

/// Benchmark comparison result
#[derive(Debug)]
pub struct BenchComparison {
    pub baseline_name: String,
    pub current_name: String,
    pub mean_change: f64, // Percentage change in mean time
    pub throughput_change: Option<f64>, // Percentage change in throughput
    pub is_regression: bool,
    pub is_improvement: bool,
    pub confidence_interval: (f64, f64), // Lower and upper bounds for difference
}

impl BenchComparison {
    pub fn print_summary(&self) {
        println!("Benchmark comparison: {} vs {}", self.baseline_name, self.current_name);
        
        if self.is_regression {
            println!("  ⚠️  REGRESSION DETECTED: {:.2}% slower", self.mean_change * 100.0);
        } else if self.is_improvement {
            println!("  ✅ IMPROVEMENT: {:.2}% faster", -self.mean_change * 100.0);
        } else {
            println!("  ➡️  No significant change ({:.2}%)", self.mean_change * 100.0);
        }
        
        if let Some(tput_change) = self.throughput_change {
            println!("  Throughput change: {:.2}%", tput_change * 100.0);
        }
        
        println!("  95% confidence interval: [{:.3}s, {:.3}s]", 
                self.confidence_interval.0, self.confidence_interval.1);
    }
}

// Statistical helper functions
fn calculate_mean(samples: &[Duration]) -> Duration {
    let sum: u128 = samples.iter().map(|d| d.as_nanos()).sum();
    Duration::from_nanos((sum / samples.len() as u128) as u64)
}

fn calculate_std_dev(samples: &[Duration], mean: Duration) -> Duration {
    let variance: f64 = samples.iter()
        .map(|d| {
            let diff = d.as_secs_f64() - mean.as_secs_f64();
            diff * diff
        })
        .sum::<f64>() / samples.len() as f64;
    
    Duration::from_secs_f64(variance.sqrt())
}

fn calculate_percentiles(sorted_samples: &[Duration]) -> BenchPercentiles {
    fn percentile(samples: &[Duration], p: f64) -> Duration {
        let index = (samples.len() as f64 * p / 100.0) as usize;
        let clamped_index = index.min(samples.len() - 1);
        samples[clamped_index]
    }
    
    BenchPercentiles {
        p50: percentile(sorted_samples, 50.0),
        p90: percentile(sorted_samples, 90.0),
        p95: percentile(sorted_samples, 95.0),
        p99: percentile(sorted_samples, 99.0),
        p99_9: percentile(sorted_samples, 99.9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_bench_runner_basic() {
        let runner = BenchRunner::with_default_config();
        let measurement = runner.bench("test_sleep", || {
            thread::sleep(Duration::from_millis(1));
        });
        
        assert_eq!(measurement.name, "test_sleep");
        assert!(measurement.mean >= Duration::from_millis(1));
        assert!(measurement.samples.len() > 0);
        assert!(measurement.min <= measurement.mean);
        assert!(measurement.max >= measurement.mean);
    }
    
    #[test]
    fn test_throughput_measurement() {
        let runner = BenchRunner::with_default_config();
        let measurement = runner.bench_throughput("test_ops", 1000, || {
            // Simulate processing 1000 operations
            thread::sleep(Duration::from_micros(100));
        });
        
        assert!(measurement.throughput.is_some());
        assert!(measurement.throughput.unwrap() > 0.0);
    }
    
    #[test]
    fn test_measurement_statistics() {
        let mut samples = Vec::new();
        samples.push(Duration::from_nanos(100));
        samples.push(Duration::from_nanos(200));
        samples.push(Duration::from_nanos(300));
        samples.push(Duration::from_nanos(400));
        samples.push(Duration::from_nanos(500));
        
        let measurement = BenchMeasurement::new(String::from("test"), samples);
        assert_eq!(measurement.mean, Duration::from_nanos(300));
        assert_eq!(measurement.min, Duration::from_nanos(100));
        assert_eq!(measurement.max, Duration::from_nanos(500));
        assert_eq!(measurement.percentiles.p50, Duration::from_nanos(300));
    }
    
    #[test]
    fn test_bencher_iter() {
        let mut bencher = Bencher::new()
            .with_sample_size(10)
            .with_measurement_time(Duration::from_millis(100));
        
        let measurement = bencher.iter(|| {
            // Simple benchmark: sleep for 1ms
            thread::sleep(Duration::from_millis(1));
        });
        
        assert_eq!(measurement.name, "benchmark");
        assert!(measurement.samples.len() > 0);
        assert!(measurement.mean >= Duration::from_millis(1));
        assert!(measurement.min <= measurement.mean);
        assert!(measurement.max >= measurement.mean);
    }
    
    #[test]
    fn test_bencher_iter_batched() {
        let mut bencher = Bencher::new()
            .with_sample_size(5)
            .with_measurement_time(Duration::from_millis(50));
        
        let measurement = bencher.iter_batched(
            || vec![1, 2, 3, 4, 5], // Setup: create input data
            |mut data| {             // Benchmark: sort the data
                data.sort();
            }
        );
        
        assert!(measurement.samples.len() > 0);
        assert!(measurement.mean > Duration::from_nanos(0));
    }
    
    #[test]
    fn test_bencher_throughput() {
        let mut bencher = Bencher::new()
            .with_sample_size(5)
            .with_measurement_time(Duration::from_millis(50));
        
        bencher.bytes(1024); // Process 1KB per iteration
        
        let measurement = bencher.iter(|| {
            thread::sleep(Duration::from_micros(100)); // Simulate 1KB processing
        });
        
        assert!(measurement.throughput.is_some());
        assert!(measurement.throughput.unwrap() > 0.0);
    }
}