//! Flow coroutine integration for Seen Language reactive programming
//!
//! This module implements Flow according to Seen's syntax design:
//! - fun Numbers(): Flow<Int> = flow { for i in 1..10 { Emit(i); Delay(100.ms) } }
//! - Integration with coroutines and async/await system
//! - Backpressure handling and cancellation support
//! - Cold streams with lazy evaluation

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use futures::stream::{Stream, StreamExt};
use futures::Future;
use seen_lexer::position::Position;
use seen_parser::ast::Expression;
use seen_concurrency::types::{AsyncValue, AsyncError, AsyncResult, TaskId};
use crate::observable::{Observable, ObservableId};

/// Flow for cold reactive streams
#[derive(Debug)]
pub struct Flow<T> {
    /// Unique flow identifier
    pub id: FlowId,
    /// Flow name for debugging
    pub name: String,
    /// Flow producer function
    producer: Arc<dyn FlowProducer<T> + Send + Sync>,
    /// Flow metadata
    metadata: FlowMetadata,
    /// Cancellation token
    cancellation_token: Arc<Mutex<CancellationToken>>,
}

/// Unique identifier for flows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowId(u64);

impl FlowId {
    /// Create a new flow ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Flow metadata
#[derive(Debug, Clone)]
pub struct FlowMetadata {
    /// Flow creation position
    pub position: Position,
    /// Flow value type
    pub value_type: String,
    /// Whether flow is cold (default) or hot
    pub is_cold: bool,
    /// Flow creation time
    pub created_at: Instant,
    /// Whether flow supports cancellation
    pub supports_cancellation: bool,
    /// Whether flow supports backpressure
    pub supports_backpressure: bool,
}

/// Producer trait for flow values
pub trait FlowProducer<T>: std::fmt::Debug {
    /// Collect values into the flow collector
    fn collect(&self, collector: &mut FlowCollector<T>) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>>;
}

/// Collector for flow values
#[derive(Debug)]
pub struct FlowCollector<T> {
    /// Buffer for emitted values
    buffer: VecDeque<T>,
    /// Whether collection is completed
    completed: bool,
    /// Error state
    error: Option<AsyncError>,
    /// Cancellation token
    cancellation_token: Arc<Mutex<CancellationToken>>,
    /// Backpressure handling
    backpressure_config: BackpressureConfig,
}

/// Cancellation token for flows
#[derive(Debug, Clone)]
pub struct CancellationToken {
    /// Whether cancellation is requested
    pub is_cancelled: bool,
    /// Cancellation reason
    pub reason: Option<String>,
    /// Cancellation time
    pub cancelled_at: Option<Instant>,
}

/// Backpressure configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum buffer size before backpressure
    pub max_buffer_size: usize,
    /// Strategy for handling backpressure
    pub strategy: BackpressureStrategy,
    /// Timeout for backpressure resolution
    pub timeout: Duration,
}

/// Strategies for handling backpressure
#[derive(Debug, Clone)]
pub enum BackpressureStrategy {
    /// Drop newest values when buffer is full
    DropNewest,
    /// Drop oldest values when buffer is full
    DropOldest,
    /// Block until buffer has space
    Block,
    /// Error when buffer is full
    Error,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: 1000,
            strategy: BackpressureStrategy::Block,
            timeout: Duration::from_secs(30),
        }
    }
}

/// Flow builder for creating flows with Seen syntax
#[derive(Debug)]
pub struct FlowBuilder {
    /// Flow expression to execute
    expression: Expression,
    /// Flow name
    name: String,
    /// Flow position
    position: Position,
    /// Backpressure configuration
    backpressure_config: BackpressureConfig,
}

/// Simple flow producer for basic flows
#[derive(Debug)]
pub struct SimpleFlowProducer<T> {
    /// Values to emit
    values: Vec<T>,
    /// Delay between emissions
    delay: Option<Duration>,
}

/// Range flow producer
#[derive(Debug)]
pub struct RangeFlowProducer {
    /// Start value
    start: i64,
    /// End value
    end: i64,
    /// Step value
    step: i64,
    /// Delay between emissions
    delay: Option<Duration>,
}

/// Timer flow producer
#[derive(Debug)]
pub struct TimerFlowProducer {
    /// Interval duration
    interval: Duration,
    /// Maximum count (None for infinite)
    max_count: Option<u64>,
}

