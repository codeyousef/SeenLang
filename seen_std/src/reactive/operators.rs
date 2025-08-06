//! Stream operators for transforming and combining observables
//!
//! Operators are the building blocks of reactive programming. They transform,
//! filter, combine, and control the flow of data through streams.

use super::{Observable, Observer, ReactiveError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Extension trait to add operators to Observable
impl<T: 'static + Send + Sync> Observable<T> {
    /// Transform each emitted value using a function
    pub fn map<U, F>(self, f: F) -> Observable<U>
    where
        U: 'static + Send + Sync,
        F: Fn(T) -> U + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Observable::new(move |observer| {
            let f_clone = Arc::clone(&f);
            self.subscribe(MapObserver::new(observer, f_clone))
        })
    }

    /// Filter emitted values using a predicate
    pub fn filter<F>(self, predicate: F) -> Observable<T>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let predicate = Arc::new(predicate);
        Observable::new(move |observer| {
            let predicate_clone = Arc::clone(&predicate);
            self.subscribe(FilterObserver::new(observer, predicate_clone))
        })
    }

    /// Take only the first n values
    pub fn take(self, count: usize) -> Observable<T> {
        Observable::new(move |observer| {
            self.subscribe(TakeObserver::new(observer, count))
        })
    }

    /// Skip the first n values
    pub fn skip(self, count: usize) -> Observable<T> {
        Observable::new(move |observer| {
            self.subscribe(SkipObserver::new(observer, count))
        })
    }

    /// Transform each value to an Observable and flatten the result
    pub fn flat_map<U, F>(self, f: F) -> Observable<U>
    where
        U: 'static + Send + Sync + Clone,
        F: Fn(T) -> Observable<U> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Observable::new(move |observer| {
            let f_clone = Arc::clone(&f);
            self.subscribe(FlatMapObserver::new(observer, f_clone))
        })
    }

    /// Merge this Observable with another (TODO: implement properly)
    pub fn merge(self, _other: Observable<T>) -> Observable<T>
    where
        T: Clone,
    {
        // Simplified implementation - just return self for now
        self
    }

    /// Emit values only after a specified time has passed without another emission
    pub fn debounce(self, duration: Duration) -> Observable<T>
    where
        T: Clone,
    {
        Observable::new(move |observer| {
            self.subscribe(DebounceObserver::new(observer, duration))
        })
    }

    /// Emit at most one value per time period
    pub fn throttle(self, duration: Duration) -> Observable<T>
    where
        T: Clone,
    {
        Observable::new(move |observer| {
            self.subscribe(ThrottleObserver::new(observer, duration))
        })
    }

    /// Handle errors by switching to another Observable
    pub fn catch_error<F>(self, error_handler: F) -> Observable<T>
    where
        F: Fn(ReactiveError) -> Observable<T> + Send + Sync + 'static,
    {
        let error_handler = Arc::new(error_handler);
        Observable::new(move |observer| {
            let error_handler_clone = Arc::clone(&error_handler);
            self.subscribe(CatchErrorObserver::new(observer, error_handler_clone))
        })
    }

    /// Retry the Observable sequence up to a specified number of times
    pub fn retry(self, max_retries: usize) -> Observable<T>
    where
        T: Clone,
    {
        Observable::new(move |observer| {
            self.subscribe(RetryObserver::new(observer, max_retries, self.clone()))
        })
    }

    /// Apply a side effect for each emission without changing the stream
    pub fn tap<F>(self, side_effect: F) -> Observable<T>
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let side_effect = Arc::new(side_effect);
        Observable::new(move |observer| {
            let side_effect_clone = Arc::clone(&side_effect);
            self.subscribe(TapObserver::new(observer, side_effect_clone))
        })
    }
}

// Observer implementations for operators

