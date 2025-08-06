//! Subject implementations - hot observables that can emit values
//!
//! Subjects are both Observers and Observables. They can be subscribed to
//! and they can emit values to their subscribers.

use super::{Observable, Observer, ReactiveError, Subscription};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Subject is a hot observable that can emit values to multiple observers
pub struct Subject<T> {
    observers: Arc<Mutex<HashMap<u64, Box<dyn Observer<T> + Send + Sync>>>>,
    next_id: Arc<Mutex<u64>>,
    completed: Arc<Mutex<bool>>,
    error: Arc<Mutex<Option<ReactiveError>>>,
}

impl<T: 'static + Send + Sync + Clone> Subject<T> {
    /// Create a new Subject
    pub fn new() -> Self {
        Self {
            observers: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
            completed: Arc::new(Mutex::new(false)),
            error: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the number of active observers
    pub fn observer_count(&self) -> usize {
        self.observers.lock().unwrap().len()
    }

    /// Check if the subject has completed
    pub fn is_completed(&self) -> bool {
        *self.completed.lock().unwrap()
    }

    /// Check if the subject has errored
    pub fn has_error(&self) -> bool {
        self.error.lock().unwrap().is_some()
    }

    /// Convert this Subject into an Observable
    pub fn as_observable(&self) -> Observable<T> {
        let observers = Arc::clone(&self.observers);
        let next_id = Arc::clone(&self.next_id);
        let completed = Arc::clone(&self.completed);
        let error = Arc::clone(&self.error);

        Observable::new(move |observer| {
            // Check if already completed or errored
            if let Ok(completed) = completed.lock() {
                if *completed {
                    let mut observer = observer;
                    observer.on_completed();
                    return Subscription::empty();
                }
            }

            if let Ok(error_opt) = error.lock() {
                if let Some(ref err) = *error_opt {
                    let mut observer = observer;
                    observer.on_error(err.clone());
                    return Subscription::empty();
                }
            }

            // Add observer to the list
            let observer_id = {
                let mut next_id = next_id.lock().unwrap();
                let id = *next_id;
                *next_id += 1;
                id
            };

            if let Ok(mut observers) = observers.lock() {
                observers.insert(observer_id, observer);
            }

            let observers_for_cleanup = Arc::clone(&observers);
            Subscription::new(move || {
                if let Ok(mut observers) = observers_for_cleanup.lock() {
                    observers.remove(&observer_id);
                }
            })
        })
    }

    /// Clean up dead observer references (no-op since we use owned observers now)
    fn cleanup_observers(&self) {
        // With owned observers, cleanup is handled by subscription disposal
    }
}

impl<T: Clone + 'static + Send + Sync> Default for Subject<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static> Observer<T> for Subject<T> {
    fn on_next(&mut self, value: T) {
        // Check if already completed or errored
        if *self.completed.lock().unwrap() || self.error.lock().unwrap().is_some() {
            return;
        }

        self.cleanup_observers();

        // Emit to all observers
        if let Ok(mut observers) = self.observers.lock() {
            for observer in observers.values_mut() {
                observer.on_next(value.clone());
            }
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        // Set error state
        if let Ok(mut error_state) = self.error.lock() {
            if error_state.is_some() || *self.completed.lock().unwrap() {
                return; // Already errored or completed
            }
            *error_state = Some(error.clone());
        }

        self.cleanup_observers();

        // Emit error to all observers
        if let Ok(mut observers) = self.observers.lock() {
            for observer in observers.values_mut() {
                observer.on_error(error.clone());
            }
        }
        
        // Clear observers after error
        if let Ok(mut observers) = self.observers.lock() {
            observers.clear();
        }
    }

    fn on_completed(&mut self) {
        // Set completed state
        if let Ok(mut completed_state) = self.completed.lock() {
            if *completed_state || self.error.lock().unwrap().is_some() {
                return; // Already completed or errored
            }
            *completed_state = true;
        }

        self.cleanup_observers();

        // Emit completion to all observers
        if let Ok(mut observers) = self.observers.lock() {
            for observer in observers.values_mut() {
                observer.on_completed();
            }
        }
        
        // Clear observers after completion
        if let Ok(mut observers) = self.observers.lock() {
            observers.clear();
        }
    }
}

/// BehaviorSubject holds a current value and emits it to new subscribers
pub struct BehaviorSubject<T> {
    subject: Subject<T>,
    current_value: Arc<Mutex<Option<T>>>,
}

impl<T: 'static + Send + Sync + Clone> BehaviorSubject<T> {
    /// Create a new BehaviorSubject with an initial value
    pub fn new(initial_value: T) -> Self {
        Self {
            subject: Subject::new(),
            current_value: Arc::new(Mutex::new(Some(initial_value))),
        }
    }

    /// Get the current value
    pub fn value(&self) -> Option<T> {
        self.current_value.lock().unwrap().clone()
    }

    /// Convert to Observable
    pub fn as_observable(&self) -> Observable<T> {
        let current_value = Arc::clone(&self.current_value);
        let subject_observable = self.subject.as_observable();

        Observable::new(move |mut observer| {
            // Emit current value first
            if let Ok(current) = current_value.lock() {
                if let Some(ref value) = *current {
                    observer.on_next(value.clone());
                }
            }
            
            // Then subscribe to future values
            subject_observable.subscribe_boxed(observer)
        })
    }
}

impl<T: Clone + Send + Sync + 'static> Observer<T> for BehaviorSubject<T> {
    fn on_next(&mut self, value: T) {
        // Update current value
        if let Ok(mut current) = self.current_value.lock() {
            *current = Some(value.clone());
        }
        
        // Forward to subject
        self.subject.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.subject.on_error(error);
    }

    fn on_completed(&mut self) {
        self.subject.on_completed();
    }
}

