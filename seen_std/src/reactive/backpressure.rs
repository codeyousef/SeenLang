//! Backpressure handling for reactive streams
//!
//! Backpressure occurs when a producer emits values faster than a consumer can process them.
//! This module provides various strategies for handling backpressure to prevent memory overflow.

use super::{ReactiveError, Observer};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Backpressure strategies
#[derive(Debug, Clone)]
pub enum BackpressureStrategy {
    /// Drop the oldest items when buffer is full
    DropOldest(usize),
    /// Drop the newest items when buffer is full
    DropNewest(usize),
    /// Buffer up to a limit, then error
    Error(usize),
    /// Throttle emissions to a maximum rate
    Throttle(Duration),
    /// Sample values at regular intervals
    Sample(Duration),
    /// Apply backpressure by blocking the producer
    Block(usize),
}

/// Error types for backpressure handling
#[derive(Debug, Clone)]
pub enum BackpressureError {
    BufferOverflow,
    RateLimitExceeded,
    ProducerBlocked,
}

impl std::fmt::Display for BackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackpressureError::BufferOverflow => write!(f, "Buffer overflow"),
            BackpressureError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            BackpressureError::ProducerBlocked => write!(f, "Producer blocked"),
        }
    }
}

impl std::error::Error for BackpressureError {}

/// Buffered observer that applies backpressure strategies
pub struct BackpressureObserver<T> {
    inner_observer: Box<dyn Observer<T> + Send + Sync>,
    strategy: BackpressureStrategy,
    buffer: Arc<Mutex<VecDeque<T>>>,
    last_emission: Arc<Mutex<Option<Instant>>>,
    last_sample: Arc<Mutex<Option<Instant>>>,
}

impl<T: Clone + Send + Sync> BackpressureObserver<T> {
    /// Create a new BackpressureObserver with the given strategy
    pub fn new(
        observer: Box<dyn Observer<T> + Send + Sync>,
        strategy: BackpressureStrategy,
    ) -> Self {
        Self {
            inner_observer: observer,
            strategy,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            last_emission: Arc::new(Mutex::new(None)),
            last_sample: Arc::new(Mutex::new(None)),
        }
    }

    /// Apply the configured backpressure strategy
    fn apply_backpressure(&mut self, value: T) -> Result<(), BackpressureError> {
        match self.strategy.clone() {
            BackpressureStrategy::DropOldest(max_size) => {
                self.handle_drop_oldest(value, max_size)
            }
            BackpressureStrategy::DropNewest(max_size) => {
                self.handle_drop_newest(value, max_size)
            }
            BackpressureStrategy::Error(max_size) => {
                self.handle_error_on_overflow(value, max_size)
            }
            BackpressureStrategy::Throttle(min_interval) => {
                self.handle_throttle(value, min_interval)
            }
            BackpressureStrategy::Sample(interval) => {
                self.handle_sample(value, interval)
            }
            BackpressureStrategy::Block(max_size) => {
                self.handle_block(value, max_size)
            }
        }
    }

    fn handle_drop_oldest(&mut self, value: T, max_size: usize) -> Result<(), BackpressureError> {
        let mut buffer = self.buffer.lock().unwrap();
        
        if buffer.len() >= max_size {
            buffer.pop_front(); // Drop oldest
        }
        
        buffer.push_back(value.clone());
        drop(buffer);
        
        self.inner_observer.on_next(value);
        Ok(())
    }

    fn handle_drop_newest(&mut self, value: T, max_size: usize) -> Result<(), BackpressureError> {
        let mut buffer = self.buffer.lock().unwrap();
        
        if buffer.len() >= max_size {
            // Drop the new value
            return Ok(());
        }
        
        buffer.push_back(value.clone());
        drop(buffer);
        
        self.inner_observer.on_next(value);
        Ok(())
    }

    fn handle_error_on_overflow(&mut self, value: T, max_size: usize) -> Result<(), BackpressureError> {
        let mut buffer = self.buffer.lock().unwrap();
        
        if buffer.len() >= max_size {
            drop(buffer);
            return Err(BackpressureError::BufferOverflow);
        }
        
        buffer.push_back(value.clone());
        drop(buffer);
        
        self.inner_observer.on_next(value);
        Ok(())
    }