impl<T> Flow<T>
where
    T: Send + 'static,
{
    /// Create a new flow
    pub fn new<P>(name: String, producer: P, position: Position) -> Self
    where
        P: FlowProducer<T> + Send + Sync + 'static,
    {
        let id = FlowId::new(rand::random());
        
        Self {
            id,
            name: name.clone(),
            producer: Arc::new(producer),
            metadata: FlowMetadata {
                position,
                value_type: std::any::type_name::<T>().to_string(),
                is_cold: true,
                created_at: Instant::now(),
                supports_cancellation: true,
                supports_backpressure: true,
            },
            cancellation_token: Arc::new(Mutex::new(CancellationToken::new())),
        }
    }
    
    /// Convert flow to observable
    pub fn to_observable(self) -> Observable<T> {
        let flow_stream = FlowStream::new(self);
        Observable::from_stream(
            Box::pin(flow_stream),
            format!("Flow({})", self.name),
            self.metadata.position,
        )
    }
    
    /// Collect all values from the flow
    pub async fn collect_all(&self) -> Result<Vec<T>, AsyncError> {
        let mut collector = FlowCollector::new(self.cancellation_token.clone());
        self.producer.collect(&mut collector).await?;
        Ok(collector.get_values())
    }
    
    /// Take the first n values from the flow
    pub async fn take(&self, count: usize) -> Result<Vec<T>, AsyncError> {
        let mut collector = FlowCollector::new(self.cancellation_token.clone());
        let mut values = Vec::new();
        
        // Start collection
        let collection_future = self.producer.collect(&mut collector);
        tokio::pin!(collection_future);
        
        // Collect values until we have enough or flow completes
        loop {
            tokio::select! {
                result = &mut collection_future => {
                    result?;
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    let new_values = collector.drain_values();
                    for value in new_values {
                        values.push(value);
                        if values.len() >= count {
                            self.cancel("take limit reached").await?;
                            return Ok(values);
                        }
                    }
                }
            }
        }
        
        Ok(values)
    }
    
    /// Cancel the flow
    pub async fn cancel(&self, reason: &str) -> Result<(), AsyncError> {
        let mut token = self.cancellation_token.lock().unwrap();
        token.cancel(reason.to_string());
        Ok(())
    }
    
    /// Check if flow is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.lock().unwrap().is_cancelled
    }
    
    /// Get flow metadata
    pub fn metadata(&self) -> &FlowMetadata {
        &self.metadata
    }
}

impl<T> FlowCollector<T> {
    /// Create a new flow collector
    pub fn new(cancellation_token: Arc<Mutex<CancellationToken>>) -> Self {
        Self {
            buffer: VecDeque::new(),
            completed: false,
            error: None,
            cancellation_token,
            backpressure_config: BackpressureConfig::default(),
        }
    }
    
    /// Emit a value (Seen syntax: Emit(value))
    pub async fn emit(&mut self, value: T) -> Result<(), AsyncError> {
        // Check cancellation
        if self.cancellation_token.lock().unwrap().is_cancelled {
            return Err(AsyncError::TaskCancelled {
                task_id: TaskId::new(0), // Dummy task ID
            });
        }
        
        // Handle backpressure
        if self.buffer.len() >= self.backpressure_config.max_buffer_size {
            match self.backpressure_config.strategy {
                BackpressureStrategy::DropNewest => {
                    // Don't add the new value
                    return Ok(());
                }
                BackpressureStrategy::DropOldest => {
                    self.buffer.pop_front();
                }
                BackpressureStrategy::Block => {
                    // Wait for buffer to have space
                    let start = Instant::now();
                    while self.buffer.len() >= self.backpressure_config.max_buffer_size {
                        if start.elapsed() > self.backpressure_config.timeout {
                            return Err(AsyncError::TaskTimeout {
                                task_id: TaskId::new(0),
                                timeout_ms: self.backpressure_config.timeout.as_millis() as u64,
                            });
                        }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                }
                BackpressureStrategy::Error => {
                    return Err(AsyncError::RuntimeError {
                        message: "Flow buffer overflow".to_string(),
                        position: Position::new(0, 0, 0),
                    });
                }
            }
        }
        
        self.buffer.push_back(value);
        Ok(())
    }
    
    /// Delay execution (Seen syntax: Delay(duration))
    pub async fn delay(&self, duration: Duration) -> Result<(), AsyncError> {
        // Check cancellation before delay
        if self.cancellation_token.lock().unwrap().is_cancelled {
            return Err(AsyncError::TaskCancelled {
                task_id: TaskId::new(0),
            });
        }
        
        tokio::time::sleep(duration).await;
        
        // Check cancellation after delay
        if self.cancellation_token.lock().unwrap().is_cancelled {
            return Err(AsyncError::TaskCancelled {
                task_id: TaskId::new(0),
            });
        }
        
        Ok(())
    }
    
    /// Complete the flow
    pub fn complete(&mut self) {
        self.completed = true;
    }
    
    /// Set error state
    pub fn error(&mut self, error: AsyncError) {
        self.error = Some(error);
        self.completed = true;
    }
    
    /// Get all collected values
    pub fn get_values(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.buffer.iter().cloned().collect()
    }
    
    /// Drain values from buffer
    pub fn drain_values(&mut self) -> Vec<T> {
        self.buffer.drain(..).collect()
    }
    
    /// Check if collection is completed
    pub fn is_completed(&self) -> bool {
        self.completed
    }
    
    /// Get error if any
    pub fn get_error(&self) -> Option<&AsyncError> {
        self.error.as_ref()
    }
}

impl CancellationToken {
    /// Create a new cancellation token
    pub fn new() -> Self {
        Self {
            is_cancelled: false,
            reason: None,
            cancelled_at: None,
        }
    }
    
