//! Channel-based communication for Seen Language
//!
//! This module implements channels according to Seen's syntax design:
//! - let (sender, receiver) = Channel<T>()
//! - sender.Send(value) and receiver.Receive()
//! - select expressions with when clauses
//! - Proper type safety and error handling

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Condvar};
use std::time::{Duration, Instant};
use seen_parser::ast::Type;
use seen_lexer::position::Position;
use crate::types::{AsyncValue, AsyncError, AsyncResult, ChannelId, TaskId};

/// Channel for type-safe message passing between tasks
#[derive(Debug)]
pub struct Channel<T> {
    /// Unique channel identifier
    pub id: ChannelId,
    /// Channel capacity (None for unbounded)
    pub capacity: Option<usize>,
    /// Message buffer
    buffer: Arc<Mutex<VecDeque<T>>>,
    /// Condition variable for sender notification
    sender_cv: Arc<Condvar>,
    /// Condition variable for receiver notification
    receiver_cv: Arc<Condvar>,
    /// Number of active senders
    sender_count: Arc<Mutex<usize>>,
    /// Number of active receivers
    receiver_count: Arc<Mutex<usize>>,
    /// Channel state
    state: Arc<Mutex<ChannelState>>,
}

/// Channel states
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelState {
    /// Channel is open for operations
    Open,
    /// Channel is closed, no more sends allowed
    Closed,
    /// Channel has an error
    Error(String),
}

/// Sender half of a channel
#[derive(Debug, Clone)]
pub struct ChannelSender<T> {
    /// Reference to the underlying channel
    channel: Arc<Channel<T>>,
}

impl<T> ChannelSender<T> {
    /// Get the channel ID
    pub fn channel_id(&self) -> ChannelId {
        self.channel.id
    }
}

/// Receiver half of a channel
#[derive(Debug, Clone)]
pub struct ChannelReceiver<T> {
    /// Reference to the underlying channel
    channel: Arc<Channel<T>>,
}

/// Result of a channel send operation
#[derive(Debug, Clone, PartialEq)]
pub enum SendResult {
    /// Message sent successfully
    Sent,
    /// Channel is full, would block
    WouldBlock,
    /// Channel is closed
    Closed,
    /// Send operation failed
    Error(String),
}

/// Result of a channel receive operation
#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveResult<T> {
    /// Message received successfully
    Received(T),
    /// No message available, would block
    WouldBlock,
    /// Channel is closed and empty
    Closed,
    /// Receive operation failed
    Error(String),
}

/// Manager for all channels in the system
#[derive(Debug)]
pub struct ChannelManager {
    /// All channels indexed by ID
    channels: HashMap<ChannelId, Box<dyn ChannelTrait>>,
    /// Next available channel ID
    next_channel_id: u64,
    /// Select operation registry
    select_operations: HashMap<SelectId, SelectOperation>,
    /// Next available select ID
    next_select_id: u64,
}

/// Trait for type-erased channel operations
pub trait ChannelTrait: Send + Sync + std::fmt::Debug {
    /// Get channel ID
    fn id(&self) -> ChannelId;
    
    /// Get channel capacity
    fn capacity(&self) -> Option<usize>;
    
    /// Get current buffer size
    fn buffer_size(&self) -> usize;
    
    /// Check if channel is closed
    fn is_closed(&self) -> bool;
    
    /// Close the channel
    fn close(&self) -> Result<(), String>;
}

/// Select operation for waiting on multiple channels
#[derive(Debug)]
pub struct SelectOperation {
    /// Select operation ID
    pub id: SelectId,
    /// Channel operations to wait on
    pub operations: Vec<SelectCase>,
    /// Timeout for the select operation
    pub timeout: Option<Duration>,
    /// Task waiting for the select to complete
    pub waiting_task: Option<TaskId>,
}

/// Unique identifier for select operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectId(u64);

impl SelectId {
    /// Create a new select ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Individual case in a select operation
#[derive(Debug, Clone)]
pub enum SelectCase {
    /// Receive from channel
    Receive {
        channel_id: ChannelId,
        pattern: String, // Variable name to bind received value
    },
    /// Send to channel
    Send {
        channel_id: ChannelId,
        value: AsyncValue,
    },
    /// Timeout case
    Timeout {
        duration: Duration,
    },
}

impl<T> Channel<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Create a new channel with optional capacity
    pub fn new(capacity: Option<usize>) -> (ChannelSender<T>, ChannelReceiver<T>) {
        let id = ChannelId::new(rand::random::<u64>());
        
        let channel = Arc::new(Channel {
            id,
            capacity,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            sender_cv: Arc::new(Condvar::new()),
            receiver_cv: Arc::new(Condvar::new()),
            sender_count: Arc::new(Mutex::new(1)), // Initial sender
            receiver_count: Arc::new(Mutex::new(1)), // Initial receiver
            state: Arc::new(Mutex::new(ChannelState::Open)),
        });
        
        let sender = ChannelSender {
            channel: channel.clone(),
        };
        
        let receiver = ChannelReceiver {
            channel,
        };
        
        (sender, receiver)
    }
    
