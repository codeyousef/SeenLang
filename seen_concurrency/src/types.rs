//! Type definitions for async and concurrency features

use seen_lexer::position::Position;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Unique identifier for async tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId {
    slot: u32,
    generation: u32,
}

impl TaskId {
    /// Create a new task ID from slot/generation parts
    pub fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Create a placeholder task ID (used for synthetic promises)
    pub fn placeholder() -> Self {
        Self {
            slot: u32::MAX,
            generation: u32::MAX,
        }
    }

    /// Slot index backing this ID
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Generation component for stale-handle detection
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Raw 64-bit value (generation in upper 32 bits, slot in lower)
    pub fn id(&self) -> u64 {
        ((self.generation as u64) << 32) | self.slot as u64
    }

    /// Produce the next generation for this slot
    pub fn next_generation(self) -> Self {
        Self {
            slot: self.slot,
            generation: self.generation.wrapping_add(1),
        }
    }
}

/// Handle for managing async tasks
#[derive(Debug, Clone)]
pub struct TaskHandle {
    /// Task ID (None if task creation failed)
    task_id: Option<TaskId>,
    /// Error if task creation failed
    error: Option<AsyncError>,
}

impl TaskHandle {
    /// Create a new task handle
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id: Some(task_id),
            error: None,
        }
    }

    /// Create an error task handle
    pub fn error(error: AsyncError) -> Self {
        Self {
            task_id: None,
            error: Some(error),
        }
    }

    /// Get the task ID if available
    pub fn task_id(&self) -> Option<TaskId> {
        self.task_id
    }

    /// Check if the handle represents an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the error if present
    pub fn get_error(&self) -> Option<&AsyncError> {
        self.error.as_ref()
    }
}

/// Task execution states
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskState {
    /// Task is ready to run
    Ready,
    /// Task is currently executing
    Running,
    /// Task is waiting for I/O or other async operations
    Waiting,
    /// Task has completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Low priority task
    Low = 0,
    /// Normal priority task
    Normal = 1,
    /// High priority task
    High = 2,
}

/// Values that can be produced by async operations
#[derive(Debug, Clone)]
pub enum AsyncValue {
    /// Unit/void value
    Unit,
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// String value
    String(String),
    /// Array of async values
    Array(Vec<AsyncValue>),
    /// Promise for future value
    Promise(Arc<Promise>),
    /// Channel for communication
    Channel(Channel),
    /// Actor reference
    Actor(ActorRef),
    /// Error value representing a failed operation
    Error,
    /// Pending value representing an incomplete async operation
    Pending,
}

/// Result type for async operations
pub type AsyncResult = Result<AsyncValue, AsyncError>;

impl PartialEq for AsyncValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AsyncValue::Unit, AsyncValue::Unit) => true,
            (AsyncValue::Integer(a), AsyncValue::Integer(b)) => a == b,
            (AsyncValue::Float(a), AsyncValue::Float(b)) => a == b,
            (AsyncValue::Boolean(a), AsyncValue::Boolean(b)) => a == b,
            (AsyncValue::String(a), AsyncValue::String(b)) => a == b,
            (AsyncValue::Array(a), AsyncValue::Array(b)) => a == b,
            (AsyncValue::Error, AsyncValue::Error) => true,
            (AsyncValue::Pending, AsyncValue::Pending) => true,
            // For complex types with Arc, compare by pointer identity
            (AsyncValue::Promise(a), AsyncValue::Promise(b)) => Arc::ptr_eq(a, b),
            (AsyncValue::Channel(a), AsyncValue::Channel(b)) => a == b,
            (AsyncValue::Actor(a), AsyncValue::Actor(b)) => a == b,
            _ => false,
        }
    }
}

/// Errors that can occur in async operations
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
pub enum AsyncError {
    /// Task not found
    #[error("Task {task_id:?} not found")]
    TaskNotFound { task_id: TaskId },

    /// Task creation failed
    #[error("Task creation failed: {reason} at {position}")]
    TaskCreationFailed { reason: String, position: Position },

    /// Task limit exceeded
    #[error("Task limit exceeded: {limit} at {position}")]
    TaskLimitExceeded { limit: usize, position: Position },

