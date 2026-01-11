//! Minimal runtime support for channel operations used by the Seen LLVM backend.
//! This crate exposes a C-compatible surface (`seen_channel_*` symbols) that the
//! compiler links against when lowering concurrent Seen programs.

use std::cell::RefCell;
use std::collections::{VecDeque, HashMap};
use std::ptr;
use std::slice;
use std::sync::{
    atomic::{AtomicI32, AtomicI64, AtomicBool, Ordering}, Arc, Condvar,
    Mutex, Once,
};
use std::time::{Duration, Instant};
use std::fs;
use std::io::{Read, Write, stdout, stderr};
use std::path::Path;

// =============================================================================
// RUNTIME STACK TRACE AND DEBUG INFRASTRUCTURE
// =============================================================================

/// Maximum depth of the call stack trace
const MAX_STACK_DEPTH: usize = 256;
/// Maximum length of a single function name
const MAX_FUNC_NAME_LEN: usize = 128;

/// Thread-local call stack for runtime debugging
thread_local! {
    static CALL_STACK: RefCell<Vec<String>> = RefCell::new(Vec::with_capacity(64));
    static LAST_SEEN_VALUES: RefCell<VecDeque<String>> = RefCell::new(VecDeque::with_capacity(32));
}

/// Global flag indicating if debug mode is enabled
static DEBUG_MODE_ENABLED: AtomicBool = AtomicBool::new(false);
static SIGNAL_HANDLERS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Initialize runtime debugging. Called once at program start if --runtime-debug is enabled.
#[unsafe(no_mangle)]
pub extern "C" fn __seen_debug_init() {
    DEBUG_MODE_ENABLED.store(true, Ordering::SeqCst);
    install_signal_handlers();
    eprintln!("[SEEN DEBUG] Runtime debugging initialized");
}

/// Push a function onto the call stack trace
#[unsafe(no_mangle)]
pub extern "C" fn __seen_push_frame(func_name: *const i8) {
    if !DEBUG_MODE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    let name = if func_name.is_null() {
        "<unknown>".to_string()
    } else {
        unsafe {
            let c_str = std::ffi::CStr::from_ptr(func_name);
            c_str.to_string_lossy().into_owned()
        }
    };
    
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.len() < MAX_STACK_DEPTH {
            eprintln!("[SEEN TRACE] -> {}", name);
            stack.push(name);
        }
    });
}

/// Pop a function from the call stack trace
#[unsafe(no_mangle)]
pub extern "C" fn __seen_pop_frame() {
    if !DEBUG_MODE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    CALL_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(name) = stack.pop() {
            eprintln!("[SEEN TRACE] <- {}", name);
        }
    });
}

/// Record a debug value (for crash diagnostics)
#[unsafe(no_mangle)]
pub extern "C" fn __seen_debug_value(label: *const i8, value: i64) {
    if !DEBUG_MODE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    let label_str = if label.is_null() {
        "?"
    } else {
        unsafe {
            std::ffi::CStr::from_ptr(label).to_str().unwrap_or("?")
        }
    };
    
    let entry = format!("{} = {}", label_str, value);
    eprintln!("[SEEN DEBUG] {}", entry);
    
    LAST_SEEN_VALUES.with(|vals| {
        let mut vals = vals.borrow_mut();
        if vals.len() >= 32 {
            vals.pop_front();
        }
        vals.push_back(entry);
    });
}

/// Record a debug string value
#[unsafe(no_mangle)]
pub extern "C" fn __seen_debug_string(label: *const i8, value: *const SeenString) {
    if !DEBUG_MODE_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    let label_str = if label.is_null() {
        "?"
    } else {
        unsafe {
            std::ffi::CStr::from_ptr(label).to_str().unwrap_or("?")
        }
    };
    
    let value_str = if value.is_null() {
        "<null>".to_string()
    } else {
        unsafe { (*value).to_str().to_string() }
    };
    
    let entry = format!("{} = \"{}\"", label_str, value_str);
    eprintln!("[SEEN DEBUG] {}", entry);
    
    LAST_SEEN_VALUES.with(|vals| {
        let mut vals = vals.borrow_mut();
        if vals.len() >= 32 {
            vals.pop_front();
        }
        vals.push_back(entry);
    });
}