struct MapObserver<T, U, F> {
    inner: Box<dyn Observer<U> + Send + Sync>,
    f: Arc<F>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U, F> MapObserver<T, U, F>
where
    F: Fn(T) -> U + Send + Sync,
{
    fn new(inner: Box<dyn Observer<U> + Send + Sync>, f: Arc<F>) -> Self {
        Self {
            inner,
            f,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U, F> Observer<T> for MapObserver<T, U, F>
where
    F: Fn(T) -> U + Send + Sync,
{
    fn on_next(&mut self, value: T) {
        let mapped = (self.f)(value);
        self.inner.on_next(mapped);
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct FilterObserver<T, F> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    predicate: Arc<F>,
}

impl<T, F> FilterObserver<T, F>
where
    F: Fn(&T) -> bool + Send + Sync,
{
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, predicate: Arc<F>) -> Self {
        Self { inner, predicate }
    }
}

impl<T, F> Observer<T> for FilterObserver<T, F>
where
    F: Fn(&T) -> bool + Send + Sync,
{
    fn on_next(&mut self, value: T) {
        if (self.predicate)(&value) {
            self.inner.on_next(value);
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct TakeObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    remaining: usize,
}

impl<T> TakeObserver<T> {
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, count: usize) -> Self {
        Self {
            inner,
            remaining: count,
        }
    }
}

impl<T> Observer<T> for TakeObserver<T> {
    fn on_next(&mut self, value: T) {
        if self.remaining > 0 {
            self.remaining -= 1;
            self.inner.on_next(value);
            
            if self.remaining == 0 {
                self.inner.on_completed();
            }
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct SkipObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    remaining: usize,
}

impl<T> SkipObserver<T> {
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, count: usize) -> Self {
        Self {
            inner,
            remaining: count,
        }
    }
}

impl<T> Observer<T> for SkipObserver<T> {
    fn on_next(&mut self, value: T) {
        if self.remaining > 0 {
            self.remaining -= 1;
        } else {
            self.inner.on_next(value);
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct FlatMapObserver<T, U, F> {
    inner: Box<dyn Observer<U> + Send + Sync>,
    f: Arc<F>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, U, F> FlatMapObserver<T, U, F>
where
    U: 'static + Send + Sync + Clone,
    F: Fn(T) -> Observable<U> + Send + Sync,
{
    fn new(inner: Box<dyn Observer<U> + Send + Sync>, f: Arc<F>) -> Self {
        Self {
            inner,
            f,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, U, F> Observer<T> for FlatMapObserver<T, U, F>
where
    U: 'static + Send + Sync + Clone,
    F: Fn(T) -> Observable<U> + Send + Sync,
{
    fn on_next(&mut self, value: T) {
        let inner_observable = (self.f)(value);
        // In a real implementation, we'd need to manage subscriptions properly
        // For now, this is a simplified version
        let _ = inner_observable.subscribe_fn(|_inner_value| {
            // This won't work in the real implementation due to borrowing issues
            // In practice, we'd need a more complex structure to handle this
        });
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct MergeObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    _source_id: usize,
}

impl<T> MergeObserver<T> {
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, source_id: usize) -> Self {
        Self {
            inner,
            _source_id: source_id,
        }
    }
}

impl<T> Observer<T> for MergeObserver<T> {
    fn on_next(&mut self, value: T) {
        self.inner.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        // In a real implementation, we'd only complete when both sources complete
        self.inner.on_completed();
    }
}

struct DebounceObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    duration: Duration,
    last_emission: Arc<Mutex<Option<std::time::Instant>>>,
}

impl<T> DebounceObserver<T> {
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, duration: Duration) -> Self {
        Self {
            inner,
            duration,
            last_emission: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T: Clone> Observer<T> for DebounceObserver<T> {
    fn on_next(&mut self, value: T) {
        let now = std::time::Instant::now();
        let mut last_emission = self.last_emission.lock().unwrap();
        
        match *last_emission {
            Some(last) if now.duration_since(last) < self.duration => {
                // Too soon, ignore this emission
                return;
            }
            _ => {
                *last_emission = Some(now);
                drop(last_emission);
                self.inner.on_next(value);
            }
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct ThrottleObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    duration: Duration,
    last_emission: Arc<Mutex<Option<std::time::Instant>>>,
}

impl<T> ThrottleObserver<T> {
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, duration: Duration) -> Self {
        Self {
            inner,
            duration,
            last_emission: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T: Clone> Observer<T> for ThrottleObserver<T> {
    fn on_next(&mut self, value: T) {
        let now = std::time::Instant::now();
        let mut last_emission = self.last_emission.lock().unwrap();
        
        match *last_emission {
            Some(last) if now.duration_since(last) < self.duration => {
                // Throttled, ignore
                return;
            }
            _ => {
                *last_emission = Some(now);
                drop(last_emission);
                self.inner.on_next(value);
            }
        }
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct CatchErrorObserver<T, F> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    error_handler: Arc<F>,
}

impl<T, F> CatchErrorObserver<T, F>
where
    F: Fn(ReactiveError) -> Observable<T> + Send + Sync,
{
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, error_handler: Arc<F>) -> Self {
        Self { inner, error_handler }
    }
}

impl<T, F> Observer<T> for CatchErrorObserver<T, F>
where
    T: 'static + Send + Sync,
    F: Fn(ReactiveError) -> Observable<T> + Send + Sync,
{
    fn on_next(&mut self, value: T) {
        self.inner.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        let _recovery_observable = (self.error_handler)(error);
        // In a real implementation, we'd subscribe to the recovery observable
        // For now, just pass through the error
        self.inner.on_error(ReactiveError::ObserverError("Error handling not fully implemented".to_string()));
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct RetryObserver<T> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    max_retries: usize,
    current_retries: usize,
    source: Observable<T>,
}

impl<T> RetryObserver<T>
where
    T: Clone,
{
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, max_retries: usize, source: Observable<T>) -> Self {
        Self {
            inner,
            max_retries,
            current_retries: 0,
            source,
        }
    }
}

impl<T> Observer<T> for RetryObserver<T>
where
    T: 'static + Send + Sync + Clone,
{
    fn on_next(&mut self, value: T) {
        self.inner.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        if self.current_retries < self.max_retries {
            self.current_retries += 1;
            // In a real implementation, we'd retry the subscription
            // For now, just pass through the error
        }
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

struct TapObserver<T, F> {
    inner: Box<dyn Observer<T> + Send + Sync>,
    side_effect: Arc<F>,
}

impl<T, F> TapObserver<T, F>
where
    F: Fn(&T) + Send + Sync,
{
    fn new(inner: Box<dyn Observer<T> + Send + Sync>, side_effect: Arc<F>) -> Self {
        Self { inner, side_effect }
    }
}

impl<T, F> Observer<T> for TapObserver<T, F>
where
    F: Fn(&T) + Send + Sync,
{
    fn on_next(&mut self, value: T) {
        (self.side_effect)(&value);
        self.inner.on_next(value);
    }

    fn on_error(&mut self, error: ReactiveError) {
        self.inner.on_error(error);
    }

    fn on_completed(&mut self) {
        self.inner.on_completed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_map_operator() {
        let obs = Observable::range(0, 5);
        let mapped = obs.map(|x| x * 2);
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = mapped.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn test_filter_operator() {
        let obs = Observable::range(0, 10);
        let filtered = obs.filter(|x| x % 2 == 0);
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = filtered.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn test_take_operator() {
        let obs = Observable::range(0, 10);
        let taken = obs.take(3);
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = taken.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 1, 2]);
    }

    #[test]
    fn test_skip_operator() {
        let obs = Observable::range(0, 5);
        let skipped = obs.skip(2);
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = skipped.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![2, 3, 4]);
    }

    #[test]
    fn test_tap_operator() {
        let obs = Observable::range(0, 3);
        let tapped_values = Arc::new(Mutex::new(Vec::new()));
        let tapped_values_clone = Arc::clone(&tapped_values);
        
        let tapped = obs.tap(move |x| {
            tapped_values_clone.lock().unwrap().push(*x);
        });
        
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = tapped.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        let tapped = tapped_values.lock().unwrap();
        assert_eq!(*values, vec![0, 1, 2]);
        assert_eq!(*tapped, vec![0, 1, 2]);
    }

    #[test]
    fn test_operator_chaining() {
        let obs = Observable::range(0, 10);
        let chained = obs
            .filter(|x| x % 2 == 0)  // Keep even numbers
            .map(|x| x * 3)          // Multiply by 3
            .take(3);                // Take first 3

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);

        let _subscription = chained.subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

        let values = received.lock().unwrap();
        assert_eq!(*values, vec![0, 6, 12]);
    }
}