//! Type definitions for async and concurrency features

use seen_lexer::position::Position;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

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
    /// Array of async values (used for structured results like select outcomes)
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
    waiting_senders: Mutex<Vec<Waker>>,
    waiting_receivers: Mutex<Vec<Waker>>,
}

impl ChannelInner {
    fn new(slot: u32, capacity: Option<usize>, generation: u32) -> Self {
        Self {
            slot,
            capacity,
            state: Mutex::new(ChannelState::Open),
            queue: Mutex::new(VecDeque::new()),
            generation: AtomicU32::new(generation),
            waiting_senders: Mutex::new(Vec::new()),
            waiting_receivers: Mutex::new(Vec::new()),
        }
    }

    fn register_sender_waker(&self, waker: &Waker) {
        if let Ok(mut waiters) = self.waiting_senders.lock() {
            waiters.push(waker.clone());
        }
    }

    fn register_receiver_waker(&self, waker: &Waker) {
        if let Ok(mut waiters) = self.waiting_receivers.lock() {
            waiters.push(waker.clone());
        }
    }

    fn notify_sender_waiters(&self) {
        if let Ok(mut waiters) = self.waiting_senders.lock() {
            for waker in waiters.drain(..) {
                waker.wake_by_ref();
            }
        }
    }

    fn notify_receiver_waiters(&self) {
        if let Ok(mut waiters) = self.waiting_receivers.lock() {
            for waker in waiters.drain(..) {
                waker.wake_by_ref();
            }
        }
    }
}

/// Channel handle for async message passing with generational safety.
#[derive(Debug, Clone)]
pub struct Channel {
    inner: Arc<ChannelInner>,
    expected_generation: u32,
}

/// Result of attempting to send a value into a channel.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelSendStatus {
    Sent,
    WouldBlock,
    Closed,
    Error(String),
}