/// Print the current call stack (for debugging or crash reports)
#[unsafe(no_mangle)]
pub extern "C" fn __seen_print_stack_trace() {
    CALL_STACK.with(|stack| {
        let stack = stack.borrow();
        if stack.is_empty() {
            eprintln!("[SEEN STACK] <empty stack>");
            return;
        }
        eprintln!("\n╔═══════════════════════════════════════════════════════════════╗");
        eprintln!("║                     SEEN STACK TRACE                          ║");
        eprintln!("╠═══════════════════════════════════════════════════════════════╣");
        for (i, frame) in stack.iter().rev().enumerate() {
            eprintln!("║ {:>3}: {:<57} ║", i, truncate_str(frame, 57));
        }
        eprintln!("╚═══════════════════════════════════════════════════════════════╝");
    });
    
    // Also print last seen debug values
    LAST_SEEN_VALUES.with(|vals| {
        let vals = vals.borrow();
        if !vals.is_empty() {
            eprintln!("\n[SEEN DEBUG] Last {} debug values:", vals.len());
            for val in vals.iter() {
                eprintln!("  {}", val);
            }
        }
    });
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Install signal handlers for SIGSEGV, SIGABRT, etc.
fn install_signal_handlers() {
    if SIGNAL_HANDLERS_INSTALLED.swap(true, Ordering::SeqCst) {
        return; // Already installed
    }
    
    #[cfg(unix)]
    {
        use std::ffi::c_int;
        
        extern "C" fn signal_handler(sig: c_int) {
            let sig_name = match sig {
                11 => "SIGSEGV (Segmentation fault)",
                6 => "SIGABRT (Aborted)",
                4 => "SIGILL (Illegal instruction)",
                8 => "SIGFPE (Floating point exception)",
                7 => "SIGBUS (Bus error)",
                _ => "Unknown signal",
            };
            
            eprintln!("\n");
            eprintln!("╔═══════════════════════════════════════════════════════════════╗");
            eprintln!("║                    SEEN RUNTIME CRASH                         ║");
            eprintln!("╠═══════════════════════════════════════════════════════════════╣");
            eprintln!("║ Signal: {:<53} ║", sig_name);
            eprintln!("╚═══════════════════════════════════════════════════════════════╝");
            
            // Print stack trace
            __seen_print_stack_trace();
            
            eprintln!("\n[SEEN DEBUG] Tip: Run with `--debug` to get more information.");
            eprintln!("[SEEN DEBUG] Tip: Use GDB/LLDB with the compiled binary for native debugging.");
            
            // Re-raise the signal with default handler
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
                libc::raise(sig);
            }
        }
        
        unsafe {
            // Install handlers for common crash signals
            libc::signal(libc::SIGSEGV, signal_handler as usize);
            libc::signal(libc::SIGABRT, signal_handler as usize);
            libc::signal(libc::SIGBUS, signal_handler as usize);
            libc::signal(libc::SIGFPE, signal_handler as usize);
            libc::signal(libc::SIGILL, signal_handler as usize);
        }
        
        eprintln!("[SEEN DEBUG] Signal handlers installed (SIGSEGV, SIGABRT, SIGBUS, SIGFPE, SIGILL)");
    }
    
    #[cfg(not(unix))]
    {
        eprintln!("[SEEN DEBUG] Signal handlers not available on this platform");
    }
}

const STATUS_RECEIVED: i64 = 0;
const STATUS_ALL_CLOSED: i64 = 3;

const TAG_INT: i32 = 0;
const TAG_BOOL: i32 = 1;
const TAG_PTR: i32 = 2;

/// SeenString layout: { len: i64, data: *const u8 }
/// This MUST match the LLVM backend's ty_string() which is { i64, ptr }
/// Field 0 = len (i64), Field 1 = data (ptr)
#[repr(C)]
pub struct SeenString {
    pub len: i64,
    pub data: *const u8,
}

// Compile-time ABI verification
const _: () = {
    // Verify SeenString is exactly 16 bytes (i64 + ptr on 64-bit)
    assert!(std::mem::size_of::<SeenString>() == 16, "SeenString must be 16 bytes");
    // Verify field offsets: len at 0, data at 8
    assert!(std::mem::offset_of!(SeenString, len) == 0, "SeenString.len must be at offset 0");
    assert!(std::mem::offset_of!(SeenString, data) == 8, "SeenString.data must be at offset 8");
};

