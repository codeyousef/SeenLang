//! Scheduler implementations for reactive streams
//!
//! Schedulers control when and where observable emissions occur.
//! Different schedulers provide different execution contexts.

use super::Subscription;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use std::collections::BinaryHeap;
use std::cmp::Reverse;

/// Trait for scheduling work
pub trait Scheduler: Send + Sync {
    /// Schedule work to be executed immediately
    fn schedule<F>(&self, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static;

    /// Schedule work to be executed after a delay
    fn schedule_delayed<F>(&self, delay: Duration, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static;

    /// Schedule recurring work
    fn schedule_periodic<F>(&self, _period: Duration, work: F) -> Subscription
    where
        F: Fn() + Send + Sync + 'static;
}

/// Immediate scheduler executes work synchronously
pub struct ImmediateScheduler;

impl ImmediateScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImmediateScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for ImmediateScheduler {
    fn schedule<F>(&self, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        work();
        Subscription::empty()
    }

    fn schedule_delayed<F>(&self, delay: Duration, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        thread::sleep(delay);
        work();
        Subscription::empty()
    }

    fn schedule_periodic<F>(&self, _period: Duration, work: F) -> Subscription
    where
        F: Fn() + Send + Sync + 'static,
    {
        // For immediate scheduler, we'll just execute once and return
        // In a real implementation, this might spawn a thread
        work();
        Subscription::empty()
    }
}

/// Async scheduler executes work on the event loop
pub struct AsyncScheduler {
    // For now, this is a placeholder implementation
    // In a real implementation, this would integrate with an async runtime
}

impl AsyncScheduler {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AsyncScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for AsyncScheduler {
    fn schedule<F>(&self, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        // For now, execute immediately
        // In a real implementation, this would schedule on the async executor
        work();
        Subscription::empty()
    }

    fn schedule_delayed<F>(&self, delay: Duration, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        // Placeholder implementation
        thread::sleep(delay);
        work();
        Subscription::empty()
    }

    fn schedule_periodic<F>(&self, _period: Duration, work: F) -> Subscription
    where
        F: Fn() + Send + Sync + 'static,
    {
        // Placeholder implementation
        work();
        Subscription::empty()
    }
}

/// Thread pool scheduler executes work on a thread pool
pub struct ThreadPoolScheduler {
    // Placeholder for thread pool
    // In a real implementation, this would manage a thread pool
}

impl ThreadPoolScheduler {
    pub fn new() -> Self {
        Self {}
    }

    pub fn with_thread_count(_count: usize) -> Self {
        Self {}
    }
}

impl Default for ThreadPoolScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for ThreadPoolScheduler {
    fn schedule<F>(&self, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        // Spawn work on a separate thread
        let handle = thread::spawn(work);
        
        Subscription::new(move || {
            // In a real implementation, we'd cancel the work if possible
            let _ = handle.join();
        })
    }

    fn schedule_delayed<F>(&self, delay: Duration, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = thread::spawn(move || {
            thread::sleep(delay);
            work();
        });
        
        Subscription::new(move || {
            let _ = handle.join();
        })
    }

    fn schedule_periodic<F>(&self, period: Duration, work: F) -> Subscription
    where
        F: Fn() + Send + Sync + 'static,
    {
        let work = Arc::new(work);
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        
        let handle = thread::spawn(move || {
            while running_clone.load(std::sync::atomic::Ordering::SeqCst) {
                work();
                thread::sleep(period);
            }
        });
        
        Subscription::new(move || {
            running.store(false, std::sync::atomic::Ordering::SeqCst);
            let _ = handle.join();
        })
    }
}

/// Virtual time scheduler for testing
pub struct VirtualTimeScheduler {
    current_time: Arc<Mutex<Instant>>,
    scheduled_work: Arc<Mutex<BinaryHeap<Reverse<ScheduledWork>>>>,
}

struct ScheduledWork {
    time: Instant,
    work: Box<dyn FnOnce() + Send>,
    id: u64,
}

impl PartialEq for ScheduledWork {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.id == other.id
    }
}

impl Eq for ScheduledWork {}

impl Ord for ScheduledWork {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.cmp(&other.time).then_with(|| self.id.cmp(&other.id))
    }
}

impl PartialOrd for ScheduledWork {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl VirtualTimeScheduler {
    pub fn new() -> Self {
        Self {
            current_time: Arc::new(Mutex::new(Instant::now())),
            scheduled_work: Arc::new(Mutex::new(BinaryHeap::new())),
        }
    }