    /// Cancel the token
    pub fn cancel(&mut self, reason: String) {
        self.is_cancelled = true;
        self.reason = Some(reason);
        self.cancelled_at = Some(Instant::now());
    }
    
    /// Reset the token
    pub fn reset(&mut self) {
        self.is_cancelled = false;
        self.reason = None;
        self.cancelled_at = None;
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream implementation for flows
#[derive(Debug)]
pub struct FlowStream<T> {
    /// The flow being streamed
    flow: Flow<T>,
    /// Collection future
    collection_future: Option<Pin<Box<dyn Future<Output = AsyncResult> + Send>>>,
    /// Collector for values
    collector: FlowCollector<T>,
    /// Whether stream has started
    started: bool,
}

impl<T> FlowStream<T>
where
    T: Send + 'static,
{
    /// Create a new flow stream
    pub fn new(flow: Flow<T>) -> Self {
        let collector = FlowCollector::new(flow.cancellation_token.clone());
        
        Self {
            flow,
            collection_future: None,
            collector,
            started: false,
        }
    }
}

impl<T> Stream for FlowStream<T>
where
    T: Send + Unpin + 'static,
{
    type Item = T;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Start collection if not started
        if !self.started {
            let collector_ref = &mut self.collector as *mut FlowCollector<T>;
            let future = unsafe {
                self.flow.producer.collect(&mut *collector_ref)
            };
            self.collection_future = Some(future);
            self.started = true;
        }
        
        // Poll the collection future
        if let Some(ref mut future) = self.collection_future {
            match future.as_mut().poll(cx) {
                Poll::Ready(Ok(_)) => {
                    // Collection completed
                    self.collection_future = None;
                }
                Poll::Ready(Err(_)) => {
                    // Collection failed
                    self.collection_future = None;
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    // Collection still in progress
                }
            }
        }
        
        // Check for available values
        if let Some(value) = self.collector.buffer.pop_front() {
            return Poll::Ready(Some(value));
        }
        
        // Check if completed
        if self.collector.is_completed() {
            return Poll::Ready(None);
        }
        
        Poll::Pending
    }
}

impl<T> FlowProducer<T> for SimpleFlowProducer<T>
where
    T: Clone + Send + 'static,
{
    fn collect(&self, collector: &mut FlowCollector<T>) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let values = self.values.clone();
        let delay = self.delay;
        
        Box::pin(async move {
            for value in values {
                collector.emit(value).await?;
                
                if let Some(delay_duration) = delay {
                    collector.delay(delay_duration).await?;
                }
            }
            
            collector.complete();
            Ok(AsyncValue::Unit)
        })
    }
}

impl FlowProducer<i64> for RangeFlowProducer {
    fn collect(&self, collector: &mut FlowCollector<i64>) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let start = self.start;
        let end = self.end;
        let step = self.step;
        let delay = self.delay;
        
        Box::pin(async move {
            let mut current = start;
            
            while (step > 0 && current < end) || (step < 0 && current > end) {
                collector.emit(current).await?;
                current += step;
                
                if let Some(delay_duration) = delay {
                    collector.delay(delay_duration).await?;
                }
            }
            
            collector.complete();
            Ok(AsyncValue::Unit)
        })
    }
}

impl FlowProducer<u64> for TimerFlowProducer {
    fn collect(&self, collector: &mut FlowCollector<u64>) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
        let interval = self.interval;
        let max_count = self.max_count;
        
        Box::pin(async move {
            let mut count = 0u64;
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                collector.emit(count).await?;
                count += 1;
                
                if let Some(max) = max_count {
                    if count >= max {
                        break;
                    }
                }
            }
            
            collector.complete();
            Ok(AsyncValue::Unit)
        })
    }
}

