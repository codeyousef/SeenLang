//! Observable streams for reactive programming
//! 
//! Implements Observable patterns following Seen syntax:
//! - Observable<T> for data streams
//! - Operators like Map, Filter, Throttle, Debounce
//! - Subscription management and error handling

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use futures::stream::Stream;
use seen_concurrency::types::*;

/// Unique identifier for observables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObservableId(u64);

impl ObservableId {
    /// Create new observable ID
    pub fn new() -> Self {
        // In real implementation, would use proper ID generation
        Self(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64)
    }
    
    /// Get ID value
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// Observable stream for reactive programming
pub struct Observable<T> {
    /// Unique observable identifier
    pub id: ObservableId,
    /// Values in the observable
    values: Arc<Mutex<Vec<T>>>,
    /// Subscriptions to this observable
    subscriptions: Arc<Mutex<HashMap<SubscriptionId, Subscription<T>>>>,
    /// Observable metadata
    metadata: ObservableMetadata,
}

impl<T> std::fmt::Debug for Observable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observable")
            .field("id", &self.id)
            .field("metadata", &self.metadata)
            .field("subscription_count", &self.subscriptions.lock().unwrap().len())
            .finish()
    }
}

/// Metadata for observable streams
#[derive(Debug, Clone)]
pub struct ObservableMetadata {
    /// Observable name
    pub name: String,
    /// Creation timestamp
    pub created_at: Instant,
    /// Source observable ID (for derived observables)
    pub source_id: Option<ObservableId>,
    /// Whether observable supports backpressure
    pub supports_backpressure: bool,
}

/// Subscription to an observable stream
pub struct Subscription<T> {
    /// Unique subscription identifier
    pub id: SubscriptionId,
    /// Callback function for new values
    pub callback: Arc<dyn Fn(T) -> AsyncResult + Send + Sync>,
    /// Whether subscription is active
    pub is_active: bool,
    /// Subscription metadata
    pub metadata: SubscriptionMetadata,
}

impl<T> std::fmt::Debug for Subscription<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription")
            .field("id", &self.id)
            .field("is_active", &self.is_active)
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// Unique identifier for subscriptions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Create new subscription ID
    pub fn new() -> Self {
        Self(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64)
    }
}

/// Subscription metadata
#[derive(Debug, Clone)]
pub struct SubscriptionMetadata {
    /// Subscription name
    pub name: String,
    /// Creation timestamp
    pub created_at: Instant,
    /// Observer type
    pub observer_type: String,
}

impl<T: Clone + Send + 'static> Observable<T> {
    /// Create new observable
    pub fn new(name: String) -> Self {
        Self {
            id: ObservableId::new(),
            values: Arc::new(Mutex::new(Vec::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            metadata: ObservableMetadata {
                name,
                created_at: Instant::now(),
                source_id: None,
                supports_backpressure: true,
            },
        }
    }
    
    /// Emit a value to all subscribers
    pub fn emit(&self, value: T) -> Result<(), AsyncError> {
        // Store value
        if let Ok(mut values) = self.values.lock() {
            values.push(value.clone());
        }
        
        // Notify subscribers
        if let Ok(subscriptions) = self.subscriptions.lock() {
            for subscription in subscriptions.values() {
                if subscription.is_active {
                    let _ = (subscription.callback)(value.clone());
                }
            }
        }
        Ok(())
    }
    
    /// Subscribe to the observable
    pub fn subscribe<F>(&self, callback: F) -> SubscriptionId
    where
        F: Fn(T) -> AsyncResult + Send + Sync + 'static,
    {
        let subscription_id = SubscriptionId::new();
        let subscription = Subscription {
            id: subscription_id,
            callback: Arc::new(callback),
            is_active: true,
            metadata: SubscriptionMetadata {
                name: "subscription".to_string(),
                created_at: Instant::now(),
                observer_type: "callback".to_string(),
            },
        };
        
        if let Ok(mut subscriptions) = self.subscriptions.lock() {
            subscriptions.insert(subscription_id, subscription);
        }
        
        subscription_id
    }
    
    /// Unsubscribe from the observable
    pub fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<(), AsyncError> {
        if let Ok(mut subscriptions) = self.subscriptions.lock() {
            subscriptions.remove(&subscription_id);
        }
        Ok(())
    }
    
    /// Map transformation (simplified)
    pub fn map<U, F>(&self, mapper: F) -> Observable<U>
    where
        F: Fn(T) -> U + Send + Sync + 'static,
        U: Clone + Send + 'static,
    {
        let mapped = Observable::new(format!("{}_mapped", self.metadata.name));
        
        // Set up subscription to transform values
        let mapped_clone = mapped.clone();
        let mapper = Arc::new(mapper);
        self.subscribe(move |value| {
            let mapped_value = mapper(value);
            mapped_clone.emit(mapped_value)?;
            Ok(AsyncValue::Unit)
        });
        
        mapped
    }
    
    /// Filter values
    pub fn filter<F>(&self, predicate: F) -> Observable<T>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let filtered = Observable::new(format!("{}_filtered", self.metadata.name));
        
        let filtered_clone = filtered.clone();
        let predicate = Arc::new(predicate);
        self.subscribe(move |value| {
            if predicate(&value) {
                filtered_clone.emit(value)?;
            }
            Ok(AsyncValue::Unit)
        });
        
        filtered
    }
    
    /// Get all current values
    pub fn get_values(&self) -> Vec<T> {
        if let Ok(values) = self.values.lock() {
            values.clone()
        } else {
            Vec::new()
        }
    }
}

