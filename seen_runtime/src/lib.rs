//! Minimal runtime support for channel operations used by the Seen LLVM backend.
//! This crate exposes a C-compatible surface (`seen_channel_*` symbols) that the
//! compiler links against when lowering concurrent Seen programs.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ptr;
use std::slice;
use std::sync::{
    atomic::{AtomicI32, Ordering}, Arc, Condvar,
    Mutex,
};
use std::time::{Duration, Instant};

const STATUS_RECEIVED: i64 = 0;
const STATUS_ALL_CLOSED: i64 = 3;

const TAG_INT: i32 = 0;
const TAG_BOOL: i32 = 1;
const TAG_PTR: i32 = 2;

#[repr(C)]
pub struct SeenBoxedValue {
    tag: i32,
    _pad: i32,
    int_payload: i64,
    ptr_payload: *mut u8,
}

impl SeenBoxedValue {
    fn boxed_int(value: i64) -> *mut SeenBoxedValue {
        Box::into_raw(Box::new(SeenBoxedValue {
            tag: TAG_INT,
            _pad: 0,
            int_payload: value,
            ptr_payload: ptr::null_mut(),
        }))
    }

    fn boxed_bool(value: bool) -> *mut SeenBoxedValue {
        Box::into_raw(Box::new(SeenBoxedValue {
            tag: TAG_BOOL,
            _pad: 0,
            int_payload: if value { 1 } else { 0 },
            ptr_payload: ptr::null_mut(),
        }))
    }

    fn boxed_ptr(ptr_val: *mut u8) -> *mut SeenBoxedValue {
        Box::into_raw(Box::new(SeenBoxedValue {
            tag: TAG_PTR,
            _pad: 0,
            int_payload: 0,
            ptr_payload: ptr_val,
        }))
    }
}

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

thread_local! {
    static SCOPE_STATE: RefCell<ScopeState> = RefCell::new(ScopeState::default());
}

#[derive(Default)]
struct ScopeState {
    task_depth: i32,
    jobs_depth: i32,
}

impl ScopeState {
    fn push(&mut self, kind: i32) {
        match kind {
            0 => self.task_depth = self.task_depth.saturating_add(1),
            1 => self.jobs_depth = self.jobs_depth.saturating_add(1),
            _ => {}
        }
    }