/// ReplaySubject buffers the last N values and replays them to new subscribers
pub struct ReplaySubject<T> {
    subject: Subject<T>,
    buffer: Arc<Mutex<Vec<T>>>,
    buffer_size: usize,
}

impl<T: 'static + Send + Sync + Clone> ReplaySubject<T> {
    /// Create a new ReplaySubject with a buffer size
    pub fn new(buffer_size: usize) -> Self {
        Self {
            subject: Subject::new(),
            buffer: Arc::new(Mutex::new(Vec::new())),
            buffer_size,
        }
    }

    /// Convert to Observable
    pub fn as_observable(&self) -> Observable<T> {
        let buffer = Arc::clone(&self.buffer);
        let subject_observable = self.subject.as_observable();

        Observable::new(move |mut observer| {
            // Replay buffered values first
            if let Ok(buffer_values) = buffer.lock() {
                for value in buffer_values.iter() {
                    observer.on_next(value.clone());
                }
            }
            
            // Then subscribe to future values
            subject_observable.subscribe_boxed(observer)
        })
    }
}

impl<T: Clone + Send + Sync + 'static> Observer<T> for ReplaySubject<T> {
    fn on_next(&mut self, value: T) {
        // Add to buffer
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(value.clone());
            if buffer.len() > self.buffer_size {
                buffer.remove(0); // Remove oldest
            }
        }
        
        // Forward to subject
        self.subject.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.subject.on_error(error);
    }

    fn on_completed(&mut self) {
        self.subject.on_completed();
    }
}

/// AsyncSubject only emits the last value when the stream completes
pub struct AsyncSubject<T> {
    subject: Subject<T>,
    last_value: Arc<Mutex<Option<T>>>,
    completed: Arc<Mutex<bool>>,
}

impl<T: 'static + Send + Sync + Clone> AsyncSubject<T> {
    /// Create a new AsyncSubject
    pub fn new() -> Self {
        Self {
            subject: Subject::new(),
            last_value: Arc::new(Mutex::new(None)),
            completed: Arc::new(Mutex::new(false)),
        }
    }

    /// Convert to Observable
    pub fn as_observable(&self) -> Observable<T> {
        let subject_observable = self.subject.as_observable();
        subject_observable
    }
}

impl<T: Clone + 'static + Send + Sync> Default for AsyncSubject<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static> Observer<T> for AsyncSubject<T> {
    fn on_next(&mut self, value: T) {
        // Just store the value, don't emit until completion
        if let Ok(mut last) = self.last_value.lock() {
            *last = Some(value);
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.subject.on_error(error);
    }

    fn on_completed(&mut self) {
        // Emit the last value if we have one
        if let Ok(last_value) = self.last_value.lock() {
            if let Some(ref value) = *last_value {
                self.subject.on_next(value.clone());
            }
        }
        
        self.subject.on_completed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_subject_basic() {
        let mut subject = Subject::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observable = subject.as_observable();
        let _subscription = observable.subscribe_fn(move |value: i32| {
            received_clone.lock().unwrap().push(value);
        });

        assert_eq!(subject.observer_count(), 1);

        subject.on_next(1);
        subject.on_next(2);
        subject.on_next(3);

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![1, 2, 3]);
    }

    #[test]
    fn test_behavior_subject() {
        let mut behavior_subject = BehaviorSubject::new(0);
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        // Subscribe should immediately receive current value
        let observable = behavior_subject.as_observable();
        let _subscription = observable.subscribe_fn(move |value: i32| {
            received_clone.lock().unwrap().push(value);
        });

        // Should have received the initial value
        {
            let values = received.lock().unwrap();
            assert_eq!(*values, vec![0]);
        }

        // Emit new values
        behavior_subject.on_next(1);
        behavior_subject.on_next(2);

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 1, 2]);
        
        // Check current value
        assert_eq!(behavior_subject.value(), Some(2));
    }

    #[test]
    fn test_replay_subject() {
        let mut replay_subject = ReplaySubject::new(2);

        // Emit some values before subscribing
        replay_subject.on_next(1);
        replay_subject.on_next(2);
        replay_subject.on_next(3);

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        // Subscribe should receive the last 2 values
        let observable = replay_subject.as_observable();
        let _subscription = observable.subscribe_fn(move |value: i32| {
            received_clone.lock().unwrap().push(value);
        });

        // Should have received the last 2 values
        {
            let values = received.lock().unwrap();
            assert_eq!(*values, vec![2, 3]);
        }

        // Emit a new value
        replay_subject.on_next(4);

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![2, 3, 4]);
    }

    #[test]
    fn test_async_subject() {
        let mut async_subject = AsyncSubject::new();
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let observable = async_subject.as_observable();
        let _subscription = observable.subscribe_fn(move |value: i32| {
            received_clone.lock().unwrap().push(value);
        });

        // Emit values - nothing should be received yet
        async_subject.on_next(1);
        async_subject.on_next(2);
        async_subject.on_next(3);

        {
            let values = received.lock().unwrap();
            assert!(values.is_empty());
        }

        // Complete - should receive the last value
        async_subject.on_completed();

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![3]);
    }

    #[test]
    fn test_subject_error_handling() {
        let mut subject = Subject::new();
        let errors_received = Arc::new(AtomicU32::new(0));
        let errors_received_clone = Arc::clone(&errors_received);

        let observer = super::super::FnObserver::new(
            |_value: i32| {},
            move |_error| {
                errors_received_clone.fetch_add(1, Ordering::SeqCst);
            },
            || {},
        );

        let observable = subject.as_observable();
        let _subscription = observable.subscribe(observer);

        subject.on_error(ReactiveError::Disposed);
        assert_eq!(errors_received.load(Ordering::SeqCst), 1);
    }
}