/// Runtime ABI verification function - callable from Seen code to verify struct layout
/// Returns 1 if ABI matches expected layout, 0 otherwise
#[unsafe(no_mangle)]
pub extern "C" fn __VerifyStringABI(test_string: SeenString) -> i64 {
    // Verify that the string struct we received has sensible values
    // If ABI is mismatched, len/data would be swapped or corrupted
    if test_string.len < 0 || test_string.len > 1_000_000_000 {
        eprintln!("ABI MISMATCH: SeenString.len has invalid value: {}", test_string.len);
        return 0;
    }
    if test_string.len > 0 && test_string.data.is_null() {
        eprintln!("ABI MISMATCH: SeenString.data is null but len > 0");
        return 0;
    }
    // Return the struct size so caller can verify
    std::mem::size_of::<SeenString>() as i64
}

/// Returns the expected field offsets as a packed i64: (len_offset << 32) | data_offset
#[unsafe(no_mangle)]
pub extern "C" fn __GetStringABIInfo() -> i64 {
    let len_offset = std::mem::offset_of!(SeenString, len) as i64;
    let data_offset = std::mem::offset_of!(SeenString, data) as i64;
    (len_offset << 32) | data_offset
}

impl SeenString {
    fn to_str(&self) -> &str {
        if self.data.is_null() || self.len == 0 {
            return "";
        }
        unsafe {
            let slice = slice::from_raw_parts(self.data, self.len as usize);
            std::str::from_utf8(slice).unwrap_or("")
        }
    }
}

const EMPTY_BYTES: [u8; 0] = [];

fn empty_seen_string() -> SeenString {
    SeenString {
        len: 0,
        data: EMPTY_BYTES.as_ptr(),
    }
}

fn leak_seen_string(text: String) -> SeenString {
    if text.is_empty() {
        return empty_seen_string();
    }
    let mut bytes = text.into_bytes();
    let len = bytes.len() as i64;
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    SeenString { len, data: ptr }
}

// Global file descriptor map
static mut FILE_MAP: Option<Mutex<HashMap<i64, fs::File>>> = None;
static FILE_MAP_ONCE: Once = Once::new();
static NEXT_FD: AtomicI64 = AtomicI64::new(100);
static mut FILE_ERROR_MAP: Option<Mutex<HashMap<i64, String>>> = None;
static FILE_ERROR_ONCE: Once = Once::new();

fn get_file_map() -> &'static Mutex<HashMap<i64, fs::File>> {
    unsafe {
        FILE_MAP_ONCE.call_once(|| {
            FILE_MAP = Some(Mutex::new(HashMap::new()));
        });
        let ptr = std::ptr::addr_of!(FILE_MAP);
        (*ptr).as_ref().unwrap()
    }
}

fn get_file_error_map() -> &'static Mutex<HashMap<i64, String>> {
    unsafe {
        FILE_ERROR_ONCE.call_once(|| {
            FILE_ERROR_MAP = Some(Mutex::new(HashMap::new()));
        });
        let ptr = std::ptr::addr_of!(FILE_ERROR_MAP);
        (*ptr).as_ref().unwrap()
    }
}

fn clear_file_error(fd: i64) {
    get_file_error_map().lock().unwrap().insert(fd, String::new());
}

fn set_file_error(fd: i64, msg: impl Into<String>) {
    get_file_error_map().lock().unwrap().insert(fd, msg.into());
}

fn take_file_error(fd: i64) -> SeenString {
    let msg = get_file_error_map()
        .lock()
        .unwrap()
        .get(&fd)
        .cloned()
        .unwrap_or_default();
    leak_seen_string(msg)
}

