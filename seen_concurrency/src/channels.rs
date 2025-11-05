//! Channel management primitives for Seen concurrency runtime.
//!
//! This module coordinates channel registration, lookups, and select bookkeeping
//! for higher-level runtimes. Channel data structures live in `crate::types`
//! where they already expose futures-aware send/receive helpers. The manager
//! keeps track of active channels via generational identifiers and hands out
//! refreshed handles when queried.

use crate::types::{
    AsyncError, AsyncValue, Channel, ChannelId, ChannelReceiveStatus, ChannelSendStatus, TaskId,
};
use seen_lexer::position::Position;
use seen_parser::ast::Type;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Stored channel entry keyed by slot identifier.
#[derive(Debug, Clone)]
struct ChannelEntry {
    handle: Channel,
}

impl ChannelEntry {
    fn new(handle: Channel) -> Self {
        Self { handle }
    }

    fn refreshed(&self) -> Channel {
        self.handle.with_refreshed_generation()
    }
}

/// Manager for all channels visible to the runtime.
#[derive(Debug, Default)]
pub struct ChannelManager {
    channels: HashMap<u32, ChannelEntry>,
    select_operations: HashMap<SelectId, SelectOperation>,
    next_select_id: u64,
}

impl ChannelManager {
    /// Create a new channel manager.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            select_operations: HashMap::new(),
            next_select_id: 1,
        }
    }

    /// Register an externally created channel handle with the manager.
    pub fn register_channel(&mut self, channel: Channel) -> ChannelId {
        let id = channel.id();
        self.channels
            .insert(id.slot(), ChannelEntry::new(channel.clone()));
        id
    }

    /// Allocate a fresh channel with optional capacity and register it.
    pub fn create_channel(&mut self, capacity: Option<usize>) -> Channel {
        let id = ChannelId::allocate();
        let channel = Channel::new(id, capacity);
        self.register_channel(channel.clone());
        channel
    }

    /// Fetch a channel handle by identifier if it exists.
    pub fn get_channel(&self, id: ChannelId) -> Option<Channel> {
        self.channels.get(&id.slot()).map(ChannelEntry::refreshed)
    }

    /// Close and remove a channel from the manager.
    pub fn close_channel(&mut self, id: ChannelId) -> Result<(), AsyncError> {
        if let Some(entry) = self.channels.remove(&id.slot()) {
            entry.handle.close();
            Ok(())
        } else {
            Err(AsyncError::ChannelError {
                reason: "Channel not found".to_string(),
                position: Position::new(0, 0, 0),
            })
        }
    }

    /// Create a select operation that can watch multiple channels.
    pub fn create_select(
        &mut self,
        operations: Vec<SelectCase>,
        timeout: Option<Duration>,
    ) -> SelectId {
        let select_id = SelectId::new(self.next_select_id);
        self.next_select_id += 1;

        let operation = SelectOperation {
            id: select_id,
            operations,
            timeout,
            waiting_task: None,
            created_at: Instant::now(),
        };

        self.select_operations.insert(select_id, operation);
        select_id
    }

    /// Try to execute a recorded select operation. The current implementation
    /// is non-blocking and reports readiness or pending status.
    pub fn execute_select(&mut self, select_id: SelectId) -> Result<SelectResult, AsyncError> {
        let select_op = self
            .select_operations
            .get(&select_id)
            .ok_or_else(|| AsyncError::ChannelError {
                reason: "Select operation not found".to_string(),
                position: Position::new(0, 0, 0),
            })?;

        if let Some(timeout) = select_op.timeout {
            if Instant::now().duration_since(select_op.created_at) >= timeout {
                return Ok(SelectResult::Timeout);
            }
        }

        for case in &select_op.operations {
            match case {
                SelectCase::Receive {
                    channel_id,
                    pattern,
                } => {
                    let Some(channel) = self.get_channel(*channel_id) else {
                        continue;
                    };
                    match channel.try_recv_with_status() {
                        ChannelReceiveStatus::Received(value) => {
                            return Ok(SelectResult::Received {
                                channel_id: *channel_id,
                                value,
                                pattern: pattern.clone(),
                            });
                        }
                        ChannelReceiveStatus::Closed => {
                            continue;
                        }
                        ChannelReceiveStatus::Error(err) => {
                            return Ok(SelectResult::Error(err));
                        }
                        ChannelReceiveStatus::WouldBlock => {
                            continue;
                        }
                    }
                }
                SelectCase::Send { channel_id, value } => {
                    let Some(channel) = self.get_channel(*channel_id) else {
                        continue;
                    };
                    match channel.send_with_status(value.clone()) {
                        ChannelSendStatus::Sent => {
                            return Ok(SelectResult::Sent {
                                channel_id: *channel_id,
                            });
                        }
                        ChannelSendStatus::Closed => continue,
                        ChannelSendStatus::Error(err) => {
                            return Ok(SelectResult::Error(err));
                        }
                        ChannelSendStatus::WouldBlock => continue,
                    }
                }
                SelectCase::Timeout { duration } => {
                    if duration.is_zero() {
                        return Ok(SelectResult::Timeout);
                    }
                }
            }
        }

        Ok(SelectResult::WouldBlock)
    }

    /// Remove a select operation from the manager.
    pub fn remove_select(&mut self, select_id: SelectId) {
        self.select_operations.remove(&select_id);
    }

    /// Current number of registered channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