    fn pop(&mut self, kind: i32) -> i32 {
        match kind {
            0 => {
                if self.task_depth > 0 {
                    self.task_depth -= 1;
                    1
                } else {
                    0
                }
            }
            1 => {
                if self.jobs_depth > 0 {
                    self.jobs_depth -= 1;
                    1
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}

static NEXT_TASK_HANDLE: AtomicI32 = AtomicI32::new(1);

#[repr(C)]
pub struct SeenTaskHandle {
    id: i32,
    kind: i32,
}

fn allocate_task_handle(kind: i32) -> *mut SeenTaskHandle {
    let id = NEXT_TASK_HANDLE.fetch_add(1, Ordering::Relaxed);
    Box::into_raw(Box::new(SeenTaskHandle { id, kind }))
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
    if value <= 0 {
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

#[unsafe(no_mangle)]
pub extern "C" fn seen_box_int(value: i64) -> *mut u8 {
    SeenBoxedValue::boxed_int(value) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_box_bool(value: i32) -> *mut u8 {
    SeenBoxedValue::boxed_bool(value != 0) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_box_ptr(value: *mut u8) -> *mut u8 {
    SeenBoxedValue::boxed_ptr(value) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_unbox_int(value: *mut u8) -> i64 {
    if value.is_null() {
        return 0;
    }
    unsafe {
        let boxed = &*value.cast::<SeenBoxedValue>();
        match boxed.tag {
            TAG_INT => boxed.int_payload,
            TAG_BOOL => boxed.int_payload,
            _ => 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_unbox_bool(value: *mut u8) -> i32 {
    if value.is_null() {
        return 0;
    }
    unsafe {
        let boxed = &*value.cast::<SeenBoxedValue>();
        match boxed.tag {
            TAG_BOOL | TAG_INT => (boxed.int_payload != 0) as i32,
            _ => 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_unbox_ptr(value: *mut u8) -> *mut u8 {
    if value.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let boxed = &*value.cast::<SeenBoxedValue>();
        if boxed.tag == TAG_PTR {
            boxed.ptr_payload
        } else {
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seen_box_free(value: *mut u8) {
    if value.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(value.cast::<SeenBoxedValue>()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __scope_push(kind: i32) {
    SCOPE_STATE.with(|state| state.borrow_mut().push(kind));
}

#[unsafe(no_mangle)]
pub extern "C" fn __scope_pop(kind: i32) -> i32 {
    SCOPE_STATE.with(|state| state.borrow_mut().pop(kind))
}

#[unsafe(no_mangle)]
pub extern "C" fn __task_handle_new(kind: i32) -> *mut SeenTaskHandle {
    allocate_task_handle(kind)
}

#[unsafe(no_mangle)]
pub extern "C" fn __spawn_task() -> *mut SeenTaskHandle {
    allocate_task_handle(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn __spawn_detached() -> *mut SeenTaskHandle {
    allocate_task_handle(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn __spawn_actor() -> *mut SeenTaskHandle {
    allocate_task_handle(2)
}

#[unsafe(no_mangle)]
pub extern "C" fn __await(_handle: *mut SeenTaskHandle) -> i32 {
    1
}

// ============================================================================
// Timing Intrinsics (for benchmarks and performance measurement)
// ============================================================================

/// Get current timestamp in milliseconds since Unix epoch
#[unsafe(no_mangle)]
pub extern "C" fn __GetTimestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Get current timestamp in nanoseconds since Unix epoch
#[unsafe(no_mangle)]
pub extern "C" fn __GetTimestampNanos() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64
}

/// Sleep for specified milliseconds
#[unsafe(no_mangle)]
pub extern "C" fn __Sleep(ms: i64) {
    if ms > 0 {
        std::thread::sleep(Duration::from_millis(ms as u64));
    }
}

// ============================================================================
// Math Intrinsics (for scientific computing and benchmarks)
// ============================================================================

/// Square root
#[unsafe(no_mangle)]
pub extern "C" fn __Sqrt(x: f64) -> f64 {
    x.sqrt()
}

/// Sine function
#[unsafe(no_mangle)]
pub extern "C" fn __Sin(x: f64) -> f64 {
    x.sin()
}

/// Cosine function
#[unsafe(no_mangle)]
pub extern "C" fn __Cos(x: f64) -> f64 {
    x.cos()
}

/// Power function (x^y)
#[unsafe(no_mangle)]
pub extern "C" fn __Pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

/// Absolute value
#[unsafe(no_mangle)]
pub extern "C" fn __Abs(x: f64) -> f64 {
    x.abs()
}

/// Floor function
#[unsafe(no_mangle)]
pub extern "C" fn __Floor(x: f64) -> f64 {
    x.floor()
}

/// Ceiling function
#[unsafe(no_mangle)]
pub extern "C" fn __Ceil(x: f64) -> f64 {
    x.ceil()
}

// ============================================================================
// I/O Intrinsics (for output and file operations)
// ============================================================================

/// Print a C-string to stdout
#[unsafe(no_mangle)]
pub extern "C" fn __Print(msg: *const u8) {
    if msg.is_null() {
        return;
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(msg as *const i8);
        if let Ok(s) = c_str.to_str() {
            print!("{}", s);
        }
    }
}

/// Print a C-string to stdout with newline
#[unsafe(no_mangle)]
pub extern "C" fn __Println(msg: *const u8) {
    if msg.is_null() {
        println!();
        return;
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(msg as *const i8);
        if let Ok(s) = c_str.to_str() {
            println!("{}", s);
        }
    }
}

// ============================================================================
// String Conversion Intrinsics (for B3 - String operations)
// ============================================================================

/// Convert Int to String (returns heap-allocated null-terminated string)
/// Caller must free the returned pointer with __FreeString
#[unsafe(no_mangle)]
pub extern "C" fn __IntToString(value: i64) -> *mut u8 {
    use std::ffi::CString;
    let s = value.to_string();
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw() as *mut u8,
        Err(_) => ptr::null_mut(),
    }
}

/// Convert Float to String (returns heap-allocated null-terminated string)
/// Caller must free the returned pointer with __FreeString
#[unsafe(no_mangle)]
pub extern "C" fn __FloatToString(value: f64) -> *mut u8 {
    use std::ffi::CString;
    let s = value.to_string();
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw() as *mut u8,
        Err(_) => ptr::null_mut(),
    }
}

/// Convert Bool to String (returns heap-allocated null-terminated string)
/// Caller must free the returned pointer with __FreeString
#[unsafe(no_mangle)]
pub extern "C" fn __BoolToString(value: i32) -> *mut u8 {
    use std::ffi::CString;
    let s = if value != 0 { "true" } else { "false" };
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw() as *mut u8,
        Err(_) => ptr::null_mut(),
    }
}

/// Parse String to Int (returns 0 on error)
#[unsafe(no_mangle)]
pub extern "C" fn __StringToInt(s: *const u8) -> i64 {
    if s.is_null() {
        return 0;
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(s as *const i8);
        if let Ok(rust_str) = c_str.to_str() {
            rust_str.trim().parse::<i64>().unwrap_or(0)
        } else {
            0
        }
    }
}

/// Parse String to Float (returns 0.0 on error)
#[unsafe(no_mangle)]
pub extern "C" fn __StringToFloat(s: *const u8) -> f64 {
    if s.is_null() {
        return 0.0;
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(s as *const i8);
        if let Ok(rust_str) = c_str.to_str() {
            rust_str.trim().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        }
    }
}

/// Free a string returned by __IntToString, __FloatToString, or __BoolToString
#[unsafe(no_mangle)]
pub extern "C" fn __FreeString(s: *mut u8) {
    if s.is_null() {
        return;
    }
    unsafe {
        use std::ffi::CString;
        drop(CString::from_raw(s as *mut i8));
    }
}

/// Get string length (number of bytes in UTF-8, excluding null terminator)
#[unsafe(no_mangle)]
pub extern "C" fn __StringLength(s: *const u8) -> i64 {
    if s.is_null() {
        return 0;
    }

    unsafe {
        let c_str = std::ffi::CStr::from_ptr(s as *const i8);
        c_str.to_bytes().len() as i64
    }
}

/// Concatenate two strings (returns heap-allocated null-terminated string)
/// Caller must free the returned pointer with __FreeString
#[unsafe(no_mangle)]
pub extern "C" fn __StringConcat(a: *const u8, b: *const u8) -> *mut u8 {
    use std::ffi::CString;

    if a.is_null() && b.is_null() {
        return match CString::new("") {
            Ok(c_str) => c_str.into_raw() as *mut u8,
            Err(_) => ptr::null_mut(),
        };
    }

    unsafe {
        let str_a = if a.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(a as *const i8).to_str().unwrap_or("")
        };

        let str_b = if b.is_null() {
            ""
        } else {
            std::ffi::CStr::from_ptr(b as *const i8).to_str().unwrap_or("")
        };

        let result = format!("{}{}", str_a, str_b);
        match CString::new(result) {
            Ok(c_str) => c_str.into_raw() as *mut u8,
            Err(_) => ptr::null_mut(),
        }
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
    #[test]
    fn boxed_values_round_trip() {
        let boxed_int = seen_box_int(42);
        assert_eq!(seen_unbox_int(boxed_int), 42);
        seen_box_free(boxed_int);

        let boxed_bool = seen_box_bool(1);
        assert_eq!(seen_unbox_bool(boxed_bool), 1);
        seen_box_free(boxed_bool);

        let payload = Box::into_raw(Box::new(77u64)) as *mut u8;
        let boxed_ptr = seen_box_ptr(payload);
        assert_eq!(seen_unbox_ptr(boxed_ptr), payload);
        seen_box_free(boxed_ptr);
        unsafe {
            drop(Box::from_raw(payload as *mut u64));
        }
    }

    #[test]
    fn scope_push_pop_tracks_depth() {
        __scope_push(0);
        assert_eq!(__scope_pop(0), 1);
        assert_eq!(__scope_pop(0), 0);
    }

    #[test]
    fn spawn_handles_are_unique() {
        let a = __spawn_task();
        let b = __spawn_task();
        assert_ne!(a, b);
        unsafe {
            drop(Box::from_raw(a as *mut SeenTaskHandle));
            drop(Box::from_raw(b as *mut SeenTaskHandle));
        }
    }
}
