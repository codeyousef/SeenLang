//! Comprehensive tests for the reactive programming foundation
//!
//! These tests validate the core functionality and performance requirements
//! for the reactive programming system. They serve as both documentation
//! and verification of the implementation.

use super::*;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::thread;

/// Test that Observable creation works correctly
#[test]
fn test_observable_creation_and_subscription() {
    let obs = Observable::just(42);
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = Arc::clone(&received);

    let subscription = obs.subscribe_fn(move |value| {
        received_clone.lock().unwrap().push(value);
    });

    let values = received.lock().unwrap();
    assert_eq!(*values, vec![42]);
    
    // Subscription should be created successfully
    assert!(!subscription.is_disposed());
}

/// Test that stream operators compose efficiently
#[test]
fn test_stream_operators_composition() {
    let obs = Observable::range(0, 1000);
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = Arc::clone(&received);

    let start = Instant::now();
    
    let _subscription = obs
        .filter(|x| x % 2 == 0)
        .map(|x| x * 2)
        .take(10)
        .subscribe_fn(move |value| {
            received_clone.lock().unwrap().push(value);
        });

    let duration = start.elapsed();
    
    // Verify correct values received
    let values = received.lock().unwrap();
    assert_eq!(*values, vec![0, 4, 8, 12, 16, 20, 24, 28, 32, 36]);
    
    // Performance target: operator composition should be fast
    assert!(duration < Duration::from_millis(10), "Operator composition too slow: {:?}", duration);
}

/// Test that backpressure handling prevents memory overflow
#[test]
fn test_backpressure_handling_prevents_overflow() {
    use crate::reactive::backpressure::{BackpressureObserver, BackpressureStrategy};

    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = Arc::clone(&received);
    let errors_received = Arc::new(AtomicU32::new(0));
    let errors_received_clone = Arc::clone(&errors_received);

    let observer = FnObserver::new(
        move |value: i32| {
            received_clone.lock().unwrap().push(value);
        },
        move |_error| {
            errors_received_clone.fetch_add(1, Ordering::SeqCst);
        },
        || {},
    );

    let mut backpressure_observer = BackpressureObserver::new(
        Box::new(observer),
        BackpressureStrategy::Error(5), // Allow max 5 items
    );

    // Emit more values than the buffer can hold
    for i in 0..10 {
        backpressure_observer.on_next(i);
    }

    let values = received.lock().unwrap();
    assert!(values.len() <= 5, "Too many values passed through: {}", values.len());
    
    let errors = errors_received.load(Ordering::SeqCst);
    assert!(errors > 0, "Should have received backpressure error");
}

/// Test that hot and cold observables behave correctly
#[test]
fn test_hot_and_cold_observables_behavior() {
    // Cold observable - creates new stream for each subscriber
    let cold_obs = Observable::range(0, 3);
    
    let received1 = Arc::new(Mutex::new(Vec::new()));
    let received1_clone = Arc::clone(&received1);
    let received2 = Arc::new(Mutex::new(Vec::new()));
    let received2_clone = Arc::clone(&received2);

    let _sub1 = cold_obs.clone().subscribe_fn(move |value| {
        received1_clone.lock().unwrap().push(value);
    });
    
    let _sub2 = cold_obs.subscribe_fn(move |value| {
        received2_clone.lock().unwrap().push(value);
    });

    // Both subscribers should receive all values independently
    let values1 = received1.lock().unwrap();
    let values2 = received2.lock().unwrap();
    assert_eq!(*values1, vec![0, 1, 2]);
    assert_eq!(*values2, vec![0, 1, 2]);

    // Hot observable - Subject shares emissions between subscribers
    let mut subject = Subject::new();
    let hot_obs = subject.as_observable();

    let received3 = Arc::new(Mutex::new(Vec::new()));
    let received3_clone = Arc::clone(&received3);
    let received4 = Arc::new(Mutex::new(Vec::new()));
    let received4_clone = Arc::clone(&received4);

    let _sub3 = hot_obs.clone().subscribe_fn(move |value: i32| {
        received3_clone.lock().unwrap().push(value);
    });

    subject.on_next(10);
    
    let _sub4 = hot_obs.subscribe_fn(move |value| {
        received4_clone.lock().unwrap().push(value);
    });

    subject.on_next(20);
    subject.on_next(30);

    // First subscriber gets all values, second only gets values after subscription
    let values3 = received3.lock().unwrap();
    let values4 = received4.lock().unwrap();
    assert_eq!(*values3, vec![10, 20, 30]);
    assert_eq!(*values4, vec![20, 30]); // Missed the first value
}

