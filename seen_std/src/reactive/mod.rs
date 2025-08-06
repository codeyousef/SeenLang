//! Reactive Programming Foundation for Seen
//!
//! Provides high-performance reactive streams with zero-cost abstractions.
//! Designed to achieve <100ns per operator overhead with memory-safe backpressure handling.
//!
//! # Design Principles
//! - Zero-cost abstractions: Operators compile to direct code with no overhead
//! - Memory safety: Automatic subscription cleanup and leak prevention
//! - Backpressure handling: Multiple strategies to prevent memory overflow
//! - Scheduler abstraction: Support for sync, async, and threaded execution
//! - Stream fusion: Operator chains are optimized into single loops where possible
//!
//! # Core Types
//! - `Observable<T>`: Cold observable streams
//! - `Subject<T>`: Hot observable that can be subscribed to and emit values
//! - `BehaviorSubject<T>`: Subject with initial value and current state
//! - `ReplaySubject<T>`: Subject that replays N previous values to new subscribers
//! - `AsyncSubject<T>`: Subject that only emits the final value on completion
//!
//! # Performance Targets
//! - Observable creation: <50ns
//! - Operator chaining: <100ns per operator
//! - Memory overhead: <1KB per stream
//! - Backpressure: No unbounded memory growth
//!
//! # Usage Example
//! ```ignore
//! use seen_std::reactive::*;
//! 
//! let numbers = Observable::range(0, 1000);
//! let subscription = numbers
//!     .filter(|x| x % 2 == 0)
//!     .map(|x| x * 2)
//!     .take(100)
//!     .subscribe(|value| {
//!         println!("Received: {}", value);
//!     });
//! ```

pub mod observable;
pub mod subject;
pub mod operators;
pub mod scheduler;
pub mod subscription;
pub mod backpressure;

#[cfg(test)]
pub mod tests;

// Re-export main types
pub use observable::Observable;
pub use subject::{Subject, BehaviorSubject, ReplaySubject, AsyncSubject};
// pub use operators::*; // Commented out due to trait conflicts
pub use scheduler::{Scheduler, ImmediateScheduler, AsyncScheduler, ThreadPoolScheduler, VirtualTimeScheduler};
pub use subscription::{Subscription, Disposable};
pub use backpressure::{BackpressureStrategy, BackpressureError};

/// Result type for reactive operations
pub type ReactiveResult<T> = Result<T, ReactiveError>;

/// Errors that can occur in reactive operations
#[derive(Debug, Clone)]
pub enum ReactiveError {
    /// Subscription was disposed
    Disposed,
    /// Observer error occurred
    ObserverError(String),
    /// Backpressure limit exceeded
    BackpressureExceeded,
    /// Scheduler error
    SchedulerError(String),
    /// Stream completion error
    CompletionError(String),
}

impl std::fmt::Display for ReactiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReactiveError::Disposed => write!(f, "Subscription was disposed"),
            ReactiveError::ObserverError(msg) => write!(f, "Observer error: {}", msg),
            ReactiveError::BackpressureExceeded => write!(f, "Backpressure limit exceeded"),
            ReactiveError::SchedulerError(msg) => write!(f, "Scheduler error: {}", msg),
            ReactiveError::CompletionError(msg) => write!(f, "Stream completion error: {}", msg),
        }
    }
}

impl std::error::Error for ReactiveError {}

/// Observer trait for receiving values, errors, and completion signals
pub trait Observer<T> {
    /// Called when a new value is emitted
    fn on_next(&mut self, value: T);
    
    /// Called when an error occurs
    fn on_error(&mut self, error: ReactiveError);
    
    /// Called when the stream completes successfully
    fn on_completed(&mut self);
}

/// Function-based observer implementation
pub struct FnObserver<T, F, E, C>
where
    F: FnMut(T),
    E: FnMut(ReactiveError),
    C: FnMut(),
{
    on_next_fn: F,
    on_error_fn: E,
    on_completed_fn: C,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, F, E, C> FnObserver<T, F, E, C>
where
    F: FnMut(T),
    E: FnMut(ReactiveError),
    C: FnMut(),
{
    pub fn new(on_next: F, on_error: E, on_completed: C) -> Self {
        Self {
            on_next_fn: on_next,
            on_error_fn: on_error,
            on_completed_fn: on_completed,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F, E, C> Observer<T> for FnObserver<T, F, E, C>
where
    F: FnMut(T),
    E: FnMut(ReactiveError),
    C: FnMut(),
{
    fn on_next(&mut self, value: T) {
        (self.on_next_fn)(value);
    }
    
    fn on_error(&mut self, error: ReactiveError) {
        (self.on_error_fn)(error);
    }
    
    fn on_completed(&mut self) {
        (self.on_completed_fn)();
    }
}