    /// Task execution timeout
    #[error("Task {task_id:?} timed out after {timeout_ms}ms")]
    TaskTimeout { task_id: TaskId, timeout_ms: u64 },

    /// Task was cancelled
    #[error("Task {task_id:?} was cancelled")]
    TaskCancelled { task_id: TaskId },

    /// Channel operation failed
    #[error("Channel operation failed: {reason} at {position}")]
    ChannelError { reason: String, position: Position },

    /// Actor operation failed
    #[error("Actor operation failed: {reason} at {position}")]
    ActorError { reason: String, position: Position },

    /// Promise already resolved
    #[error("Promise already resolved at {position}")]
    PromiseAlreadyResolved { position: Position },

    /// Promise rejected with error
    #[error("Promise rejected: {reason} at {position}")]
    PromiseRejected { reason: String, position: Position },

    /// Await operation failed
    #[error("Await failed: {reason} at {position}")]
    AwaitFailed { reason: String, position: Position },

    /// Runtime error during async execution
    #[error("Runtime error: {message} at {position}")]
    RuntimeError { message: String, position: Position },
}

/// Promise for representing future values
#[derive(Debug, Clone)]
pub struct Promise {
    /// Task ID this promise is associated with
    task_id: TaskId,
    /// Current state of the promise
    state: PromiseState,
    /// Resolved value (if completed successfully)
    value: Option<AsyncValue>,
    /// Rejection error (if failed)
    error: Option<String>, // Simplified to string
}

/// Promise states
#[derive(Debug, Clone, PartialEq)]
pub enum PromiseState {
    /// Promise is still pending
    Pending,
    /// Promise resolved successfully
    Resolved,
    /// Promise was rejected with error
    Rejected,
}

impl Promise {
    /// Create a new pending promise
    pub fn new(task_id: TaskId) -> Self {
        Self {
            task_id,
            state: PromiseState::Pending,
            value: None,
            error: None,
        }
    }

    /// Create a resolved promise
    pub fn resolved(value: AsyncValue) -> Self {
        Self {
            task_id: TaskId::placeholder(), // Synthetic ID for resolved promises
            state: PromiseState::Resolved,
            value: Some(value),
            error: None,
        }
    }

    /// Create a rejected promise
    pub fn rejected(error: String) -> Self {
        Self {
            task_id: TaskId::placeholder(), // Synthetic ID for rejected promises
            state: PromiseState::Rejected,
            value: None,
            error: Some(error),
        }
    }

    /// Resolve the promise with a value
    pub fn resolve(&mut self, value: AsyncValue) {
        if self.state == PromiseState::Pending {
            self.state = PromiseState::Resolved;
            self.value = Some(value);
        }
    }

    /// Reject the promise with an error
    pub fn reject(&mut self, error: String) {
        if self.state == PromiseState::Pending {
            self.state = PromiseState::Rejected;
            self.error = Some(error);
        }
    }

    /// Check if promise is pending
    pub fn is_pending(&self) -> bool {
        self.state == PromiseState::Pending
    }

    /// Check if promise is resolved
    pub fn is_resolved(&self) -> bool {
        self.state == PromiseState::Resolved
    }

    /// Check if promise is rejected
    pub fn is_rejected(&self) -> bool {
        self.state == PromiseState::Rejected
    }

    /// Get the resolved value if available
    pub fn value(&self) -> Option<&AsyncValue> {
        self.value.as_ref()
    }

    /// Get the rejection error if available
    pub fn get_error(&self) -> Option<&String> {
        self.error.as_ref()
    }

    /// Get the task ID
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }
}

/// Shared state for a channel; kept behind an `Arc` so handles can cheaply clone.
#[derive(Debug)]
struct ChannelInner {
    slot: u32,
    capacity: Option<usize>,
    state: Mutex<ChannelState>,
    queue: Mutex<VecDeque<AsyncValue>>,
    generation: AtomicU32,
}

impl ChannelInner {
    fn new(slot: u32, capacity: Option<usize>, generation: u32) -> Self {
        Self {
            slot,
            capacity,
            state: Mutex::new(ChannelState::Open),
            queue: Mutex::new(VecDeque::new()),
            generation: AtomicU32::new(generation),
        }
    }
}

/// Channel handle for async message passing with generational safety.
#[derive(Debug, Clone)]
pub struct Channel {
    inner: Arc<ChannelInner>,
    expected_generation: u32,
}