    fn handle_throttle(&mut self, value: T, min_interval: Duration) -> Result<(), BackpressureError> {
        let now = Instant::now();
        let mut last_emission = self.last_emission.lock().unwrap();
        
        match *last_emission {
            Some(last) if now.duration_since(last) < min_interval => {
                // Too soon, drop this emission
                return Ok(());
            }
            _ => {
                *last_emission = Some(now);
                drop(last_emission);
                self.inner_observer.on_next(value);
                Ok(())
            }
        }
    }

    fn handle_sample(&mut self, value: T, interval: Duration) -> Result<(), BackpressureError> {
        let now = Instant::now();
        let mut last_sample = self.last_sample.lock().unwrap();
        
        // Always store the latest value
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
        buffer.push_back(value);
        drop(buffer);
        
        match *last_sample {
            Some(last) if now.duration_since(last) < interval => {
                // Not time to sample yet
                return Ok(());
            }
            _ => {
                *last_sample = Some(now);
                drop(last_sample);
                
                // Emit the latest value
                let buffer = self.buffer.lock().unwrap();
                if let Some(latest_value) = buffer.back() {
                    self.inner_observer.on_next(latest_value.clone());
                }
                Ok(())
            }
        }
    }

    fn handle_block(&mut self, value: T, max_size: usize) -> Result<(), BackpressureError> {
        let mut buffer = self.buffer.lock().unwrap();
        
        if buffer.len() >= max_size {
            drop(buffer);
            // In a real implementation, this would block until space is available
            // For now, we return an error
            return Err(BackpressureError::ProducerBlocked);
        }
        
        buffer.push_back(value.clone());
        drop(buffer);
        
        self.inner_observer.on_next(value);
        Ok(())
    }

    /// Flush any buffered items
    pub fn flush(&mut self) {
        let mut buffer = self.buffer.lock().unwrap();
        while let Some(value) = buffer.pop_front() {
            self.inner_observer.on_next(value);
        }
    }

    /// Get current buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }
}

impl<T: Clone + Send + Sync> Observer<T> for BackpressureObserver<T> {
    fn on_next(&mut self, value: T) {
        match self.apply_backpressure(value) {
            Ok(()) => {}
            Err(err) => {
                let reactive_error = match err {
                    BackpressureError::BufferOverflow => ReactiveError::BackpressureExceeded,
                    BackpressureError::RateLimitExceeded => ReactiveError::BackpressureExceeded,
                    BackpressureError::ProducerBlocked => ReactiveError::BackpressureExceeded,
                };
                self.inner_observer.on_error(reactive_error);
            }
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner_observer.on_error(error);
    }

    fn on_completed(&mut self) {
        // Flush any remaining items before completing
        self.flush();
        self.inner_observer.on_completed();
    }
}

/// Builder for creating backpressure observers
pub struct BackpressureBuilder<T> {
    strategy: Option<BackpressureStrategy>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> BackpressureBuilder<T> {
    pub fn new() -> Self {
        Self {
            strategy: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set strategy to drop oldest items when buffer is full
    pub fn drop_oldest(mut self, max_size: usize) -> Self {
        self.strategy = Some(BackpressureStrategy::DropOldest(max_size));
        self
    }

    /// Set strategy to drop newest items when buffer is full
    pub fn drop_newest(mut self, max_size: usize) -> Self {
        self.strategy = Some(BackpressureStrategy::DropNewest(max_size));
        self
    }

    /// Set strategy to error when buffer overflows
    pub fn error_on_overflow(mut self, max_size: usize) -> Self {
        self.strategy = Some(BackpressureStrategy::Error(max_size));
        self
    }

    /// Set strategy to throttle emissions
    pub fn throttle(mut self, min_interval: Duration) -> Self {
        self.strategy = Some(BackpressureStrategy::Throttle(min_interval));
        self
    }

    /// Set strategy to sample at regular intervals
    pub fn sample(mut self, interval: Duration) -> Self {
        self.strategy = Some(BackpressureStrategy::Sample(interval));
        self
    }

    /// Set strategy to block producer when buffer is full
    pub fn block(mut self, max_size: usize) -> Self {
        self.strategy = Some(BackpressureStrategy::Block(max_size));
        self
    }

    /// Build the backpressure observer
    pub fn build(self, observer: Box<dyn Observer<T> + Send + Sync>) -> BackpressureObserver<T>
    where
        T: Clone + Send + Sync,
    {
        let strategy = self.strategy.unwrap_or(BackpressureStrategy::Error(1000));
        BackpressureObserver::new(observer, strategy)
    }
}

impl<T> Default for BackpressureBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::FnObserver;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_drop_oldest_strategy() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observer = FnObserver::new(
            move |value: i32| {
                received_clone.lock().unwrap().push(value);
            },
            |_| {},
            || {},
        );

        let mut backpressure_observer = BackpressureObserver::new(
            Box::new(observer),
            BackpressureStrategy::DropOldest(2),
        );

        backpressure_observer.on_next(1);
        backpressure_observer.on_next(2);
        backpressure_observer.on_next(3); // Should drop 1
        backpressure_observer.on_next(4); // Should drop 2

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 2, 3, 4]); // All values are still emitted to observer
        assert_eq!(backpressure_observer.buffer_size(), 2); // But buffer maintains size
    }

