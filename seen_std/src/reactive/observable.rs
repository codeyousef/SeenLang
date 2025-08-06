//! Observable implementation - the core of reactive streams
//!
//! Observables represent lazy streams of data that can be subscribed to.
//! They are "cold" by default, meaning they don't emit values until subscribed to.

use super::{Observer, ReactiveError, Subscription};
use std::sync::Arc;
use std::time::Duration;

/// Observable represents a stream of values that can be observed
pub struct Observable<T> {
    subscribe_fn: Arc<dyn Fn(Box<dyn Observer<T> + Send + Sync>) -> Subscription + Send + Sync>,
}

impl<T: 'static + Send + Sync> Observable<T> {
    /// Create a new Observable from a subscription function
    pub fn new<F>(subscribe_fn: F) -> Self
    where
        F: Fn(Box<dyn Observer<T> + Send + Sync>) -> Subscription + Send + Sync + 'static,
    {
        Self {
            subscribe_fn: Arc::new(subscribe_fn),
        }
    }

    /// Subscribe to this Observable with an observer
    pub fn subscribe<O>(&self, observer: O) -> Subscription
    where
        O: Observer<T> + Send + Sync + 'static,
    {
        (self.subscribe_fn)(Box::new(observer))
    }

    /// Subscribe with a boxed observer (for internal use)
    pub fn subscribe_boxed(&self, observer: Box<dyn Observer<T> + Send + Sync>) -> Subscription {
        (self.subscribe_fn)(observer)
    }

    /// Subscribe with just an on_next function (convenience method)
    pub fn subscribe_fn<F>(&self, on_next: F) -> Subscription
    where
        F: FnMut(T) + Send + Sync + 'static,
    {
        use super::FnObserver;
        let observer = FnObserver::new(
            on_next,
            |_error| {}, // Default error handler
            || {},       // Default completion handler
        );
        self.subscribe(observer)
    }

    /// Create an Observable that emits a single value
    pub fn just(value: T) -> Self
    where
        T: Clone,
    {
        let value = Arc::new(value);
        Self::new(move |mut observer| {
            let value_clone = Arc::clone(&value);
            observer.on_next((*value_clone).clone());
            observer.on_completed();
            Subscription::empty()
        })
    }

    /// Create an Observable that emits values from an iterator
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Clone,
    {
        let values: Vec<T> = iter.into_iter().collect();
        Self::new(move |mut observer| {
            for value in values.iter() {
                observer.on_next(value.clone());
            }
            observer.on_completed();
            Subscription::empty()
        })
    }


    /// Create an Observable that emits values at regular intervals
    pub fn interval(period: Duration) -> Observable<u64>
    where
        T: From<u64>,
    {
        Observable::new(move |mut observer| {
            // TODO: Implement actual timer-based emission
            // For now, this is a placeholder that emits a few values
            for i in 0..5 {
                observer.on_next(i);
                // In real implementation, this would use a timer
                std::thread::sleep(period);
            }
            observer.on_completed();
            Subscription::empty()
        })
    }

    /// Create an Observable that never emits any values
    pub fn never() -> Self {
        Self::new(|_observer| {
            // Never call anything on the observer
            Subscription::empty()
        })
    }

    /// Create an Observable that immediately completes without emitting values
    pub fn empty() -> Self {
        Self::new(|mut observer| {
            observer.on_completed();
            Subscription::empty()
        })
    }

    /// Create an Observable that immediately emits an error
    pub fn error(error: ReactiveError) -> Self {
        Self::new(move |mut observer| {
            observer.on_error(error.clone());
            Subscription::empty()
        })
    }
}

impl<T> Clone for Observable<T> {
    fn clone(&self) -> Self {
        Self {
            subscribe_fn: Arc::clone(&self.subscribe_fn),
        }
    }
}

impl Observable<i32> {
    /// Create an Observable that emits a range of integers
    pub fn range(start: i32, count: i32) -> Observable<i32> {
        Observable::new(move |mut observer| {
            for i in start..(start + count) {
                observer.on_next(i);
            }
            observer.on_completed();
            Subscription::empty()
        })
    }
}

// Thread safety markers
unsafe impl<T: Send> Send for Observable<T> {}
unsafe impl<T: Sync> Sync for Observable<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_observable_creation() {
        let _obs = Observable::just(42);
        // Basic creation should not panic
    }

    #[test]
    fn test_observable_just_subscription() {
        let obs = Observable::just(42);
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = obs.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![42]);
    }

    #[test]
    fn test_observable_range() {
        let obs = Observable::range(0, 5);
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = obs.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_observable_from_iter() {
        let obs = Observable::from_iter(vec![1, 2, 3, 4, 5]);
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = obs.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_observable_empty() {
        let obs: Observable<i32> = Observable::empty();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = obs.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, Vec::<i32>::new());
    }

    #[test]
    fn test_observable_error() {
        let obs: Observable<i32> = Observable::error(ReactiveError::Disposed);
        let received_errors = Arc::new(Mutex::new(Vec::new()));
        let received_errors_clone = Arc::clone(&received_errors);

        let observer = super::super::FnObserver::new(
            |_value| {},
            move |error| {
                received_errors_clone.lock().unwrap().push(format!("{}", error));
            },
            || {},
        );

        let _subscription = obs.subscribe(observer);

        let errors = received_errors.lock().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Subscription was disposed"));
    }
}