/// Select operation metadata stored by the manager.
#[derive(Debug, Clone)]
pub struct SelectOperation {
    pub id: SelectId,
    pub operations: Vec<SelectCase>,
    pub timeout: Option<Duration>,
    pub waiting_task: Option<TaskId>,
    pub created_at: Instant,
}

/// Unique identifier for select operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectId(u64);

impl SelectId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Individual case inside a select statement.
#[derive(Debug, Clone)]
pub enum SelectCase {
    Receive {
        channel_id: ChannelId,
        pattern: String,
    },
    Send {
        channel_id: ChannelId,
        value: AsyncValue,
    },
    Timeout {
        duration: Duration,
    },
}

/// Result of a select operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectResult {
    Received {
        channel_id: ChannelId,
        value: AsyncValue,
        pattern: String,
    },
    Sent {
        channel_id: ChannelId,
    },
    Timeout,
    WouldBlock,
    Error(String),
}

/// Helper function used by the interpreter to instantiate channels from type
/// information. The manager registration is left to the caller because it lives
/// behind runtime state.
pub fn create_channel_from_type(
    _channel_type: &Type,
    capacity: Option<usize>,
) -> Result<(AsyncValue, AsyncValue), AsyncError> {
    let id = ChannelId::allocate();
    let channel = Channel::new(id, capacity);
    let sender = channel.clone();
    Ok((AsyncValue::Channel(sender), AsyncValue::Channel(channel)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AsyncValue;

    #[test]
    fn register_and_lookup_channel() {
        let mut manager = ChannelManager::new();
        let channel = manager.create_channel(Some(1));
        assert_eq!(manager.channel_count(), 1);

        let id = channel.id();
        let fetched = manager.get_channel(id).expect("channel should exist");
        assert_eq!(fetched.id().slot(), id.slot());
    }

    #[test]
    fn send_and_receive_through_manager_handle() {
        let mut manager = ChannelManager::new();
        let channel = manager.create_channel(Some(1));
        let id = channel.id();

        assert_eq!(
            channel.send_with_status(AsyncValue::Integer(7)),
            ChannelSendStatus::Sent
        );

        let fetched = manager.get_channel(id).expect("channel present");
        match fetched.try_recv_with_status() {
            ChannelReceiveStatus::Received(value) => {
                assert_eq!(value, AsyncValue::Integer(7));
            }
            other => panic!("expected received value, got {:?}", other),
        }
    }

    #[test]
    fn closing_channel_removes_registration() {
        let mut manager = ChannelManager::new();
        let channel = manager.create_channel(None);
        let id = channel.id();

        manager.close_channel(id).expect("close succeeds");
        assert!(manager.get_channel(id).is_none());
    }
}