impl Channel {
    fn ensure_fresh(&self) -> Result<(), String> {
        let current = self.inner.generation.load(Ordering::SeqCst);
        if current != self.expected_generation {
            Err("Stale channel handle".to_string())
        } else {
            Ok(())
        }
    }

    /// Create a new channel handle for the provided identifier/capacity.
    pub fn new(id: ChannelId, capacity: Option<usize>) -> Self {
        let inner = Arc::new(ChannelInner::new(id.slot(), capacity, id.generation()));
        Self {
            inner,
            expected_generation: id.generation(),
        }
    }

    fn refresh(&self) -> Self {
        let generation = self.inner.generation.load(Ordering::SeqCst);
        Self {
            inner: Arc::clone(&self.inner),
            expected_generation: generation,
        }
    }

    /// Access the channel identifier (slot + current generation).
    pub fn id(&self) -> ChannelId {
        ChannelId::new(
            self.inner.slot,
            self.inner.generation.load(Ordering::SeqCst),
        )
    }

    /// Channel capacity (None for unbounded).
    pub fn capacity(&self) -> Option<usize> {
        self.inner.capacity
    }

    /// Send a value to the channel (blocking semantics).
    pub fn send(&self, value: AsyncValue) -> Result<(), String> {
        self.ensure_fresh()?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "Channel state poisoned".to_string())?;

        match &mut *state {
            ChannelState::Open => {
                let mut queue = self
                    .inner
                    .queue
                    .lock()
                    .map_err(|_| "Channel queue poisoned".to_string())?;

                if let Some(cap) = self.inner.capacity {
                    if queue.len() >= cap {
                        return Err("Channel is full".to_string());
                    }
                }

                queue.push_back(value);
                Ok(())
            }
            ChannelState::Closed => Err("Channel is closed".to_string()),
            ChannelState::Error(err) => Err(err.clone()),
        }
    }

    /// Try to receive a value from the channel without blocking.
    pub fn try_recv(&self) -> Result<AsyncValue, String> {
        self.ensure_fresh()?;
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| "Channel state poisoned".to_string())?;

        match &*state {
            ChannelState::Open => {
                let mut queue = self
                    .inner
                    .queue
                    .lock()
                    .map_err(|_| "Channel queue poisoned".to_string())?;
                queue
                    .pop_front()
                    .ok_or_else(|| "No message available".to_string())
            }
            ChannelState::Closed => {
                let mut queue = self
                    .inner
                    .queue
                    .lock()
                    .map_err(|_| "Channel queue poisoned".to_string())?;
                queue
                    .pop_front()
                    .ok_or_else(|| "Channel is closed".to_string())
            }
            ChannelState::Error(err) => Err(err.clone()),
        }
    }

    /// Mark the channel as closed and invalidate existing handles.
    pub fn close(&self) {
        if self.ensure_fresh().is_err() {
            return;
        }

        if let Ok(mut state) = self.inner.state.lock() {
            *state = ChannelState::Closed;
            self.inner.generation.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Check if the channel currently has zero buffered messages.
    pub fn is_empty(&self) -> bool {
        if self.ensure_fresh().is_err() {
            return true;
        }
        self.inner
            .queue
            .lock()
            .map(|queue| queue.is_empty())
            .unwrap_or(true)
    }

    /// Buffered message count (0 on stale handles).
    pub fn len(&self) -> usize {
        if self.ensure_fresh().is_err() {
            return 0;
        }
        self.inner
            .queue
            .lock()
            .map(|queue| queue.len())
            .unwrap_or(0)
    }

    /// Internal helper to propagate an updated generation after structural changes.
    pub fn with_refreshed_generation(&self) -> Self {
        self.refresh()
    }
}

impl PartialEq for Channel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
            && self.expected_generation == other.expected_generation
    }
}

impl Eq for Channel {}

/// Unique identifier for channels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId {
    slot: u32,
    generation: u32,
}

impl ChannelId {
    /// Allocate a brand-new channel identifier.
    pub fn allocate() -> Self {
        static NEXT_SLOT: AtomicU32 = AtomicU32::new(1);
        let slot = NEXT_SLOT.fetch_add(1, Ordering::SeqCst);
        Self {
            slot,
            generation: 0,
        }
    }

