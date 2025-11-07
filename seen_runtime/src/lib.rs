//! Minimal runtime support for channel operations used by the Seen LLVM backend.
//! This crate exposes a C-compatible surface (`seen_channel_*` symbols) that the
//! compiler links against when lowering concurrent Seen programs.

use std::collections::VecDeque;
use std::ptr;
use std::slice;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const STATUS_RECEIVED: i64 = 0;
const STATUS_ALL_CLOSED: i64 = 3;

#[repr(C)]
pub struct SeenSelectResult {
    pub payload: *mut u8,
    pub index: i64,
    pub status: i64,
}

#[repr(C)]
struct SeenChannel {
    channel: RuntimeChannel,
}

#[derive(Clone)]
struct RuntimeChannel {
    inner: Arc<ChannelInner>,
}

struct ChannelInner {
    queue: Mutex<VecDeque<*mut u8>>,
    capacity: Option<usize>,
    not_empty: Condvar,
    not_full: Condvar,
    closed: Mutex<bool>,
}

impl RuntimeChannel {
    fn new(capacity: Option<usize>) -> Self {
        Self {
            inner: Arc::new(ChannelInner {
                queue: Mutex::new(VecDeque::new()),
                capacity,
                not_empty: Condvar::new(),
                not_full: Condvar::new(),
                closed: Mutex::new(false),
            }),
        }
    }

    fn send(&self, payload: *mut u8) {
        let mut queue = self.inner.queue.lock().expect("channel queue poisoned");
        while self
            .inner
            .capacity
            .map(|cap| queue.len() >= cap)
            .unwrap_or(false)
        {
            queue = self
                .inner
                .not_full
                .wait(queue)
                .expect("channel wait (full) poisoned");
        }
        queue.push_back(payload);
        self.inner.not_empty.notify_one();
    }

    fn recv(&self) -> Option<*mut u8> {
        let mut queue = self.inner.queue.lock().expect("channel queue poisoned");
        loop {
            if let Some(value) = queue.pop_front() {
                self.inner.not_full.notify_one();
                return Some(value);
            }
            if *self.inner.closed.lock().expect("closed flag poisoned") {
                return None;
            }
            queue = self
                .inner
                .not_empty
                .wait(queue)
                .expect("channel wait (empty) poisoned");
        }
    }

    fn try_recv(&self) -> Option<*mut u8> {
        let mut queue = self.inner.queue.lock().expect("channel queue poisoned");
        let value = queue.pop_front();
        if value.is_some() {
            self.inner.not_full.notify_one();
        }
        value
    }

    fn close(&self) {
        if let Ok(mut guard) = self.inner.closed.lock() {
            *guard = true;
        }
        self.inner.not_empty.notify_all();
        self.inner.not_full.notify_all();
    }
}

fn capacity_from_i64(value: i64) -> Option<usize> {
    if value < 0 {
        None
    } else {
        Some(value as usize)
    }
}

fn channel_from_raw<'a>(ptr: *mut u8) -> Option<&'a SeenChannel> {
    if ptr.is_null() {
        None
    } else {
        unsafe { (ptr as *mut SeenChannel).as_ref() }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_channel_new(capacity: i64) -> *mut u8 {
    let cap = capacity_from_i64(capacity);
    let handle = Box::new(SeenChannel {
        channel: RuntimeChannel::new(cap),
    });
    Box::into_raw(handle) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_channel_send(channel: *mut u8, payload: *mut u8) -> *mut u8 {
    if let Some(handle) = channel_from_raw(channel) {
        handle.channel.send(payload);
    }
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_channel_recv(channel: *mut u8) -> *mut u8 {
    channel_from_raw(channel)
        .and_then(|handle| handle.channel.recv())
        .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_channel_close(channel: *mut u8) {
    if let Some(handle) = channel_from_raw(channel) {
        handle.channel.close();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_channel_select(
    cases_ptr: *const *mut u8,
    out_ptr: *mut SeenSelectResult,
    count: u64,
) -> *mut u8 {
    if cases_ptr.is_null() || out_ptr.is_null() || count == 0 {
        return ptr::null_mut();
    }

    let cases =
        unsafe { slice::from_raw_parts(cases_ptr as *const *mut SeenChannel, count as usize) };
    let start = Instant::now();

    loop {
        for (idx, handle_ptr) in cases.iter().enumerate() {
            let handle = unsafe { (*handle_ptr).as_ref() };
            let Some(channel) = handle else {
                continue;
            };
            if let Some(payload) = channel.channel.try_recv() {
                unsafe {
                    (*out_ptr).payload = payload;
                    (*out_ptr).index = idx as i64;
                    (*out_ptr).status = STATUS_RECEIVED;
                }
                return out_ptr as *mut u8;
            }
        }

        if start.elapsed() > Duration::from_secs(1) {
            unsafe {
                (*out_ptr).payload = ptr::null_mut();
                (*out_ptr).index = -1;
                (*out_ptr).status = STATUS_ALL_CLOSED;
            }
            return out_ptr as *mut u8;
        }

        std::thread::sleep(Duration::from_micros(200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_and_recv_round_trips_pointer() {
        let channel = seen_channel_new(0);
        let payload = Box::into_raw(Box::new(42u64)) as *mut u8;
        seen_channel_send(channel, payload);
        let received = seen_channel_recv(channel);
        assert_eq!(received, payload);
        unsafe {
            drop(Box::from_raw(received as *mut u64));
            drop(Box::from_raw(channel as *mut SeenChannel));
        }
    }

    #[test]
    fn select_reports_index_of_ready_channel() {
        let ch_a = seen_channel_new(0) as *mut SeenChannel;
        let ch_b = seen_channel_new(0) as *mut SeenChannel;
        let payload = Box::into_raw(Box::new(7u64)) as *mut u8;
        seen_channel_send(ch_b as *mut u8, payload);
        let cases = vec![ch_a, ch_b];
        let mut result = SeenSelectResult {
            payload: ptr::null_mut(),
            index: -1,
            status: -1,
        };
        let out = seen_channel_select(cases.as_ptr() as *const *mut u8, &mut result, 2);
        assert!(!out.is_null());
        assert_eq!(result.index, 1);
        assert_eq!(result.status, STATUS_RECEIVED);
        assert_eq!(result.payload, payload);

        unsafe {
            drop(Box::from_raw(payload as *mut u64));
            drop(Box::from_raw(ch_a));
            drop(Box::from_raw(ch_b));
        }
    }
}