/// Result of attempting to receive a value from a channel.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelReceiveStatus {
    Received(AsyncValue),
    WouldBlock,
    Closed,
    Error(String),
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

    pub fn register_sender_waker(&self, waker: &Waker) -> Result<(), String> {
        self.ensure_fresh()?;
        self.inner.register_sender_waker(waker);
        Ok(())
    }

    pub fn register_receiver_waker(&self, waker: &Waker) -> Result<(), String> {
        self.ensure_fresh()?;
        self.inner.register_receiver_waker(waker);
        Ok(())
    }

    fn notify_senders(&self) {
        self.inner.notify_sender_waiters();
    }

    fn notify_receivers(&self) {
        self.inner.notify_receiver_waiters();
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

    /// Send a value to the channel (non-blocking) and report detailed status.
    pub fn send_with_status(&self, value: AsyncValue) -> ChannelSendStatus {
        if let Err(err) = self.ensure_fresh() {
            if err.contains("Stale channel handle") {
                return ChannelSendStatus::Closed;
            }
            return ChannelSendStatus::Error(err);
        }

        let mut state = match self.inner.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return ChannelSendStatus::Error("Channel state poisoned".to_string());
            }
        };

        match &mut *state {
            ChannelState::Open => {
                let mut queue = match self.inner.queue.lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        return ChannelSendStatus::Error("Channel queue poisoned".to_string());
                    }
                };

                if let Some(cap) = self.inner.capacity {
                    if queue.len() >= cap {
                        return ChannelSendStatus::WouldBlock;
                    }
                }

                queue.push_back(value);
                drop(queue);
                self.notify_receivers();
                ChannelSendStatus::Sent
            }
            ChannelState::Closed => ChannelSendStatus::Closed,
            ChannelState::Error(err) => ChannelSendStatus::Error(err.clone()),
        }
    }

    /// Send a value to the channel (blocking semantics).
    pub fn send(&self, value: AsyncValue) -> Result<(), String> {
        match self.send_with_status(value) {
            ChannelSendStatus::Sent => Ok(()),
            ChannelSendStatus::WouldBlock => Err("Channel is full".to_string()),
            ChannelSendStatus::Closed => Err("Channel is closed".to_string()),
            ChannelSendStatus::Error(err) => Err(err),
        }
    }

    /// Try to receive a value from the channel without blocking.
    pub fn try_recv(&self) -> Result<AsyncValue, String> {
        match self.try_recv_with_status() {
            ChannelReceiveStatus::Received(value) => Ok(value),
            ChannelReceiveStatus::WouldBlock => Err("No message available".to_string()),
            ChannelReceiveStatus::Closed => Err("Channel is closed".to_string()),
            ChannelReceiveStatus::Error(err) => Err(err),
        }
    }

    /// Receive a value from the channel without blocking, returning detailed status.
    pub fn try_recv_with_status(&self) -> ChannelReceiveStatus {
        if let Err(err) = self.ensure_fresh() {
            if err.contains("Stale channel handle") {
                return ChannelReceiveStatus::Closed;
            }
            return ChannelReceiveStatus::Error(err);
        }

        let state = match self.inner.state.lock() {
            Ok(state) => state,
            Err(_) => {
                return ChannelReceiveStatus::Error("Channel state poisoned".to_string());
            }
        };

        match &*state {
            ChannelState::Open => {
                let mut queue = match self.inner.queue.lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        return ChannelReceiveStatus::Error("Channel queue poisoned".to_string());
                    }
                };

                let result = match queue.pop_front() {
                    Some(value) => ChannelReceiveStatus::Received(value),
                    None => ChannelReceiveStatus::WouldBlock,
                };
                drop(queue);
                if matches!(result, ChannelReceiveStatus::Received(_)) {
                    self.notify_senders();
                }
                result
            }
            ChannelState::Closed => {
                let mut queue = match self.inner.queue.lock() {
                    Ok(queue) => queue,
                    Err(_) => {
                        return ChannelReceiveStatus::Error("Channel queue poisoned".to_string());
                    }
                };

                let result = match queue.pop_front() {
                    Some(value) => ChannelReceiveStatus::Received(value),
                    None => ChannelReceiveStatus::Closed,
                };
                drop(queue);
                self.notify_senders();
                result
            }
            ChannelState::Error(err) => ChannelReceiveStatus::Error(err.clone()),
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
            drop(state);
            self.notify_senders();
            self.notify_receivers();
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

    /// Create a future that resolves when the value is successfully sent.
    pub fn send_future(&self, value: AsyncValue) -> ChannelSendFuture {
        ChannelSendFuture::new(self.clone(), value)
    }

    /// Create a future that resolves with the next value received on the channel.
    pub fn receive_future(&self) -> ChannelReceiveFuture {
        ChannelReceiveFuture::new(self.clone())
    }
}

impl PartialEq for Channel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
            && self.expected_generation == other.expected_generation
    }
}

impl Eq for Channel {}

pub struct ChannelSendFuture {
    channel: Channel,
    value: Option<AsyncValue>,
}

impl ChannelSendFuture {
    fn new(channel: Channel, value: AsyncValue) -> Self {
        Self {
            channel,
            value: Some(value),
        }
    }
}

impl Future for ChannelSendFuture {
    type Output = AsyncResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(value) = self.value.clone() else {
            return Poll::Ready(Ok(AsyncValue::Boolean(true)));
        };

        match self.channel.send_with_status(value.clone()) {
            ChannelSendStatus::Sent => {
                self.value = None;
                Poll::Ready(Ok(AsyncValue::Boolean(true)))
            }
            ChannelSendStatus::WouldBlock => {
                if let Err(err) = self.channel.register_sender_waker(cx.waker()) {
                    Poll::Ready(Err(AsyncError::ChannelError {
                        reason: err,
                        position: Position::new(0, 0, 0),
                    }))
                } else {
                    self.value = Some(value);
                    Poll::Pending
                }
            }
            ChannelSendStatus::Closed => Poll::Ready(Err(AsyncError::ChannelError {
                reason: "Channel is closed".to_string(),
                position: Position::new(0, 0, 0),
            })),
            ChannelSendStatus::Error(err) => Poll::Ready(Err(AsyncError::ChannelError {
                reason: err,
                position: Position::new(0, 0, 0),
            })),
        }
    }
}

pub struct ChannelReceiveFuture {
    channel: Channel,
}

impl ChannelReceiveFuture {
    fn new(channel: Channel) -> Self {
        Self { channel }
    }
}