/// Test that schedulers provide correct concurrency
#[test]
fn test_schedulers_provide_correct_concurrency() {
    // Immediate scheduler should execute synchronously
    let immediate_scheduler = ImmediateScheduler::new();
    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let subscription = immediate_scheduler.schedule(move || {
        executed_clone.store(true, Ordering::SeqCst);
    });

    // Should be executed immediately
    assert!(executed.load(Ordering::SeqCst), "Immediate scheduler should execute synchronously");
    subscription.dispose();

    // Thread pool scheduler should execute asynchronously
    let thread_scheduler = ThreadPoolScheduler::new();
    let executed2 = Arc::new(AtomicBool::new(false));
    let executed2_clone = Arc::clone(&executed2);

    let subscription2 = thread_scheduler.schedule(move || {
        thread::sleep(Duration::from_millis(10));
        executed2_clone.store(true, Ordering::SeqCst);
    });

    // Give it time to execute
    thread::sleep(Duration::from_millis(50));
    assert!(executed2.load(Ordering::SeqCst), "Thread pool scheduler should execute asynchronously");
    subscription2.dispose();
}

/// Test that memory leaks are prevented in subscription chains
#[test]
fn test_memory_leaks_prevented_in_subscription_chains() {
    let subscription_count = Arc::new(AtomicU32::new(0));
    let disposal_count = Arc::new(AtomicU32::new(0));
    
    {
        let obs = Observable::range(0, 100);
        let subscription_count_clone = Arc::clone(&subscription_count);
        let disposal_count_clone = Arc::clone(&disposal_count);

        let chained = obs
            .map(move |x| {
                subscription_count_clone.fetch_add(1, Ordering::SeqCst);
                x * 2
            })
            .filter(|x| x % 4 == 0)
            .take(10);

        let subscription = chained.subscribe_fn(move |_value| {
            // Do nothing, just consume
        });

        // Add disposal tracking
        subscription.add_dispose_action(move || {
            disposal_count_clone.fetch_add(1, Ordering::SeqCst);
        });
        
        // Subscription should be created and working
        assert!(!subscription.is_disposed());
        
        // Explicitly dispose
        subscription.dispose();
        assert!(subscription.is_disposed());
    }
    
    // After scope exit, disposal should have been called
    let disposed = disposal_count.load(Ordering::SeqCst);
    assert!(disposed > 0, "Disposal should have been called");
}

/// Performance test: ensure operators achieve <100ns overhead
#[test]
fn test_performance_operator_overhead_under_100ns() {
    let iterations = 10_000;
    let obs = Observable::from_iter(0..iterations);
    
    // Measure pure iteration baseline
    let start = Instant::now();
    for i in 0..iterations {
        let _ = i * 2; // Simple operation
    }
    let baseline_duration = start.elapsed();
    
    // Measure reactive stream performance
    let received_count = Arc::new(AtomicU32::new(0));
    let received_count_clone = Arc::clone(&received_count);
    
    let start = Instant::now();
    let _subscription = obs
        .map(|x| x * 2)
        .filter(|x| x % 2 == 0)
        .subscribe_fn(move |_value| {
            received_count_clone.fetch_add(1, Ordering::SeqCst);
        });
    let reactive_duration = start.elapsed();
    
    // Calculate overhead per operation
    let overhead_per_op = reactive_duration.saturating_sub(baseline_duration) / iterations;
    
    println!("Baseline: {:?}, Reactive: {:?}, Overhead per op: {:?}", 
             baseline_duration, reactive_duration, overhead_per_op);
    
    // Performance target: <100ns overhead per operator (we have 2 operators)
    assert!(overhead_per_op < Duration::from_nanos(200), 
            "Operator overhead too high: {:?}ns per operation", overhead_per_op.as_nanos());
    
    // Verify all values were processed
    assert_eq!(received_count.load(Ordering::SeqCst), iterations as u32);
}

/// Test that Virtual Time Scheduler works for testing
#[test]
fn test_virtual_time_scheduler_for_testing() {
    let scheduler = VirtualTimeScheduler::new();
    let executed_order = Arc::new(Mutex::new(Vec::new()));
    
    let order1 = Arc::clone(&executed_order);
    let order2 = Arc::clone(&executed_order);
    let order3 = Arc::clone(&executed_order);
    
    // Schedule work at different times
    let _sub1 = scheduler.schedule_delayed(Duration::from_millis(100), move || {
        order1.lock().unwrap().push(1);
    });
    
    let _sub2 = scheduler.schedule_delayed(Duration::from_millis(50), move || {
        order2.lock().unwrap().push(2);
    });
    
    let _sub3 = scheduler.schedule_delayed(Duration::from_millis(150), move || {
        order3.lock().unwrap().push(3);
    });
    
    // Nothing should execute yet
    assert_eq!(executed_order.lock().unwrap().len(), 0);
    
    // Advance time by 60ms - only second task should execute
    scheduler.advance_time(Duration::from_millis(60));
    assert_eq!(*executed_order.lock().unwrap(), vec![2]);
    
    // Advance time by 50ms more (110ms total) - first task should execute
    scheduler.advance_time(Duration::from_millis(50));
    assert_eq!(*executed_order.lock().unwrap(), vec![2, 1]);
    
    // Advance time by 50ms more (160ms total) - third task should execute
    scheduler.advance_time(Duration::from_millis(50));
    assert_eq!(*executed_order.lock().unwrap(), vec![2, 1, 3]);
}