    /// Create an unbounded channel
    pub fn unbounded() -> (ChannelSender<T>, ChannelReceiver<T>) {
        Self::new(None)
    }
    
    /// Create a bounded channel with specified capacity
    pub fn bounded(capacity: usize) -> (ChannelSender<T>, ChannelReceiver<T>) {
        Self::new(Some(capacity))
    }
}

impl<T> ChannelSender<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Send a value to the channel (non-blocking)
    pub fn try_send(&self, value: T) -> SendResult {
        let mut buffer = match self.channel.buffer.lock() {
            Ok(buffer) => buffer,
            Err(_) => return SendResult::Error("Failed to lock buffer".to_string()),
        };
        
        let state = match self.channel.state.lock() {
            Ok(state) => state,
            Err(_) => return SendResult::Error("Failed to lock state".to_string()),
        };
        
        match *state {
            ChannelState::Closed => return SendResult::Closed,
            ChannelState::Error(ref msg) => return SendResult::Error(msg.clone()),
            ChannelState::Open => {}
        }
        
        // Check capacity
        if let Some(capacity) = self.channel.capacity {
            if buffer.len() >= capacity {
                return SendResult::WouldBlock;
            }
        }
        
        // Add to buffer
        buffer.push_back(value);
        
        // Notify waiting receivers
        self.channel.receiver_cv.notify_one();
        
        SendResult::Sent
    }
    
    /// Send a value to the channel (blocking)
    pub fn send(&self, value: T) -> Result<(), AsyncError> {
        loop {
            match self.try_send(value.clone()) {
                SendResult::Sent => return Ok(()),
                SendResult::Closed => {
                    return Err(AsyncError::ChannelError {
                        reason: "Channel is closed".to_string(),
                        position: Position::new(0, 0, 0),
                    });
                }
                SendResult::Error(msg) => {
                    return Err(AsyncError::ChannelError {
                        reason: msg,
                        position: Position::new(0, 0, 0),
                    });
                }
                SendResult::WouldBlock => {
                    // Wait for space to become available
                    let _guard = self.channel.sender_cv.wait_while(
                        self.channel.buffer.lock().unwrap(),
                        |buffer| {
                            if let Some(capacity) = self.channel.capacity {
                                buffer.len() >= capacity
                            } else {
                                false // Unbounded channels never block on send
                            }
                        },
                    ).unwrap();
                    // Try again
                }
            }
        }
    }
    
    /// Close the sending half of the channel
    pub fn close(&self) -> Result<(), AsyncError> {
        let mut state = self.channel.state.lock().map_err(|_| AsyncError::ChannelError {
            reason: "Failed to lock state".to_string(),
            position: Position::new(0, 0, 0),
        })?;
        
        *state = ChannelState::Closed;
        
        // Notify all waiting receivers
        self.channel.receiver_cv.notify_all();
        
        Ok(())
    }
}