impl Future for ChannelReceiveFuture {
    type Output = AsyncResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.channel.try_recv_with_status() {
            ChannelReceiveStatus::Received(value) => Poll::Ready(Ok(value)),
            ChannelReceiveStatus::WouldBlock => {
                if let Err(err) = self.channel.register_receiver_waker(cx.waker()) {
                    Poll::Ready(Err(AsyncError::ChannelError {
                        reason: err,
                        position: Position::new(0, 0, 0),
                    }))
                } else {
                    Poll::Pending
                }
            }
            ChannelReceiveStatus::Closed => Poll::Ready(Err(AsyncError::ChannelError {
                reason: "Channel is closed".to_string(),
                position: Position::new(0, 0, 0),
            })),
            ChannelReceiveStatus::Error(err) => Poll::Ready(Err(AsyncError::ChannelError {
                reason: err,
                position: Position::new(0, 0, 0),
            })),
        }
    }
}

/// Select case describing either a receive or send operation on a channel.
#[derive(Debug, Clone)]
pub enum ChannelSelectCase {
    /// Wait for the next value on a channel.
    Receive { channel: Channel },
    /// Attempt to send a value through a channel.
    Send { channel: Channel, value: AsyncValue },
}

/// Outcome produced when a channel select finishes.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelSelectOutcome {
    /// A receive case completed with a value.
    Received {
        case_index: usize,
        value: AsyncValue,
    },
    /// A send case successfully transmitted its payload.
    Sent { case_index: usize },
    /// The matched channel was already closed.
    Closed { case_index: usize },
    /// All cases are closed; nothing further can make progress.
    AllClosed,
    /// The optional timeout elapsed before a case became ready.
    Timeout,
}

#[derive(Debug, Clone)]
enum ChannelSelectKind {
    Receive,
    Send { value: AsyncValue },
}

#[derive(Debug, Clone)]
struct ChannelSelectCaseState {
    channel: Channel,
    kind: ChannelSelectKind,
}

impl ChannelSelectCaseState {
    fn new(case: ChannelSelectCase) -> Self {
        match case {
            ChannelSelectCase::Receive { channel } => Self {
                channel,
                kind: ChannelSelectKind::Receive,
            },
            ChannelSelectCase::Send { channel, value } => Self {
                channel,
                kind: ChannelSelectKind::Send { value },
            },
        }
    }

    fn poll_case(&self, index: usize, cx: &mut Context<'_>) -> CasePoll {
        match &self.kind {
            ChannelSelectKind::Receive => match self.channel.try_recv_with_status() {
                ChannelReceiveStatus::Received(value) => {
                    CasePoll::Ready(Ok(ChannelSelectOutcome::Received {
                        case_index: index,
                        value,
                    }))
                }
                ChannelReceiveStatus::WouldBlock => {
                    match self.channel.register_receiver_waker(cx.waker()) {
                        Ok(()) => CasePoll::Pending,
                        Err(err) => CasePoll::Ready(Err(AsyncError::ChannelError {
                            reason: err,
                            position: Position::new(0, 0, 0),
                        })),
                    }
                }
                ChannelReceiveStatus::Closed => CasePoll::Closed,
                ChannelReceiveStatus::Error(err) => {
                    CasePoll::Ready(Err(AsyncError::ChannelError {
                        reason: err,
                        position: Position::new(0, 0, 0),
                    }))
                }
            },
            ChannelSelectKind::Send { value } => match self.channel.send_with_status(value.clone())
            {
                ChannelSendStatus::Sent => {
                    CasePoll::Ready(Ok(ChannelSelectOutcome::Sent { case_index: index }))
                }
                ChannelSendStatus::WouldBlock => {
                    match self.channel.register_sender_waker(cx.waker()) {
                        Ok(()) => CasePoll::Pending,
                        Err(err) => CasePoll::Ready(Err(AsyncError::ChannelError {
                            reason: err,
                            position: Position::new(0, 0, 0),
                        })),
                    }
                }
                ChannelSendStatus::Closed => {
                    CasePoll::Ready(Ok(ChannelSelectOutcome::Closed { case_index: index }))
                }
                ChannelSendStatus::Error(err) => CasePoll::Ready(Err(AsyncError::ChannelError {
                    reason: err,
                    position: Position::new(0, 0, 0),
                })),
            },
        }
    }
}

