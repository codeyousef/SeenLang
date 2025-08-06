//! Subscription and disposable resource management
//!
//! Subscriptions represent the connection between an Observable and Observer.
//! They can be disposed to clean up resources and prevent memory leaks.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;

/// Trait for objects that can be disposed
pub trait Disposable {
    /// Dispose of this resource
    fn dispose(&self);
    
    /// Check if this resource has been disposed
    fn is_disposed(&self) -> bool;
}

/// Subscription represents a connection that can be disposed
pub struct Subscription {
    disposed: Arc<AtomicBool>,
    dispose_actions: Arc<Mutex<Vec<Box<dyn FnOnce() + Send + Sync>>>>,
}

impl Subscription {
    /// Create a new Subscription with a dispose action
    pub fn new<F>(dispose_action: F) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Self {
            disposed: Arc::new(AtomicBool::new(false)),
            dispose_actions: Arc::new(Mutex::new(vec![Box::new(dispose_action)])),
        }
    }

    /// Create an empty subscription that does nothing when disposed
    pub fn empty() -> Self {
        Self {
            disposed: Arc::new(AtomicBool::new(false)),
            dispose_actions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add an additional dispose action
    pub fn add_dispose_action<F>(&self, action: F)
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        if !self.is_disposed() {
            if let Ok(mut actions) = self.dispose_actions.lock() {
                actions.push(Box::new(action));
            }
        } else {
            // If already disposed, execute the action immediately
            action();
        }
    }

    /// Create a subscription that manages multiple child subscriptions
    pub fn composite(subscriptions: Vec<Subscription>) -> Self {
        let child_subscriptions = Arc::new(Mutex::new(subscriptions));
        
        Self::new(move || {
            if let Ok(subscriptions) = child_subscriptions.lock() {
                for subscription in subscriptions.iter() {
                    subscription.dispose();
                }
            }
        })
    }
}

impl Disposable for Subscription {
    fn dispose(&self) {
        // Use compare_exchange to ensure we only dispose once
        if self.disposed.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            // Take all dispose actions and execute them
            if let Ok(mut actions) = self.dispose_actions.lock() {
                let actions_to_run = std::mem::take(&mut *actions);
                drop(actions); // Release the lock before executing actions
                
                for action in actions_to_run {
                    action();
                }
            }
        }
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // Don't auto-dispose on drop to allow manual resource management
        // Users must explicitly call dispose() or let the CompositeDisposable manage it
    }
}

/// CompositeDisposable manages multiple disposables
pub struct CompositeDisposable {
    disposed: Arc<AtomicBool>,
    disposables: Arc<Mutex<HashMap<u64, Box<dyn Disposable + Send + Sync>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl CompositeDisposable {
    /// Create a new CompositeDisposable
    pub fn new() -> Self {
        Self {
            disposed: Arc::new(AtomicBool::new(false)),
            disposables: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Add a disposable and return its ID for removal
    pub fn add<D>(&self, disposable: D) -> u64
    where
        D: Disposable + Send + Sync + 'static,
    {
        if self.is_disposed() {
            disposable.dispose();
            return 0;
        }

        let id = {
            let mut next_id = self.next_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };

        if let Ok(mut disposables) = self.disposables.lock() {
            disposables.insert(id, Box::new(disposable));
        }

        id
    }

    /// Remove a disposable by ID
    pub fn remove(&self, id: u64) {
        if let Ok(mut disposables) = self.disposables.lock() {
            disposables.remove(&id);
        }
    }

    /// Clear all disposables without disposing them
    pub fn clear(&self) {
        if let Ok(mut disposables) = self.disposables.lock() {
            disposables.clear();
        }
    }
}

impl Default for CompositeDisposable {
    fn default() -> Self {
        Self::new()
    }
}

impl Disposable for CompositeDisposable {
    fn dispose(&self) {
        if self.disposed.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            if let Ok(disposables) = self.disposables.lock() {
                for (_, disposable) in disposables.iter() {
                    disposable.dispose();
                }
            }
        }
    }

    fn is_disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_subscription_dispose() {
        let disposed_count = Arc::new(AtomicU32::new(0));
        let disposed_count_clone = Arc::clone(&disposed_count);

        let subscription = Subscription::new(move || {
            disposed_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert!(!subscription.is_disposed());
        subscription.dispose();
        assert!(subscription.is_disposed());
        assert_eq!(disposed_count.load(Ordering::SeqCst), 1);

        // Disposing again should not call the action again
        subscription.dispose();
        assert_eq!(disposed_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subscription_empty() {
        let subscription = Subscription::empty();
        assert!(!subscription.is_disposed());
        subscription.dispose();
        assert!(subscription.is_disposed());
        // Should not panic or cause issues
    }

    #[test]
    fn test_subscription_composite() {
        let disposed_count = Arc::new(AtomicU32::new(0));
        let disposed_count1 = Arc::clone(&disposed_count);
        let disposed_count2 = Arc::clone(&disposed_count);

        let sub1 = Subscription::new(move || {
            disposed_count1.fetch_add(1, Ordering::SeqCst);
        });
        let sub2 = Subscription::new(move || {
            disposed_count2.fetch_add(1, Ordering::SeqCst);
        });

        let composite = Subscription::composite(vec![sub1, sub2]);
        composite.dispose();

        assert_eq!(disposed_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_composite_disposable() {
        let disposed_count = Arc::new(AtomicU32::new(0));
        let disposed_count1 = Arc::clone(&disposed_count);
        let disposed_count2 = Arc::clone(&disposed_count);

        let composite = CompositeDisposable::new();

        let sub1 = Subscription::new(move || {
            disposed_count1.fetch_add(1, Ordering::SeqCst);
        });
        let sub2 = Subscription::new(move || {
            disposed_count2.fetch_add(1, Ordering::SeqCst);
        });

        let id1 = composite.add(sub1);
        let _id2 = composite.add(sub2);

        assert_eq!(disposed_count.load(Ordering::SeqCst), 0);

        composite.remove(id1);
        composite.dispose();

        // Only sub2 should be disposed (sub1 was removed)
        assert_eq!(disposed_count.load(Ordering::SeqCst), 1);
        assert!(composite.is_disposed());
    }

    #[test]
    fn test_subscription_add_dispose_action() {
        let disposed_count = Arc::new(AtomicU32::new(0));
        let disposed_count1 = Arc::clone(&disposed_count);
        let disposed_count2 = Arc::clone(&disposed_count);

        let subscription = Subscription::new(move || {
            disposed_count1.fetch_add(1, Ordering::SeqCst);
        });

        subscription.add_dispose_action(move || {
            disposed_count2.fetch_add(1, Ordering::SeqCst);
        });

        subscription.dispose();
        assert_eq!(disposed_count.load(Ordering::SeqCst), 2);
    }
}