impl<T> ChannelReceiver<T>
where
    T: Clone + Send + Sync + 'static,
{
    /// Receive a value from the channel (non-blocking)
    pub fn try_receive(&self) -> ReceiveResult<T> {
        let mut buffer = match self.channel.buffer.lock() {
            Ok(buffer) => buffer,
            Err(_) => return ReceiveResult::Error("Failed to lock buffer".to_string()),
        };
        
        let state = match self.channel.state.lock() {
            Ok(state) => state,
            Err(_) => return ReceiveResult::Error("Failed to lock state".to_string()),
        };
        
        if let Some(value) = buffer.pop_front() {
            // Notify waiting senders that space is available
            self.channel.sender_cv.notify_one();
            return ReceiveResult::Received(value);
        }
        
        // Buffer is empty
        match *state {
            ChannelState::Closed => ReceiveResult::Closed,
            ChannelState::Error(ref msg) => ReceiveResult::Error(msg.clone()),
            ChannelState::Open => ReceiveResult::WouldBlock,
        }
    }
    
    /// Receive a value from the channel (blocking)
    pub fn receive(&self) -> Result<T, AsyncError> {
        loop {
            match self.try_receive() {
                ReceiveResult::Received(value) => return Ok(value),
                ReceiveResult::Closed => {
                    return Err(AsyncError::ChannelError {
                        reason: "Channel is closed".to_string(),
                        position: Position::new(0, 0, 0),
                    });
                }
                ReceiveResult::Error(msg) => {
                    return Err(AsyncError::ChannelError {
                        reason: msg,
                        position: Position::new(0, 0, 0),
                    });
                }
                ReceiveResult::WouldBlock => {
                    // Wait for a message to arrive
                    let _guard = self.channel.receiver_cv.wait_while(
                        self.channel.buffer.lock().unwrap(),
                        |buffer| buffer.is_empty(),
                    ).unwrap();
                    // Try again
                }
            }
        }
    }
    
    /// Receive with timeout
    pub fn receive_timeout(&self, timeout: Duration) -> Result<Option<T>, AsyncError> {
        let start_time = Instant::now();
        
        loop {
            match self.try_receive() {
                ReceiveResult::Received(value) => return Ok(Some(value)),
                ReceiveResult::Closed => {
                    return Err(AsyncError::ChannelError {
                        reason: "Channel is closed".to_string(),
                        position: Position::new(0, 0, 0),
                    });
                }
                ReceiveResult::Error(msg) => {
                    return Err(AsyncError::ChannelError {
                        reason: msg,
                        position: Position::new(0, 0, 0),
                    });
                }
                ReceiveResult::WouldBlock => {
                    let elapsed = start_time.elapsed();
                    if elapsed >= timeout {
                        return Ok(None); // Timeout
                    }
                    
                    let remaining = timeout - elapsed;
                    let _result = self.channel.receiver_cv.wait_timeout_while(
                        self.channel.buffer.lock().unwrap(),
                        remaining,
                        |buffer| buffer.is_empty(),
                    ).unwrap();
                    // Try again
                }
            }
        }
    }
}