#[derive(Debug)]
enum CasePoll {
    Ready(Result<ChannelSelectOutcome, AsyncError>),
    Pending,
    Closed,
}

#[derive(Debug)]
struct ChannelSelectTimeout {
    duration: Duration,
    start: Instant,
    armed: bool,
}

impl ChannelSelectTimeout {
    fn new(duration: Duration) -> Self {
        Self {
            duration,
            start: Instant::now(),
            armed: false,
        }
    }

    fn expired(&self) -> bool {
        self.start.elapsed() >= self.duration
    }

    fn remaining(&self) -> Duration {
        let elapsed = self.start.elapsed();
        if elapsed >= self.duration {
            Duration::from_secs(0)
        } else {
            self.duration - elapsed
        }
    }

    fn reset(&mut self) {
        self.start = Instant::now();
        self.armed = false;
    }

    fn arm(&mut self, waker: &Waker) {
        if self.armed || self.expired() {
            return;
        }

        let delay = self.remaining();
        let waker = waker.clone();
        self.armed = true;

        thread::spawn(move || {
            thread::sleep(delay);
            waker.wake();
        });
    }
}

/// Future that resolves when one of the channel select cases completes.
#[derive(Debug)]
pub struct ChannelSelectFuture {
    cases: Vec<ChannelSelectCaseState>,
    start_index: usize,
    timeout: Option<ChannelSelectTimeout>,
}

impl ChannelSelectFuture {
    /// Create a new select future from the provided cases.
    pub fn new(cases: Vec<ChannelSelectCase>, timeout: Option<Duration>) -> Self {
        let states = cases.into_iter().map(ChannelSelectCaseState::new).collect();
        Self {
            cases: states,
            start_index: 0,
            timeout: timeout.map(ChannelSelectTimeout::new),
        }
    }

    fn advance_start(&mut self) {
        if !self.cases.is_empty() {
            self.start_index = (self.start_index + 1) % self.cases.len();
        }
    }

    fn reset_timeout(&mut self) {
        if let Some(timeout) = &mut self.timeout {
            timeout.reset();
        }
    }
}

impl Future for ChannelSelectFuture {
    type Output = Result<ChannelSelectOutcome, AsyncError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.cases.is_empty() {
            return Poll::Ready(Ok(ChannelSelectOutcome::AllClosed));
        }

        let len = self.cases.len();
        let mut pending_cases = false;
        let mut closed_cases = 0usize;

        for offset in 0..len {
            let idx = (self.start_index + offset) % len;
            match self.cases[idx].poll_case(idx, cx) {
                CasePoll::Ready(result) => {
                    self.reset_timeout();
                    self.start_index = (idx + 1) % len;
                    return Poll::Ready(result);
                }
                CasePoll::Pending => {
                    pending_cases = true;
                }
                CasePoll::Closed => {
                    closed_cases += 1;
                }
            }
        }

        if closed_cases == len {
            self.reset_timeout();
            return Poll::Ready(Ok(ChannelSelectOutcome::AllClosed));
        }

        if let Some(timeout) = &mut self.timeout {
            if timeout.expired() {
                timeout.reset();
                return Poll::Ready(Ok(ChannelSelectOutcome::Timeout));
            }
            timeout.arm(cx.waker());
        }

        if pending_cases {
            self.advance_start();
            Poll::Pending
        } else {
            // Should not reach here frequently, but ensures progress if all cases closed except one.
            self.reset_timeout();
            Poll::Ready(Ok(ChannelSelectOutcome::AllClosed))
        }
    }
}