    /// Construct from explicit slot/generation parts.
    pub fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Slot component of the identifier.
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Generation component of the identifier.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Encode as a u64 (generation in the high 32 bits).
    pub fn id(&self) -> u64 {
        ((self.generation as u64) << 32) | self.slot as u64
    }

    /// Produce the next generation for this slot.
    pub fn next_generation(self) -> Self {
        Self {
            slot: self.slot,
            generation: self.generation.wrapping_add(1),
        }
    }
}

/// Channel states
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelState {
    /// Channel is open for sending/receiving
    Open,
    /// Channel is closed
    Closed,
    /// Channel has error
    Error(String),
}

/// Shared state for an actor reference with generational tracking
#[derive(Debug)]
struct ActorInner {
    slot: u32,
    actor_type: String,
    mailbox: Arc<Mailbox>,
    generation: AtomicU32,
}

impl ActorInner {
    fn new(slot: u32, actor_type: String, mailbox: Arc<Mailbox>, generation: u32) -> Self {
        Self {
            slot,
            actor_type,
            mailbox,
            generation: AtomicU32::new(generation),
        }
    }
}

/// Actor reference for message-based concurrency
#[derive(Debug, Clone)]
pub struct ActorRef {
    inner: Arc<ActorInner>,
    expected_generation: u32,
}

impl ActorRef {
    fn ensure_fresh(&self) -> Result<(), String> {
        let current = self.inner.generation.load(Ordering::SeqCst);
        if current != self.expected_generation {
            Err("Stale actor handle".to_string())
        } else {
            Ok(())
        }
    }

    /// Create a new actor reference from its components.
    pub fn new(id: ActorId, actor_type: String, mailbox: Arc<Mailbox>) -> Self {
        let inner = Arc::new(ActorInner::new(
            id.slot(),
            actor_type,
            mailbox,
            id.generation(),
        ));
        Self {
            inner,
            expected_generation: id.generation(),
        }
    }

    /// Actor identifier for this handle.
    pub fn id(&self) -> ActorId {
        ActorId::new(
            self.inner.slot,
            self.inner.generation.load(Ordering::SeqCst),
        )
    }

    /// Actor type name (for diagnostics).
    pub fn actor_type(&self) -> &str {
        &self.inner.actor_type
    }

    /// Access the actor mailbox (cloned Arc).
    pub fn mailbox(&self) -> Result<Arc<Mailbox>, String> {
        self.ensure_fresh()?;
        Ok(Arc::clone(&self.inner.mailbox))
    }

    /// Invalidate this handle (e.g., when actor stops).
    pub fn invalidate(&self) {
        self.inner.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Produce a refreshed handle tracking the current generation.
    pub fn refreshed(&self) -> Self {
        let generation = self.inner.generation.load(Ordering::SeqCst);
        Self {
            inner: Arc::clone(&self.inner),
            expected_generation: generation,
        }
    }
}

impl PartialEq for ActorRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
            && self.expected_generation == other.expected_generation
    }
}

impl Eq for ActorRef {}

/// Unique identifier for actors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId {
    slot: u32,
    generation: u32,
}

impl ActorId {
    /// Allocate a unique actor identifier.
    pub fn allocate() -> Self {
        static NEXT_SLOT: AtomicU32 = AtomicU32::new(1);
        let slot = NEXT_SLOT.fetch_add(1, Ordering::SeqCst);
        Self {
            slot,
            generation: 0,
        }
    }

    /// Construct from explicit slot/generation components.
    pub fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    /// Slot component of the identifier.
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// Generation component of the identifier.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Encode as a 64-bit identifier.
    pub fn id(&self) -> u64 {
        ((self.generation as u64) << 32) | self.slot as u64
    }

    /// Produce the next generation for this actor slot.
    pub fn next_generation(self) -> Self {
        Self {
            slot: self.slot,
            generation: self.generation.wrapping_add(1),
        }
    }
}

/// Mailbox for actor message passing
#[derive(Debug)]
pub struct Mailbox {
    /// Messages waiting to be processed
    pub messages: std::sync::Mutex<std::collections::VecDeque<ActorMessage>>,
    /// Mailbox capacity
    pub capacity: Option<usize>,
}