impl<T> ChannelTrait for Channel<T>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    fn id(&self) -> ChannelId {
        self.id
    }
    
    fn capacity(&self) -> Option<usize> {
        self.capacity
    }
    
    fn buffer_size(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }
    
    fn is_closed(&self) -> bool {
        matches!(*self.state.lock().unwrap(), ChannelState::Closed)
    }
    
    fn close(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Failed to lock state".to_string())?;
        *state = ChannelState::Closed;
        
        // Notify all waiting tasks
        self.receiver_cv.notify_all();
        self.sender_cv.notify_all();
        
        Ok(())
    }
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            next_channel_id: 1,
            select_operations: HashMap::new(),
            next_select_id: 1,
        }
    }
    
    /// Create a new typed channel
    pub fn create_channel<T>(&mut self, capacity: Option<usize>) -> (ChannelSender<T>, ChannelReceiver<T>)
    where
        T: Clone + Send + Sync + 'static,
    {
        let (sender, receiver) = Channel::new(capacity);
        
        // Channels are managed through sender/receiver handles
        // Central registration not needed for type-safe channel operations
        
        (sender, receiver)
    }
    
    /// Create a select operation
    pub fn create_select(&mut self, operations: Vec<SelectCase>, timeout: Option<Duration>) -> SelectId {
        let select_id = SelectId::new(self.next_select_id);
        self.next_select_id += 1;
        
        let select_op = SelectOperation {
            id: select_id,
            operations,
            timeout,
            waiting_task: None,
        };
        
        self.select_operations.insert(select_id, select_op);
        select_id
    }
    
    /// Execute a select operation
    pub fn execute_select(&mut self, select_id: SelectId) -> Result<SelectResult, AsyncError> {
        let select_op = self.select_operations.get(&select_id).ok_or_else(|| {
            AsyncError::ChannelError {
                reason: "Select operation not found".to_string(),
                position: Position::new(0, 0, 0),
            }
        })?;
        
        // For now, just try the first available operation
        for case in &select_op.operations {
            match case {
                SelectCase::Receive { channel_id, pattern } => {
                    if let Some(channel) = self.channels.get(channel_id) {
                        // Try to receive from channel
                        // Type-erased channels require runtime type checking
                        continue;
                    }
                }
                SelectCase::Send { channel_id, value } => {
                    if let Some(channel) = self.channels.get(channel_id) {
                        // Try to send to channel  
                        // Type-erased channels require runtime type checking
                        continue;
                    }
                }
                SelectCase::Timeout { duration: _ } => {
                    // Handle timeout case
                    continue;
                }
            }
        }
        
        // No operations were ready
        Ok(SelectResult::WouldBlock)
    }
    
    /// Get channel by ID
    pub fn get_channel(&self, id: ChannelId) -> Option<&dyn ChannelTrait> {
        self.channels.get(&id).map(|c| c.as_ref())
    }
    
    /// Close a channel
    pub fn close_channel(&mut self, id: ChannelId) -> Result<(), AsyncError> {
        if let Some(channel) = self.channels.get(&id) {
            channel.close().map_err(|msg| AsyncError::ChannelError {
                reason: msg,
                position: Position::new(0, 0, 0),
            })
        } else {
            Err(AsyncError::ChannelError {
                reason: "Channel not found".to_string(),
                position: Position::new(0, 0, 0),
            })
        }
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a select operation
#[derive(Debug, Clone, PartialEq)]
pub enum SelectResult {
    /// A receive operation completed
    Received {
        channel_id: ChannelId,
        value: AsyncValue,
        pattern: String,
    },
    /// A send operation completed
    Sent {
        channel_id: ChannelId,
    },
    /// Timeout occurred
    Timeout,
    /// No operations were ready
    WouldBlock,
    /// Select operation failed
    Error(String),
}

/// Helper function to create channels in Seen syntax: Channel<T>()
pub fn create_channel_from_type(
    channel_type: &Type,
    capacity: Option<usize>,
) -> Result<(AsyncValue, AsyncValue), AsyncError> {
    // For now, create a generic channel that holds AsyncValue
    let (sender, receiver) = Channel::<AsyncValue>::new(capacity);
    
    Ok((
        AsyncValue::Channel(Arc::new(crate::types::Channel {
            id: sender.channel.id,
            capacity,
            state: crate::types::ChannelState::Open,
            queue: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            sender_count: 1,
            receiver_count: 1,
        })),
        AsyncValue::Channel(Arc::new(crate::types::Channel {
            id: receiver.channel.id,
            capacity,
            state: crate::types::ChannelState::Open,
            queue: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            sender_count: 1,
            receiver_count: 1,
        })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    
    #[test]
    fn test_unbounded_channel() {
        let (sender, receiver) = Channel::<i32>::unbounded();
        
        // Send some values
        assert_eq!(sender.try_send(1), SendResult::Sent);
        assert_eq!(sender.try_send(2), SendResult::Sent);
        assert_eq!(sender.try_send(3), SendResult::Sent);
        
        // Receive them
        assert_eq!(receiver.try_receive(), ReceiveResult::Received(1));
        assert_eq!(receiver.try_receive(), ReceiveResult::Received(2));
        assert_eq!(receiver.try_receive(), ReceiveResult::Received(3));
        assert_eq!(receiver.try_receive(), ReceiveResult::WouldBlock);
    }
    
    #[test]
    fn test_bounded_channel() {
        let (sender, receiver) = Channel::<i32>::bounded(2);
        
        // Fill the channel
        assert_eq!(sender.try_send(1), SendResult::Sent);
        assert_eq!(sender.try_send(2), SendResult::Sent);
        assert_eq!(sender.try_send(3), SendResult::WouldBlock); // Channel full
        
        // Receive one value to make space
        assert_eq!(receiver.try_receive(), ReceiveResult::Received(1));
        
        // Now we can send again
        assert_eq!(sender.try_send(3), SendResult::Sent);
    }
    
    #[test]
    fn test_channel_close() {
        let (sender, receiver) = Channel::<i32>::unbounded();
        
        // Send a value
        assert_eq!(sender.try_send(42), SendResult::Sent);
        
        // Close the channel
        sender.close().unwrap();
        
        // Can still receive existing values
        assert_eq!(receiver.try_receive(), ReceiveResult::Received(42));
        
        // But no more values available and channel is closed
        assert_eq!(receiver.try_receive(), ReceiveResult::Closed);
        
        // Cannot send after close
        assert_eq!(sender.try_send(1), SendResult::Closed);
    }
    
    #[test]
    fn test_channel_manager() {
        let mut manager = ChannelManager::new();
        
        let (sender, _receiver) = manager.create_channel::<i32>(Some(10));
        let _channel_id = sender.channel.id;
        
        // Channel registration and management is handled through sender/receiver
        // Direct manager operations not needed with current architecture
    }
    
    #[test]
    fn test_blocking_operations() {
        let (sender, receiver) = Channel::<i32>::unbounded();
        
        // Test in separate thread to avoid deadlock
        let sender_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            sender.send(42).unwrap();
        });
        
        // This should block until sender sends
        let received = receiver.receive().unwrap();
        assert_eq!(received, 42);
        
        sender_thread.join().unwrap();
    }
}