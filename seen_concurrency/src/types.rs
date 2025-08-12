//! Type definitions for async and concurrency features

use std::sync::Arc;
use seen_lexer::position::Position;
use serde::{Serialize, Deserialize};

/// Unique identifier for async tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(u64);

impl TaskId {
    /// Create a new task ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
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
    Channel(Arc<Channel>),
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
            (AsyncValue::Channel(a), AsyncValue::Channel(b)) => Arc::ptr_eq(a, b),
            (AsyncValue::Actor(a), AsyncValue::Actor(b)) => a.id == b.id,
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
    TaskCreationFailed {
        reason: String,
        position: Position,
    },
    
    /// Task limit exceeded
    #[error("Task limit exceeded: {limit} at {position}")]
    TaskLimitExceeded {
        limit: usize,
        position: Position,
    },
    
    /// Task execution timeout
    #[error("Task {task_id:?} timed out after {timeout_ms}ms")]
    TaskTimeout {
        task_id: TaskId,
        timeout_ms: u64,
    },
    
    /// Task was cancelled
    #[error("Task {task_id:?} was cancelled")]
    TaskCancelled { task_id: TaskId },
    
    /// Channel operation failed
    #[error("Channel operation failed: {reason} at {position}")]
    ChannelError {
        reason: String,
        position: Position,
    },
    
    /// Actor operation failed
    #[error("Actor operation failed: {reason} at {position}")]
    ActorError {
        reason: String,
        position: Position,
    },
    
    /// Promise already resolved
    #[error("Promise already resolved at {position}")]
    PromiseAlreadyResolved { position: Position },
    
    /// Promise rejected with error
    #[error("Promise rejected: {reason} at {position}")]
    PromiseRejected {
        reason: String,
        position: Position,
    },
    
    /// Await operation failed
    #[error("Await failed: {reason} at {position}")]
    AwaitFailed {
        reason: String,
        position: Position,
    },
    
    /// Runtime error during async execution
    #[error("Runtime error: {message} at {position}")]
    RuntimeError {
        message: String,
        position: Position,
    },
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
            task_id: TaskId::new(0), // Synthetic ID for resolved promises
            state: PromiseState::Resolved,
            value: Some(value),
            error: None,
        }
    }
    
    /// Create a rejected promise
    pub fn rejected(error: String) -> Self {
        Self {
            task_id: TaskId::new(0), // Synthetic ID for rejected promises
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

/// Channel for async message passing
#[derive(Debug)]
pub struct Channel {
    /// Channel ID
    pub id: ChannelId,
    /// Channel capacity (None for unbounded)
    pub capacity: Option<usize>,
    /// Channel state
    pub state: ChannelState,
    /// Sender count
    pub sender_count: usize,
    /// Receiver count
    pub receiver_count: usize,
}

/// Unique identifier for channels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(u64);

impl ChannelId {
    /// Create a new channel ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
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

/// Actor reference for message-based concurrency
#[derive(Debug, Clone)]
pub struct ActorRef {
    /// Actor ID
    pub id: ActorId,
    /// Actor type name
    pub actor_type: String,
    /// Mailbox for receiving messages
    pub mailbox: Arc<Mailbox>,
}

/// Unique identifier for actors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorId(u64);

impl ActorId {
    /// Create a new actor ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
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
    Receive {
        channel_id: ChannelId,
    },
    /// Close the channel
    Close {
        channel_id: ChannelId,
    },
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
    Stop {
        actor_id: ActorId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_task_id_creation() {
        let id = TaskId::new(42);
        assert_eq!(id.id(), 42);
    }
    
    #[test]
    fn test_task_handle() {
        let task_id = TaskId::new(1);
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
        let task_id = TaskId::new(1);
        let promise = Promise::new(task_id);
        
        assert!(promise.is_pending());
        assert!(!promise.is_resolved());
        assert!(!promise.is_rejected());
        assert_eq!(promise.task_id(), task_id);
    }
    
    #[test]
    fn test_promise_resolution() {
        let task_id = TaskId::new(1);
        let mut promise = Promise::new(task_id);
        
        let value = AsyncValue::Integer(42);
        promise.resolve(value.clone());
        
        assert!(promise.is_resolved());
        assert_eq!(promise.value(), Some(&value));
    }
    
    #[test]
    fn test_promise_rejection() {
        let task_id = TaskId::new(1);
        let mut promise = Promise::new(task_id);
        
        let error = "test error".to_string();
        
        promise.reject(error.clone());
        
        assert!(promise.is_rejected());
        assert_eq!(promise.get_error(), Some(&error));
    }
    
    #[test]
    fn test_async_value_types() {
        assert_eq!(
            AsyncValue::Integer(42), 
            AsyncValue::Integer(42)
        );
        
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
        let id = ChannelId::new(123);
        assert_eq!(id.id(), 123);
    }
    
    #[test]
    fn test_actor_id() {
        let id = ActorId::new(456);
        assert_eq!(id.id(), 456);
    }
}