    #[test]
    fn test_drop_newest_strategy() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observer = FnObserver::new(
            move |value: i32| {
                received_clone.lock().unwrap().push(value);
            },
            |_| {},
            || {},
        );

        let mut backpressure_observer = BackpressureObserver::new(
            Box::new(observer),
            BackpressureStrategy::DropNewest(2),
        );

        backpressure_observer.on_next(1);
        backpressure_observer.on_next(2);
        backpressure_observer.on_next(3); // Should be dropped
        backpressure_observer.on_next(4); // Should be dropped

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 2]); // Only first 2 values emitted
    }

    #[test]
    fn test_error_on_overflow_strategy() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);
        let errors_received = Arc::new(AtomicU32::new(0));
        let errors_received_clone = Arc::clone(&errors_received);

        let observer = FnObserver::new(
            move |value: i32| {
                received_clone.lock().unwrap().push(value);
            },
            move |_| {
                errors_received_clone.fetch_add(1, Ordering::SeqCst);
            },
            || {},
        );

        let mut backpressure_observer = BackpressureObserver::new(
            Box::new(observer),
            BackpressureStrategy::Error(2),
        );

        backpressure_observer.on_next(1);
        backpressure_observer.on_next(2);
        backpressure_observer.on_next(3); // Should cause error

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 2]); // Only first 2 values emitted
        assert_eq!(errors_received.load(Ordering::SeqCst), 1); // One error
    }

    #[test]
    fn test_throttle_strategy() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observer = FnObserver::new(
            move |value: i32| {
                received_clone.lock().unwrap().push(value);
            },
            |_| {},
            || {},
        );

        let mut backpressure_observer = BackpressureObserver::new(
            Box::new(observer),
            BackpressureStrategy::Throttle(Duration::from_millis(100)),
        );

        backpressure_observer.on_next(1);
        backpressure_observer.on_next(2); // Should be throttled
        backpressure_observer.on_next(3); // Should be throttled

        std::thread::sleep(Duration::from_millis(150));
        backpressure_observer.on_next(4); // Should be emitted

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 4]); // Only non-throttled values
    }

    #[test]
    fn test_backpressure_builder() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observer = FnObserver::new(
            move |value: i32| {
                received_clone.lock().unwrap().push(value);
            },
            |_| {},
            || {},
        );

        let mut backpressure_observer = BackpressureBuilder::new()
            .drop_oldest(2)
            .build(Box::new(observer));

        backpressure_observer.on_next(1);
        backpressure_observer.on_next(2);
        backpressure_observer.on_next(3);

        assert_eq!(backpressure_observer.buffer_size(), 2);
    }
}