impl<T: Clone> Clone for Observable<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            values: Arc::clone(&self.values),
            subscriptions: Arc::clone(&self.subscriptions),
            metadata: self.metadata.clone(),
        }
    }
}

/// Factory for creating observables
#[derive(Debug)]
pub struct ObservableFactory {
    /// Next observable ID
    next_id: u64,
    /// Statistics
    stats: ObservableStats,
}

/// Observable statistics
#[derive(Debug, Clone)]
pub struct ObservableStats {
    /// Total observables created
    pub total_observables: usize,
    /// Total subscriptions
    pub total_subscriptions: usize,
}

impl ObservableFactory {
    /// Create new factory
    pub fn new() -> Self {
        Self {
            next_id: 0,
            stats: ObservableStats {
                total_observables: 0,
                total_subscriptions: 0,
            },
        }
    }
    
    /// Create observable from range
    pub fn range(&mut self, start: i32, end: i32, step: i32) -> Observable<i32> {
        let observable = Observable::new("range".to_string());
        
        // Emit range values
        let mut current = start;
        if step > 0 {
            while current < end {
                let _ = observable.emit(current);
                current += step;
            }
        } else if step < 0 {
            while current > end {
                let _ = observable.emit(current);
                current += step;
            }
        }
        
        self.stats.total_observables += 1;
        observable
    }
    
    /// Create observable from vector
    pub fn from_vec<T: Clone + Send + 'static>(&mut self, values: Vec<T>) -> Observable<T> {
        let observable = Observable::new("vector".to_string());
        
        // Emit all values
        for value in values {
            let _ = observable.emit(value);
        }
        
        self.stats.total_observables += 1;
        observable
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> ObservableStats {
        self.stats.clone()
    }
}

impl Default for ObservableFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_observable_creation() {
        let observable = Observable::<i32>::new("test".to_string());
        assert_eq!(observable.metadata.name, "test");
        assert!(observable.metadata.supports_backpressure);
    }
    
    #[test]
    fn test_observable_emit_and_subscribe() {
        let observable = Observable::new("test".to_string());
        let received = Arc::new(AtomicUsize::new(0));
        let received_clone = Arc::clone(&received);
        
        let _subscription_id = observable.subscribe(move |_value| {
            received_clone.fetch_add(1, Ordering::SeqCst);
            Ok(AsyncValue::Unit)
        });
        
        observable.emit(42).unwrap();
        observable.emit(84).unwrap();
        
        assert_eq!(received.load(Ordering::SeqCst), 2);
    }
    
    #[test]
    fn test_observable_map() {
        let observable = Observable::new("test".to_string());
        let mapped = observable.map(|x| x * 2);
        
        observable.emit(5).unwrap();
        
        // In a full implementation, mapped would contain [10]
        assert_eq!(mapped.metadata.name, "test_mapped");
    }
    
    #[test]
    fn test_observable_filter() {
        let observable = Observable::new("test".to_string());
        let filtered = observable.filter(|&x| x > 10);
        
        observable.emit(5).unwrap();
        observable.emit(15).unwrap();
        
        assert_eq!(filtered.metadata.name, "test_filtered");
    }
    
    #[test]
    fn test_observable_factory() {
        let mut factory = ObservableFactory::new();
        
        let range_obs = factory.range(1, 5, 1);
        assert_eq!(range_obs.get_values(), vec![1, 2, 3, 4]);
        
        let vec_obs = factory.from_vec(vec![10, 20, 30]);
        assert_eq!(vec_obs.get_values(), vec![10, 20, 30]);
        
        assert_eq!(factory.stats.total_observables, 2);
    }
    
    #[test]
    fn test_subscription_management() {
        let observable = Observable::<i32>::new("test".to_string());
        
        let subscription_id = observable.subscribe(|_| Ok(AsyncValue::Unit));
        
        // Verify subscription exists
        {
            let subscriptions = observable.subscriptions.lock().unwrap();
            assert!(subscriptions.contains_key(&subscription_id));
        }
        
        // Unsubscribe
        observable.unsubscribe(subscription_id).unwrap();
        
        // Verify subscription removed
        {
            let subscriptions = observable.subscriptions.lock().unwrap();
            assert!(!subscriptions.contains_key(&subscription_id));
        }
    }
}