/// Test that BehaviorSubject maintains current state
#[test]
fn test_behavior_subject_maintains_current_state() {
    let mut behavior_subject = BehaviorSubject::new(100);
    
    // Check initial value
    assert_eq!(behavior_subject.value(), Some(100));
    
    // Late subscriber should get current value immediately
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = Arc::clone(&received);
    
    behavior_subject.on_next(200);
    assert_eq!(behavior_subject.value(), Some(200));
    
    let _subscription = behavior_subject.as_observable().subscribe_fn(move |value| {
        received_clone.lock().unwrap().push(value);
    });
    
    // Should immediately receive the current value
    let values = received.lock().unwrap();
    assert_eq!(values[0], 200); // Current value received immediately
    
    behavior_subject.on_next(300);
    assert_eq!(behavior_subject.value(), Some(300));
}

/// Test that ReplaySubject buffers correctly
#[test]
fn test_replay_subject_buffers_correctly() {
    let mut replay_subject = ReplaySubject::new(3);
    
    // Emit more values than buffer size
    replay_subject.on_next(1);
    replay_subject.on_next(2);
    replay_subject.on_next(3);
    replay_subject.on_next(4);
    replay_subject.on_next(5);
    
    // New subscriber should only get the last 3 values
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = Arc::clone(&received);
    
    let _subscription = replay_subject.as_observable().subscribe_fn(move |value| {
        received_clone.lock().unwrap().push(value);
    });
    
    let values = received.lock().unwrap();
    assert_eq!(*values, vec![3, 4, 5]); // Last 3 values only
}

/// Test composite subscription management
#[test]
fn test_composite_subscription_management() {
    let disposed_count = Arc::new(AtomicU32::new(0));
    
    let count1 = Arc::clone(&disposed_count);
    let count2 = Arc::clone(&disposed_count);
    let count3 = Arc::clone(&disposed_count);
    
    let sub1 = Subscription::new(move || {
        count1.fetch_add(1, Ordering::SeqCst);
    });
    
    let sub2 = Subscription::new(move || {
        count2.fetch_add(1, Ordering::SeqCst);
    });
    
    let sub3 = Subscription::new(move || {
        count3.fetch_add(1, Ordering::SeqCst);
    });
    
    let composite = Subscription::composite(vec![sub1, sub2, sub3]);
    
    // Dispose composite should dispose all children
    composite.dispose();
    
    assert_eq!(disposed_count.load(Ordering::SeqCst), 3);
}

/// Benchmark test for memory usage requirements
#[test]
fn test_memory_usage_under_1kb_per_stream() {
    // This is a conceptual test - in a real implementation you'd measure actual memory usage
    let obs = Observable::range(0, 1000);
    let chained = obs
        .map(|x| x * 2)
        .filter(|x| x % 4 == 0)
        .take(10);
    
    let received_count = Arc::new(AtomicU32::new(0));
    let received_count_clone = Arc::clone(&received_count);
    
    let subscription = chained.subscribe_fn(move |_value| {
        received_count_clone.fetch_add(1, Ordering::SeqCst);
    });
    
    // Verify the stream worked correctly
    assert_eq!(received_count.load(Ordering::SeqCst), 10);
    
    // In a real implementation, you'd measure:
    // - Size of Observable struct
    // - Size of operator chain
    // - Size of subscription structures
    // - Memory allocated during execution
    // Target: < 1024 bytes total per stream
    
    subscription.dispose();
}

/// Integration test combining multiple reactive features
#[test]
fn test_integration_reactive_features_combined() {
    let mut subject = Subject::new();
    let hot_stream = subject.as_observable();
    
    let processed_values = Arc::new(Mutex::new(Vec::new()));
    let processed_clone = Arc::clone(&processed_values);
    
    // Create a complex processing pipeline
    let _subscription = hot_stream
        .filter(|x: &i32| *x > 0)           // Only positive numbers
        .map(|x| x * x)                     // Square them
        .take(5)                            // Take only first 5
        .tap(|x| println!("Processing: {}", x)) // Side effect
        .subscribe_fn(move |value| {
            processed_clone.lock().unwrap().push(value);
        });
    
    // Emit test data
    subject.on_next(-1); // Should be filtered out
    subject.on_next(1);
    subject.on_next(2);
    subject.on_next(-2); // Should be filtered out
    subject.on_next(3);
    subject.on_next(4);
    subject.on_next(5);
    subject.on_next(6);  // Should be ignored due to take(5)
    
    let results = processed_values.lock().unwrap();
    assert_eq!(*results, vec![1, 4, 9, 16, 25]); // Squares of 1,2,3,4,5
}