#[repr(C)]
pub struct SeenArray {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

#[repr(C)]
pub struct CommandResult {
    pub success: bool,
    pub output: SeenString,
}

#[unsafe(no_mangle)]
pub extern "C" fn __ExecuteCommand(result: *mut CommandResult, command: *const SeenString) {
    let cmd_str = if command.is_null() {
        ""
    } else {
        let cmd = unsafe { &*command };
        cmd.to_str()
    };

    let (success, output_str) = match std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            let combined = if stderr.is_empty() { stdout } else { format!("{}\n{}", stdout, stderr) };
            (out.status.success(), combined)
        }
        Err(e) => (false, format!("Error: {}", e)),
    };

    let output = leak_seen_string(output_str);
    
    unsafe {
        *result = CommandResult { success, output };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __CommandOutput(command: SeenString) -> SeenString {
    let cmd_str = command.to_str();
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .output()
    {
        Ok(output) => leak_seen_string(String::from_utf8_lossy(&output.stdout).into_owned()),
        Err(_) => empty_seen_string(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __GetCommandLineArgsHelper(argc: i32, argv: *const *const u8) -> *mut SeenArray {
    let mut args = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        let c_str = unsafe { std::ffi::CStr::from_ptr(*argv.add(i as usize) as *const i8) };
        let s = c_str.to_string_lossy().into_owned();
        let len = s.len() as i64;
        let c_string = std::ffi::CString::new(s).unwrap();
        let ptr = c_string.into_raw() as *const u8;
        args.push(SeenString { len, data: ptr });
    }
    
    // Leak the vector content to keep strings alive
    let args_ptr = args.as_mut_ptr();
    std::mem::forget(args); // Don't drop the vector, but we need to construct a new array for Seen
    
    // Construct SeenArray on heap
    let array = Box::new(SeenArray {
        len: argc as i64,
        cap: argc as i64,
        data: args_ptr as *mut u8,
    });
    Box::into_raw(array)
}

#[unsafe(no_mangle)]
pub extern "C" fn __ReadFileFromPath(path: SeenString) -> SeenString {
    let path_str = path.to_str();
    match fs::read_to_string(path_str) {
        Ok(content) => leak_seen_string(content),
        Err(_) => empty_seen_string(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __OpenFile(path: SeenString, mode: SeenString) -> i64 {
    eprintln!("DEBUG: __OpenFile path.len={} path.data={:?}", path.len, path.data);
    eprintln!("DEBUG: __OpenFile mode.len={} mode.data={:?}", mode.len, mode.data);
    let path_str = path.to_str();
    let mode_str = mode.to_str();
    eprintln!("DEBUG: __OpenFile path='{}' mode='{}'", path_str, mode_str);
    if let Ok(cwd) = std::env::current_dir() {
        eprintln!("DEBUG: CWD={:?}", cwd);
    }

    // Fast-path standard streams without going through the FILE_MAP
    match path_str {
        "/dev/stdout" => return 1,
        "/dev/stderr" => return 2,
        "/dev/stdin" => return 0,
        _ => {}
    }
    
    let file_res = match mode_str {
        "r" => fs::File::open(path_str),
        "w" => fs::File::create(path_str),
        "a" => fs::OpenOptions::new().append(true).create(true).open(path_str),
        _ => {
            eprintln!("DEBUG: __OpenFile unknown mode '{}'", mode_str);
            return -1;
        }
    };
    eprintln!("DEBUG: __OpenFile file_res={:?}", file_res);
    
    match file_res {
        Ok(file) => {
            let fd = NEXT_FD.fetch_add(1, Ordering::SeqCst);
            get_file_map().lock().unwrap().insert(fd, file);
            clear_file_error(fd);
            fd
        }
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __CloseFile(fd: i64) -> i64 {
    // Do not attempt to close standard streams
    if fd >= 0 && fd < 3 {
        return 0;
    }

    if get_file_map().lock().unwrap().remove(&fd).is_some() {
        clear_file_error(fd);
        0
    } else {
        set_file_error(fd, "invalid fd");
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __WriteFile(fd: i64, content: SeenString) -> i64 {
    // Handle standard streams directly
    if fd == 1 || fd == 2 {
        let bytes = unsafe { slice::from_raw_parts(content.data, content.len as usize) };
        let io_res = if fd == 1 {
            stdout().write_all(bytes)
        } else {
            stderr().write_all(bytes)
        };
        return match io_res {
            Ok(_) => {
                clear_file_error(fd);
                content.len
            }
            Err(e) => {
                set_file_error(fd, format!("write failed: {}", e));
                -1
            }
        };
    }

    let mut map = get_file_map().lock().unwrap();
    if let Some(file) = map.get_mut(&fd) {
        let bytes = unsafe { slice::from_raw_parts(content.data, content.len as usize) };
        match file.write_all(bytes) {
            Ok(_) => {
                clear_file_error(fd);
                content.len
            }
            Err(e) => {
                set_file_error(fd, format!("write failed: {}", e));
                -1
            }
        }
    } else {
        set_file_error(fd, "invalid fd");
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __WriteFileToPath(path: SeenString, content: SeenString) -> i64 {
    if path.data.is_null() {
        return 0;
    }
    
    let path_str = path.to_str();
    
    if content.data.is_null() && content.len > 0 {
        return 0;
    }
    
    let bytes = unsafe { slice::from_raw_parts(content.data, content.len as usize) };
    match fs::write(path_str, bytes) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __ReadFile(fd: i64) -> SeenString {
    let mut map = get_file_map().lock().unwrap();
    if let Some(file) = map.get_mut(&fd) {
        let mut buffer = String::new();
        match file.read_to_string(&mut buffer) {
            Ok(_) => {
                clear_file_error(fd);
                leak_seen_string(buffer)
            }
            Err(e) => {
                set_file_error(fd, format!("read failed: {}", e));
                empty_seen_string()
            }
        }
    } else {
        set_file_error(fd, "invalid fd");
        empty_seen_string()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __ReadFile_SRET(result: *mut SeenString, fd: i64) {
    let s = __ReadFile(fd);
    unsafe { *result = s; }
}

#[unsafe(no_mangle)]
pub extern "C" fn __FileExists(path: SeenString) -> bool {
    Path::new(path.to_str()).exists()
}

#[unsafe(no_mangle)]
pub extern "C" fn __DeleteFile(path: SeenString) -> bool {
    fs::remove_file(path.to_str()).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn __FileSize(fd: i64) -> i64 {
    let mut map = get_file_map().lock().unwrap();
    if let Some(file) = map.get_mut(&fd) {
        match file.metadata() {
            Ok(m) => {
                clear_file_error(fd);
                m.len() as i64
            }
            Err(e) => {
                set_file_error(fd, format!("stat failed: {}", e));
                -1
            }
        }
    } else {
        set_file_error(fd, "invalid fd");
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __FileError(_fd: i64) -> SeenString {
    take_file_error(_fd)
}

/// SRET version of __FileError for proper ABI handling
#[unsafe(no_mangle)]
pub extern "C" fn __FileError_SRET(result: *mut SeenString, fd: i64) {
    let s = take_file_error(fd);
    unsafe { *result = s; }
}

#[unsafe(no_mangle)]
pub extern "C" fn __WriteFileBytes(fd: i64, bytes: SeenArray) -> i64 {
    if bytes.len < 0 {
        set_file_error(fd, "negative length");
        return -1;
    }

    let mut map = get_file_map().lock().unwrap();
    let file = match map.get_mut(&fd) {
        Some(f) => f,
        None => {
            set_file_error(fd, "invalid fd");
            return -1;
        }
    };

    if bytes.len == 0 {
        clear_file_error(fd);
        return 0;
    }

    if bytes.data.is_null() {
        set_file_error(fd, "bytes data is null");
        return -1;
    }

    let slice_len = bytes.len as usize;
    let ints = unsafe { slice::from_raw_parts(bytes.data as *const i64, slice_len) };
    let mut buf = Vec::with_capacity(slice_len);
    for &v in ints {
        if v < 0 || v > 255 {
            set_file_error(fd, "byte out of range");
            return -1;
        }
        buf.push(v as u8);
    }

    match file.write_all(&buf) {
        Ok(_) => {
            clear_file_error(fd);
            bytes.len
        }
        Err(e) => {
            set_file_error(fd, format!("write failed: {}", e));
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __ReadFileBytes(fd: i64, size: i64) -> SeenArray {
    if size <= 0 {
        clear_file_error(fd);
        return SeenArray { len: 0, cap: 0, data: ptr::null_mut() };
    }

    let mut map = get_file_map().lock().unwrap();
    let file = match map.get_mut(&fd) {
        Some(f) => f,
        None => {
            set_file_error(fd, "invalid fd");
            return SeenArray { len: 0, cap: 0, data: ptr::null_mut() };
        }
    };

    let mut reader = file.take(size as u64);
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize);
    match reader.read_to_end(&mut buf) {
        Ok(_) => {
            clear_file_error(fd);
        }
        Err(e) => {
            set_file_error(fd, format!("read failed: {}", e));
            return SeenArray { len: 0, cap: 0, data: ptr::null_mut() };
        }
    }

    let mut ints: Vec<i64> = buf.into_iter().map(|b| b as i64).collect();
    let len = ints.len() as i64;
    let ptr = if len == 0 {
        ptr::null_mut()
    } else {
        let raw = ints.as_mut_ptr();
        std::mem::forget(ints);
        raw as *mut u8
    };

    SeenArray { len, cap: len, data: ptr }
}

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
// Array Runtime (for generic array allocation and management)
// ============================================================================

/// Allocate a new array with specified element size and capacity
/// Returns pointer to Array<T> struct on heap
#[unsafe(no_mangle)]
pub extern "C" fn __ArrayNew(element_size: i64, capacity: i64) -> *mut SeenArray {
    if element_size <= 0 || capacity < 0 {
        return ptr::null_mut();
    }

    let byte_capacity = (element_size * capacity) as usize;
    let data_ptr = if byte_capacity > 0 {
        unsafe {
            let layout = std::alloc::Layout::from_size_align_unchecked(byte_capacity, 8);
            let ptr = std::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                return ptr::null_mut();
            }
            ptr
        }
    } else {
        ptr::null_mut()
    };

    Box::into_raw(Box::new(SeenArray {
        len: 0,
        cap: capacity,
        data: data_ptr,
    }))
}

/// Allocate array with initial length set to capacity (filled with zeros)
#[unsafe(no_mangle)]
pub extern "C" fn __ArrayWithLength(element_size: i64, length: i64) -> *mut SeenArray {
    let arr_ptr = __ArrayNew(element_size, length);
    if !arr_ptr.is_null() {
        unsafe {
            (*arr_ptr).len = length;
        }
    }
    arr_ptr
}

/// Free an array and its data
#[unsafe(no_mangle)]
pub extern "C" fn __ArrayFree(arr_ptr: *mut SeenArray, element_size: i64) {
    if arr_ptr.is_null() {
        return;
    }

    unsafe {
        let arr = Box::from_raw(arr_ptr);
        if !arr.data.is_null() && arr.cap > 0 && element_size > 0 {
            let byte_capacity = (element_size * arr.cap) as usize;
            let layout = std::alloc::Layout::from_size_align_unchecked(byte_capacity, 8);
            std::alloc::dealloc(arr.data, layout);
        }
    }
}

/// Push element to end of array (grows if needed)
/// Returns 0 on success, -1 on failure
#[unsafe(no_mangle)]
pub extern "C" fn __ArrayPush(arr_ptr: *mut SeenArray, element_ptr: *const u8, element_size: i64) -> i32 {
    if arr_ptr.is_null() || element_ptr.is_null() || element_size <= 0 {
        return -1;
    }

    unsafe {
        let arr = &mut *arr_ptr;
        
        // Grow if needed
        if arr.len >= arr.cap {
            let new_capacity = if arr.cap == 0 { 8 } else { arr.cap * 2 };
            let new_byte_capacity = (element_size * new_capacity) as usize;
            let new_layout = std::alloc::Layout::from_size_align_unchecked(new_byte_capacity, 8);
            let new_data = std::alloc::alloc_zeroed(new_layout);
            
            if new_data.is_null() {
                return -1;
            }

            // Copy old data
            if !arr.data.is_null() && arr.len > 0 {
                let old_byte_size = (element_size * arr.len) as usize;
                std::ptr::copy_nonoverlapping(arr.data, new_data, old_byte_size);
                
                // Free old data
                if arr.cap > 0 {
                    let old_byte_capacity = (element_size * arr.cap) as usize;
                    let old_layout = std::alloc::Layout::from_size_align_unchecked(old_byte_capacity, 8);
                    std::alloc::dealloc(arr.data, old_layout);
                }
            }

            arr.data = new_data;
            arr.cap = new_capacity;
        }

        // Copy element to end
        let offset = (arr.len * element_size) as isize;
        let dest = arr.data.offset(offset);
        std::ptr::copy_nonoverlapping(element_ptr, dest, element_size as usize);
        arr.len += 1;
        
        0
    }
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
pub extern "C" fn __IntToString(value: i64) -> SeenString {
    let s = value.to_string();
    string_to_seen_string(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn __FloatToString(value: f64) -> SeenString {
    let s = value.to_string();
    string_to_seen_string(s)
}

#[unsafe(no_mangle)]
#[unsafe(no_mangle)]
pub extern "C" fn __BoolToString(value: i64) -> SeenString {
    let s = if value != 0 { "true" } else { "false" }.to_string();
    string_to_seen_string(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn __CharToString(value: i64) -> SeenString {
    eprintln!("DEBUG: __CharToString value={}", value);
    let ch = std::char::from_u32(value as u32).unwrap_or('\u{FFFD}');
    string_to_seen_string(ch.to_string())
}

fn string_to_seen_string(s: String) -> SeenString {
    let len = s.len() as i64;
    let c_str = std::ffi::CString::new(s).unwrap();
    let ptr = c_str.into_raw() as *const u8;
    SeenString { len, data: ptr }
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