    /// Get the current virtual time
    pub fn current_time(&self) -> Instant {
        *self.current_time.lock().unwrap()
    }

    /// Advance virtual time by the given duration
    pub fn advance_time(&self, duration: Duration) {
        let mut current_time = self.current_time.lock().unwrap();
        *current_time += duration;
        let target_time = *current_time;
        drop(current_time);

        // Execute all work scheduled up to this time
        let mut scheduled_work = self.scheduled_work.lock().unwrap();
        let mut work_to_execute = Vec::new();

        while let Some(Reverse(next_work)) = scheduled_work.peek() {
            if next_work.time <= target_time {
                if let Some(Reverse(work)) = scheduled_work.pop() {
                    work_to_execute.push(work);
                }
            } else {
                break;
            }
        }
        drop(scheduled_work);

        // Execute work outside of lock
        for work in work_to_execute {
            (work.work)();
        }
    }

    /// Execute all scheduled work regardless of time
    pub fn flush(&self) {
        let mut scheduled_work = self.scheduled_work.lock().unwrap();
        let work_to_execute: Vec<_> = scheduled_work.drain().collect();
        drop(scheduled_work);

        for Reverse(work) in work_to_execute {
            (work.work)();
        }
    }
}

impl Default for VirtualTimeScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for VirtualTimeScheduler {
    fn schedule<F>(&self, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        self.schedule_delayed(Duration::from_nanos(0), work)
    }

    fn schedule_delayed<F>(&self, delay: Duration, work: F) -> Subscription
    where
        F: FnOnce() + Send + 'static,
    {
        let current_time = self.current_time();
        let scheduled_time = current_time + delay;
        
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let scheduled_work = ScheduledWork {
            time: scheduled_time,
            work: Box::new(work),
            id,
        };

        let mut queue = self.scheduled_work.lock().unwrap();
        queue.push(Reverse(scheduled_work));

        Subscription::empty()
    }

    fn schedule_periodic<F>(&self, _period: Duration, work: F) -> Subscription
    where
        F: Fn() + Send + Sync + 'static,
    {
        // For virtual time scheduler, execute once
        // In a real implementation, this would schedule recurring work
        work();
        Subscription::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Disposable;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_immediate_scheduler() {
        let scheduler = ImmediateScheduler::new();
        let executed = Arc::new(AtomicU32::new(0));
        let executed_clone = Arc::clone(&executed);

        let subscription = scheduler.schedule(move || {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(executed.load(Ordering::SeqCst), 1);
        subscription.dispose(); // Should not cause issues
    }

    #[test]
    fn test_immediate_scheduler_delayed() {
        let scheduler = ImmediateScheduler::new();
        let executed = Arc::new(AtomicU32::new(0));
        let executed_clone = Arc::clone(&executed);

        let start = Instant::now();
        let subscription = scheduler.schedule_delayed(Duration::from_millis(10), move || {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        subscription.dispose();
    }

    #[test]
    fn test_thread_pool_scheduler() {
        let scheduler = ThreadPoolScheduler::new();
        let executed = Arc::new(AtomicU32::new(0));
        let executed_clone = Arc::clone(&executed);

        let subscription = scheduler.schedule(move || {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Give the thread time to execute
        thread::sleep(Duration::from_millis(50));
        assert_eq!(executed.load(Ordering::SeqCst), 1);
        subscription.dispose();
    }

    #[test]
    fn test_virtual_time_scheduler() {
        let scheduler = VirtualTimeScheduler::new();
        let executed = Arc::new(AtomicU32::new(0));
        let executed_clone = Arc::clone(&executed);

        let _subscription = scheduler.schedule_delayed(Duration::from_millis(100), move || {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Work should not be executed yet
        assert_eq!(executed.load(Ordering::SeqCst), 0);

        // Advance time by 50ms - still should not execute
        scheduler.advance_time(Duration::from_millis(50));
        assert_eq!(executed.load(Ordering::SeqCst), 0);

        // Advance time by another 50ms - now should execute
        scheduler.advance_time(Duration::from_millis(50));
        assert_eq!(executed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_virtual_time_scheduler_flush() {
        let scheduler = VirtualTimeScheduler::new();
        let executed = Arc::new(AtomicU32::new(0));
        let executed_clone = Arc::clone(&executed);

        let _subscription = scheduler.schedule_delayed(Duration::from_secs(1000), move || {
            executed_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Work should not be executed yet
        assert_eq!(executed.load(Ordering::SeqCst), 0);

        // Flush should execute all work regardless of time
        scheduler.flush();
        assert_eq!(executed.load(Ordering::SeqCst), 1);
    }
}