/// Helper to construct a select future from cases and optional timeout.
pub fn channel_select_future(
    cases: Vec<ChannelSelectCase>,
    timeout: Option<Duration>,
) -> ChannelSelectFuture {
    ChannelSelectFuture::new(cases, timeout)
}

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
    use crate::async_runtime::{AsyncExecutionContext, AsyncFunction, AsyncRuntime};
    use std::collections::VecDeque;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::thread;
    use std::time::Duration;

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
        assert!(matches!(result, Err(err) if err.contains("closed")));
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

    #[test]
    fn channel_send_reports_back_pressure() {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, Some(1));

        let first = channel.send_with_status(AsyncValue::Integer(1));
        assert_eq!(first, ChannelSendStatus::Sent);

        let second = channel.send_with_status(AsyncValue::Integer(2));
        assert_eq!(second, ChannelSendStatus::WouldBlock);
    }

    #[test]
    fn channel_receive_reports_closed() {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, Some(1));

        // Close channel without draining to simulate shutdown.
        channel.close();
        let status = channel.try_recv_with_status();
        assert!(
            matches!(
                status,
                ChannelReceiveStatus::Closed | ChannelReceiveStatus::Error(_)
            ),
            "expected closed or error status, got {:?}",
            status
        );
    }

    #[test]
    fn channel_send_future_completes_when_capacity_frees() {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, Some(1));

        assert_eq!(
            channel.send_with_status(AsyncValue::Integer(1)),
            ChannelSendStatus::Sent
        );

        let mut future = channel.send_future(AsyncValue::Integer(2));
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        assert!(matches!(
            Pin::new(&mut future).poll(&mut context),
            Poll::Pending
        ));

        let receiver_channel = channel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            let _ = receiver_channel.try_recv_with_status();
        })
        .join()
        .unwrap();

        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(Ok(AsyncValue::Boolean(result))) => assert!(result),
            other => panic!("expected ready boolean result, got {:?}", other),
        }
    }

    #[test]
    fn channel_receive_future_completes_when_value_available() {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, Some(1));

        let mut future = channel.receive_future();
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        assert!(matches!(
            Pin::new(&mut future).poll(&mut context),
            Poll::Pending
        ));

        let sender_channel = channel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            let _ = sender_channel.send_with_status(AsyncValue::Integer(99));
        })
        .join()
        .unwrap();

        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(Ok(AsyncValue::Integer(value))) => assert_eq!(value, 99),
            other => panic!("expected ready integer value, got {:?}", other),
        }
    }

    #[derive(Debug, Clone)]
    struct PipelineProducer {
        channel: Channel,
        value: AsyncValue,
    }

    impl AsyncFunction for PipelineProducer {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let channel = self.channel.clone();
            let value = self.value.clone();
            Box::pin(async move {
                match channel.send_future(value).await {
                    Ok(_) => Ok(AsyncValue::Unit),
                    Err(err) => Err(err),
                }
            })
        }

        fn name(&self) -> &str {
            "pipeline-producer"
        }
    }

    #[derive(Debug, Clone)]
    struct PipelineForwarder {
        input: Channel,
        output: Channel,
    }

    impl AsyncFunction for PipelineForwarder {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let input = self.input.clone();
            let output = self.output.clone();
            Box::pin(async move {
                let value = match input.receive_future().await {
                    Ok(value) => value,
                    Err(err) => return Err(err),
                };
                match output.send_future(value.clone()).await {
                    Ok(_) => Ok(AsyncValue::Unit),
                    Err(err) => Err(err),
                }
            })
        }

        fn name(&self) -> &str {
            "pipeline-forwarder"
        }
    }

    #[derive(Debug, Clone)]
    struct PipelineConsumer {
        input: Channel,
        collected: Arc<Mutex<Vec<AsyncValue>>>,
    }

    impl AsyncFunction for PipelineConsumer {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let input = self.input.clone();
            let collected = Arc::clone(&self.collected);
            Box::pin(async move {
                let value = match input.receive_future().await {
                    Ok(value) => value,
                    Err(err) => return Err(err),
                };

                if let Ok(mut guard) = collected.lock() {
                    guard.push(value.clone());
                }

                Ok(value)
            })
        }

        fn name(&self) -> &str {
            "pipeline-consumer"
        }
    }

    #[test]
    fn channel_values_flow_through_multiple_stages() {
        let mut runtime = AsyncRuntime::new();

        let stage1 = Channel::new(ChannelId::allocate(), Some(1));
        let stage2 = Channel::new(ChannelId::allocate(), Some(1));
        let collected = Arc::new(Mutex::new(Vec::new()));

        let producer_handle = runtime.spawn_task(
            Box::new(PipelineProducer {
                channel: stage1.clone(),
                value: AsyncValue::Integer(17),
            }),
            TaskPriority::Normal,
        );
        let forward_handle = runtime.spawn_task(
            Box::new(PipelineForwarder {
                input: stage1,
                output: stage2.clone(),
            }),
            TaskPriority::Normal,
        );
        let consumer_handle = runtime.spawn_task(
            Box::new(PipelineConsumer {
                input: stage2,
                collected: Arc::clone(&collected),
            }),
            TaskPriority::Normal,
        );

        let producer_id = producer_handle
            .task_id()
            .expect("producer task should have an id");
        let forward_id = forward_handle
            .task_id()
            .expect("forwarder task should have an id");
        let consumer_id = consumer_handle
            .task_id()
            .expect("consumer task should have an id");

        runtime
            .run_until_complete()
            .expect("pipeline tasks should complete");

        assert_eq!(
            runtime
                .wait_for_task(producer_id)
                .expect("producer result available"),
            AsyncValue::Unit
        );
        assert_eq!(
            runtime
                .wait_for_task(forward_id)
                .expect("forwarder result available"),
            AsyncValue::Unit
        );
        assert_eq!(
            runtime
                .wait_for_task(consumer_id)
                .expect("consumer result available"),
            AsyncValue::Integer(17)
        );

        let guard = collected.lock().expect("collected buffer poisoned");
        assert_eq!(
            guard.as_slice(),
            [AsyncValue::Integer(17)],
            "expected consumer to record forwarded value"
        );
    }

    #[derive(Debug)]
    struct SelectOutcomeFunction {
        cases: Vec<ChannelSelectCase>,
    }

    impl AsyncFunction for SelectOutcomeFunction {
        fn execute(
            &self,
            _context: &mut AsyncExecutionContext,
        ) -> Pin<Box<dyn Future<Output = AsyncResult> + Send>> {
            let cases = self.cases.clone();
            Box::pin(async move {
                match channel_select_future(cases, None).await? {
                    ChannelSelectOutcome::Received { case_index, value } => {
                        Ok(AsyncValue::Array(vec![
                            AsyncValue::Integer(case_index as i64),
                            value,
                            AsyncValue::String("received".to_string()),
                        ]))
                    }
                    ChannelSelectOutcome::AllClosed => Ok(AsyncValue::Array(vec![
                        AsyncValue::Integer(-1),
                        AsyncValue::Unit,
                        AsyncValue::String("closed".to_string()),
                    ])),
                    other => Ok(AsyncValue::Array(vec![
                        AsyncValue::Integer(-1),
                        AsyncValue::Unit,
                        AsyncValue::String(format!("{:?}", other)),
                    ])),
                }
            })
        }

        fn name(&self) -> &str {
            "select-outcome-test"
        }
    }

    #[test]
    fn channel_select_future_detects_first_ready_case() {
        let mut runtime = AsyncRuntime::new();
        let primary = Channel::new(ChannelId::allocate(), Some(1));
        let secondary = Channel::new(ChannelId::allocate(), Some(1));

        let select_cases = vec![
            ChannelSelectCase::Receive {
                channel: primary.clone(),
            },
            ChannelSelectCase::Receive {
                channel: secondary.clone(),
            },
        ];

        let handle = runtime.spawn_task(
            Box::new(SelectOutcomeFunction {
                cases: select_cases,
            }),
            TaskPriority::Normal,
        );
        let task_id = handle
            .task_id()
            .expect("select task should have a valid identifier");

        let sender = primary.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let _ = sender.send_with_status(AsyncValue::Integer(77));
        });

        let result = runtime
            .wait_for_task(task_id)
            .expect("select task should complete");

        match result {
            AsyncValue::Array(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0], AsyncValue::Integer(0));
                assert_eq!(values[1], AsyncValue::Integer(77));
                assert_eq!(values[2], AsyncValue::String("received".to_string()));
            }
            other => panic!("unexpected select result: {:?}", other),
        }
    }

    fn noop_waker() -> Waker {
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
    }
}