/// Flow factory for creating common flows
#[derive(Debug)]
pub struct FlowFactory;

impl FlowFactory {
    /// Create a flow from a vector
    pub fn from_vec<T>(values: Vec<T>) -> Flow<T>
    where
        T: Clone + Send + 'static,
    {
        let producer = SimpleFlowProducer {
            values,
            delay: None,
        };
        
        Flow::new(
            "FromVec".to_string(),
            producer,
            Position::new(0, 0, 0),
        )
    }
    
    /// Create a range flow
    pub fn range(start: i64, end: i64, step: i64) -> Flow<i64> {
        let producer = RangeFlowProducer {
            start,
            end,
            step,
            delay: None,
        };
        
        Flow::new(
            format!("Range({}, {}, {})", start, end, step),
            producer,
            Position::new(0, 0, 0),
        )
    }
    
    /// Create a timer flow
    pub fn timer(interval: Duration, max_count: Option<u64>) -> Flow<u64> {
        let producer = TimerFlowProducer {
            interval,
            max_count,
        };
        
        Flow::new(
            format!("Timer({:?})", interval),
            producer,
            Position::new(0, 0, 0),
        )
    }
    
    /// Create a flow with delay between emissions
    pub fn with_delay<T>(values: Vec<T>, delay: Duration) -> Flow<T>
    where
        T: Clone + Send + 'static,
    {
        let producer = SimpleFlowProducer {
            values,
            delay: Some(delay),
        };
        
        Flow::new(
            "WithDelay".to_string(),
            producer,
            Position::new(0, 0, 0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_flow_creation() {
        let flow = FlowFactory::from_vec(vec![1, 2, 3]);
        
        assert!(!flow.is_cancelled());
        assert!(flow.metadata.is_cold);
        assert!(flow.metadata.supports_cancellation);
    }
    
    #[tokio::test]
    async fn test_flow_collect_all() {
        let flow = FlowFactory::from_vec(vec![1, 2, 3]);
        let values = flow.collect_all().await.unwrap();
        
        assert_eq!(values, vec![1, 2, 3]);
    }
    
    #[tokio::test]
    async fn test_flow_take() {
        let flow = FlowFactory::range(1, 100, 1);
        let values = flow.take(5).await.unwrap();
        
        assert_eq!(values, vec![1, 2, 3, 4, 5]);
    }
    
    #[tokio::test]
    async fn test_flow_cancellation() {
        let flow = FlowFactory::timer(Duration::from_millis(10), None);
        
        // Cancel after a short time
        tokio::time::sleep(Duration::from_millis(50)).await;
        flow.cancel("test cancellation").await.unwrap();
        
        assert!(flow.is_cancelled());
    }
    
    #[tokio::test]
    async fn test_range_flow() {
        let flow = FlowFactory::range(5, 10, 2);
        let values = flow.collect_all().await.unwrap();
        
        assert_eq!(values, vec![5, 7, 9]);
    }
    
    #[tokio::test]
    async fn test_flow_with_delay() {
        let start = Instant::now();
        let flow = FlowFactory::with_delay(vec![1, 2], Duration::from_millis(10));
        let _values = flow.collect_all().await.unwrap();
        
        // Should take at least 10ms due to delay
        assert!(start.elapsed() >= Duration::from_millis(10));
    }
    
    #[tokio::test]
    async fn test_flow_to_observable() {
        let flow = FlowFactory::from_vec(vec![1, 2, 3]);
        let observable = flow.to_observable();
        
        assert!(observable.metadata().name.contains("Flow"));
    }
    
    #[test]
    fn test_cancellation_token() {
        let mut token = CancellationToken::new();
        
        assert!(!token.is_cancelled);
        
        token.cancel("test reason".to_string());
        
        assert!(token.is_cancelled);
        assert_eq!(token.reason, Some("test reason".to_string()));
        assert!(token.cancelled_at.is_some());
    }
    
    #[tokio::test]
    async fn test_flow_collector_emit() {
        let token = Arc::new(Mutex::new(CancellationToken::new()));
        let mut collector = FlowCollector::new(token);
        
        collector.emit(42).await.unwrap();
        collector.emit(43).await.unwrap();
        
        let values = collector.get_values();
        assert_eq!(values, vec![42, 43]);
    }
    
    #[tokio::test]
    async fn test_flow_collector_delay() {
        let token = Arc::new(Mutex::new(CancellationToken::new()));
        let collector = FlowCollector::new(token);
        
        let start = Instant::now();
        collector.delay(Duration::from_millis(10)).await.unwrap();
        
        assert!(start.elapsed() >= Duration::from_millis(10));
    }
}