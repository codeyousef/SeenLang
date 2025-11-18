//! Channel management primitives for Seen concurrency runtime.
//!
//! This module coordinates channel registration, lookups, and select bookkeeping
//! for higher-level runtimes. Channel data structures live in `crate::types`
//! where they already expose futures-aware send/receive helpers. The manager
//! keeps track of active channels via generational identifiers and hands out
//! refreshed handles when queried.

use crate::types::{
    channel_select_future, ChannelSelectCase, ChannelSelectFuture, ChannelSelectOutcome,
};
use crate::types::{AsyncError, AsyncValue, Channel, ChannelId, TaskId};
use seen_lexer::position::Position;
use seen_parser::ast::Type;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
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

    /// Build a future that resolves when any of the select operations fires.
    pub fn select_future(
        &self,
        operations: &[SelectCase],
        timeout: Option<Duration>,
    ) -> Result<ManagerSelectFuture, AsyncError> {
        let mut cases = Vec::with_capacity(operations.len());
        let mut metadata = Vec::with_capacity(operations.len());
        let mut timeout_acc: Option<Duration> = timeout;

        for op in operations {
            match op {
                SelectCase::Receive {
                    channel_id,
                    pattern,
                } => {
                    let channel =
                        self.get_channel(*channel_id)
                            .ok_or_else(|| AsyncError::ChannelError {
                                reason: format!("Channel {:?} not found", channel_id.id()),
                                position: Position::new(0, 0, 0),
                            })?;
                    cases.push(ChannelSelectCase::Receive {
                        channel: channel.clone(),
                    });
                    metadata.push(SelectCaseMeta {
                        channel_id: *channel_id,
                        pattern: Some(pattern.clone()),
                    });
                }
                SelectCase::Send { channel_id, value } => {
                    let channel =
                        self.get_channel(*channel_id)
                            .ok_or_else(|| AsyncError::ChannelError {
                                reason: format!("Channel {:?} not found", channel_id.id()),
                                position: Position::new(0, 0, 0),
                            })?;
                    cases.push(ChannelSelectCase::Send {
                        channel: channel.clone(),
                        value: value.clone(),
                    });
                    metadata.push(SelectCaseMeta {
                        channel_id: *channel_id,
                        pattern: None,
                    });
                }
                SelectCase::Timeout { duration } => {
                    timeout_acc = Some(match timeout_acc {
                        Some(existing) => existing.min(*duration),
                        None => *duration,
                    });
                }
            }
        }

        let inner = channel_select_future(cases, timeout_acc);
        Ok(ManagerSelectFuture::new(inner, metadata))
    }

    /// Try to execute a recorded select operation. The current implementation
    /// is non-blocking and reports readiness or pending status.
    pub fn execute_select(&mut self, select_id: SelectId) -> Result<SelectResult, AsyncError> {
        let select_op =
            self.select_operations
                .get(&select_id)
                .ok_or_else(|| AsyncError::ChannelError {
                    reason: "Select operation not found".to_string(),
                    position: Position::new(0, 0, 0),
                })?;

        let future = self.select_future(&select_op.operations, select_op.timeout)?;
        let mut future = Box::pin(future);
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => result,
            Poll::Pending => Ok(SelectResult::WouldBlock),
        }
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

#[derive(Debug, Clone)]
struct SelectCaseMeta {
    channel_id: ChannelId,
    pattern: Option<String>,
}

/// Future wrapper that maps raw channel select outcomes back to manager-level results.
pub struct ManagerSelectFuture {
    inner: ChannelSelectFuture,
    metadata: Vec<SelectCaseMeta>,
}

impl ManagerSelectFuture {
    fn new(inner: ChannelSelectFuture, metadata: Vec<SelectCaseMeta>) -> Self {
        Self { inner, metadata }
    }

    fn map_outcome(&self, outcome: ChannelSelectOutcome) -> SelectResult {
        match outcome {
            ChannelSelectOutcome::Received { case_index, value } => {
                if let Some(meta) = self.metadata.get(case_index) {
                    if let Some(pattern) = &meta.pattern {
                        SelectResult::Received {
                            channel_id: meta.channel_id,
                            value,
                            pattern: pattern.clone(),
                        }
                    } else {
                        SelectResult::Error("Select receive metadata missing pattern".to_string())
                    }
                } else {
                    SelectResult::Error("Select outcome index out of bounds".to_string())
                }
            }
            ChannelSelectOutcome::Sent { case_index } => {
                if let Some(meta) = self.metadata.get(case_index) {
                    SelectResult::Sent {
                        channel_id: meta.channel_id,
                    }
                } else {
                    SelectResult::Error("Select outcome index out of bounds".to_string())
                }
            }
            ChannelSelectOutcome::Closed { .. } | ChannelSelectOutcome::AllClosed => {
                SelectResult::WouldBlock
            }
            ChannelSelectOutcome::Timeout => SelectResult::Timeout,
        }
    }
}

impl Future for ManagerSelectFuture {
    type Output = Result<SelectResult, AsyncError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.inner).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => match result {
                Ok(outcome) => Poll::Ready(Ok(self.map_outcome(outcome))),
                Err(err) => Poll::Ready(Err(err)),
            },
        }
    }
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

fn noop_waker() -> Waker {
    unsafe fn clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &VTABLE)
    }

    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AsyncValue, ChannelReceiveStatus, ChannelSendStatus};
    use std::task::{Context, Poll};

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

    #[test]
    fn select_future_resolves_on_receive() {
        let mut manager = ChannelManager::new();
        let channel = manager.create_channel(Some(1));
        let id = channel.id();

        let cases = [SelectCase::Receive {
            channel_id: id,
            pattern: "val".to_string(),
        }];

        let mut future = Box::pin(manager.select_future(&cases, None).unwrap());
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));

        assert_eq!(
            channel.send_with_status(AsyncValue::Integer(42)),
            ChannelSendStatus::Sent
        );

        match future.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(SelectResult::Received { value, .. })) => {
                assert_eq!(value, AsyncValue::Integer(42));
            }
            other => panic!("expected received result, got {:?}", other),
        }
    }

    #[test]
    fn select_future_handles_blocked_send() {
        let mut manager = ChannelManager::new();
        let channel = manager.create_channel(Some(1));
        let id = channel.id();

        // Fill channel to force send branch to block.
        assert_eq!(
            channel.send_with_status(AsyncValue::Integer(1)),
            ChannelSendStatus::Sent
        );

        let cases = [SelectCase::Send {
            channel_id: id,
            value: AsyncValue::Integer(2),
        }];

        let mut future = Box::pin(manager.select_future(&cases, None).unwrap());
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);

        assert!(matches!(future.as_mut().poll(&mut cx), Poll::Pending));

        let drain = manager.get_channel(id).expect("channel available");
        assert!(matches!(
            drain.try_recv_with_status(),
            ChannelReceiveStatus::Received(_)
        ));

        match future.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(SelectResult::Sent { channel_id })) => {
                assert_eq!(channel_id, id);
            }
            other => panic!("expected send result, got {:?}", other),
        }
    }
}