/// Message sent to actors
#[derive(Debug, Clone)]
pub struct ActorMessage {
    /// Message sender
    pub sender: Option<ActorId>,
    /// Message content
    pub content: AsyncValue,
    /// Message timestamp
    pub timestamp: std::time::SystemTime,
    /// Message priority
    pub priority: TaskPriority,
}

/// Channel operations
#[derive(Debug, Clone)]
pub enum ChannelOperation {
    /// Send a value to the channel
    Send {
        channel_id: ChannelId,
        value: AsyncValue,
    },
    /// Receive a value from the channel
    Receive { channel_id: ChannelId },
    /// Close the channel
    Close { channel_id: ChannelId },
}

/// Actor operations
#[derive(Debug, Clone)]
pub enum ActorOperation {
    /// Send a message to an actor
    Send {
        actor_id: ActorId,
        message: ActorMessage,
    },
    /// Spawn a new actor
    Spawn {
        actor_type: String,
        init_params: Vec<AsyncValue>,
    },
    /// Stop an actor
    Stop { actor_id: ActorId },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[test]
    fn test_task_id_creation() {
        let id = TaskId::new(42, 7);
        assert_eq!(id.slot(), 42);
        assert_eq!(id.generation(), 7);
        assert_eq!(id.id(), ((7u64) << 32) | 42);
    }

    #[test]
    fn test_task_handle() {
        let task_id = TaskId::new(1, 0);
        let handle = TaskHandle::new(task_id);

        assert_eq!(handle.task_id(), Some(task_id));
        assert!(!handle.is_error());
    }

    #[test]
    fn test_task_handle_error() {
        let error = AsyncError::TaskCreationFailed {
            reason: "test error".to_string(),
            position: Position::new(1, 1, 0),
        };

        let handle = TaskHandle::error(error.clone());
        assert!(handle.is_error());
        assert_eq!(handle.task_id(), None);
    }

    #[test]
    fn test_promise_creation() {
        let task_id = TaskId::new(1, 0);
        let promise = Promise::new(task_id);

        assert!(promise.is_pending());
        assert!(!promise.is_resolved());
        assert!(!promise.is_rejected());
        assert_eq!(promise.task_id(), task_id);
    }

    #[test]
    fn test_promise_resolution() {
        let task_id = TaskId::new(1, 0);
        let mut promise = Promise::new(task_id);

        let value = AsyncValue::Integer(42);
        promise.resolve(value.clone());

        assert!(promise.is_resolved());
        assert_eq!(promise.value(), Some(&value));
    }

    #[test]
    fn test_promise_rejection() {
        let task_id = TaskId::new(1, 0);
        let mut promise = Promise::new(task_id);

        let error = "test error".to_string();

        promise.reject(error.clone());

        assert!(promise.is_rejected());
        assert_eq!(promise.get_error(), Some(&error));
    }

    #[test]
    fn test_async_value_types() {
        assert_eq!(AsyncValue::Integer(42), AsyncValue::Integer(42));

        assert_eq!(
            AsyncValue::String("test".to_string()),
            AsyncValue::String("test".to_string())
        );

        assert_eq!(AsyncValue::Unit, AsyncValue::Unit);
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[test]
    fn test_channel_id() {
        let id = ChannelId::new(123, 5);
        assert_eq!(id.slot(), 123);
        assert_eq!(id.generation(), 5);
        assert_eq!(id.id(), ((5u64) << 32) | 123);
    }

    #[test]
    fn test_actor_id() {
        let id = ActorId::new(456, 2);
        assert_eq!(id.slot(), 456);
        assert_eq!(id.generation(), 2);
        assert_eq!(id.id(), ((2u64) << 32) | 456);
    }

    #[test]
    fn channel_handle_invalidated_on_close() {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, None);
        channel.close();
        let result = channel.try_recv();
        assert!(matches!(result, Err(err) if err.contains("Stale")));
    }

    #[test]
    fn actor_ref_invalidated_blocks_mailbox_access() {
        let mailbox = Arc::new(Mailbox {
            messages: Mutex::new(VecDeque::new()),
            capacity: None,
        });
        let actor_ref = ActorRef::new(ActorId::new(10, 0), "TestActor".to_string(), mailbox);
        actor_ref.invalidate();
        assert!(actor_ref.mailbox().is_err());
    }
}
