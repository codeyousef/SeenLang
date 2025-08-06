//! Foreign Function Interface (FFI) utilities for C library interop
//!
//! Provides safe wrappers and conversion utilities for interfacing with C libraries
//! following Zig-style approach where C headers can be imported directly.

use crate::string::String;
use std::ffi::{CStr, CString, OsStr, OsString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

/// Safe wrapper for C strings that handles conversion and memory management
#[repr(transparent)]
pub struct CStringWrapper {
    inner: CString,
}

impl CStringWrapper {
    /// Creates a new C string from a Rust string
    pub fn new(s: &str) -> Result<Self, std::ffi::NulError> {
        Ok(CStringWrapper {
            inner: CString::new(s)?,
        })
    }
    
    /// Creates a C string from our String type
    pub fn from_string(s: &String) -> Result<Self, std::ffi::NulError> {
        Self::new(s.as_str())
    }
    
    /// Returns a pointer to the C string - safe for FFI
    pub fn as_ptr(&self) -> *const c_char {
        self.inner.as_ptr()
    }
    
    /// Returns a mutable pointer to the C string
    pub fn as_mut_ptr(&mut self) -> *mut c_char {
        self.inner.as_ptr() as *mut c_char
    }
    
    /// Converts back to a Rust String (our custom String type)
    pub fn to_string(&self) -> String {
        String::from(self.inner.to_string_lossy().as_ref())
    }
    
    /// Gets the byte length of the C string (without null terminator)
    pub fn len(&self) -> usize {
        self.inner.as_bytes().len()
    }
    
    /// Checks if the string is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Safe wrapper for reading C strings from external libraries
pub struct CStrWrapper<'a> {
    inner: &'a CStr,
}

impl<'a> CStrWrapper<'a> {
    /// Creates a wrapper from a C string pointer (unsafe - caller must ensure validity)
    /// 
    /// # Safety
    /// - ptr must be a valid pointer to a null-terminated C string
    /// - The string must remain valid for the lifetime 'a
    pub unsafe fn from_ptr(ptr: *const c_char) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(CStrWrapper {
                inner: CStr::from_ptr(ptr),
            })
        }
    }
    
    /// Converts to our String type with UTF-8 validation
    pub fn to_string(&self) -> String {
        String::from(self.inner.to_string_lossy().as_ref())
    }
    
    /// Converts to Rust str with UTF-8 validation
    pub fn to_str(&self) -> Result<&str, std::str::Utf8Error> {
        self.inner.to_str()
    }
    
    /// Gets the byte slice (without null terminator)
    pub fn to_bytes(&self) -> &[u8] {
        self.inner.to_bytes()
    }
    
    /// Gets the raw C string bytes (with null terminator)
    pub fn to_bytes_with_nul(&self) -> &[u8] {
        self.inner.to_bytes_with_nul()
    }
}

/// Error handling for FFI operations
#[derive(Debug, Clone)]
pub enum FfiError {
    NullPointer,
    InvalidUtf8,
    NulError(std::ffi::NulError),
    AllocationFailure,
    InvalidParameter,
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::NullPointer => write!(f, "Null pointer encountered"),
            FfiError::InvalidUtf8 => write!(f, "Invalid UTF-8 sequence"),
            FfiError::NulError(e) => write!(f, "Null byte in string: {}", e),
            FfiError::AllocationFailure => write!(f, "Memory allocation failed"),
            FfiError::InvalidParameter => write!(f, "Invalid parameter"),
        }
    }
}

impl std::error::Error for FfiError {}

impl From<std::ffi::NulError> for FfiError {
    fn from(e: std::ffi::NulError) -> Self {
        FfiError::NulError(e)
    }
}

/// Result type for FFI operations
pub type FfiResult<T> = Result<T, FfiError>;

/// Memory management utilities for FFI
pub mod memory {
    use super::*;
    
    /// Allocates memory using the C allocator (malloc)
    /// 
    /// # Safety
    /// The returned pointer must be freed using `free_c_memory`
    pub unsafe fn alloc_c_memory(size: usize) -> *mut c_void {
        if size == 0 {
            return ptr::null_mut();
        }
        libc::malloc(size)
    }
    
    /// Frees memory allocated by `alloc_c_memory` or C libraries
    /// 
    /// # Safety
    /// - ptr must have been allocated by malloc or compatible allocator
    /// - ptr must not be used after this call
    pub unsafe fn free_c_memory(ptr: *mut c_void) {
        if !ptr.is_null() {
            libc::free(ptr);
        }
    }
    
    /// Reallocates memory using C realloc
    /// 
    /// # Safety
    /// - ptr must be null or allocated by malloc/realloc
    /// - The returned pointer must be freed using `free_c_memory`
    pub unsafe fn realloc_c_memory(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        libc::realloc(ptr, new_size)
    }
    
    /// Copies memory using C memcpy
    /// 
    /// # Safety
    /// - dest and src must be valid for len bytes
    /// - dest and src must not overlap (use memmove for overlapping)
    pub unsafe fn copy_c_memory(dest: *mut c_void, src: *const c_void, len: usize) {
        libc::memcpy(dest, src, len);
    }
    
    /// Safe wrapper for allocating and copying data to C memory
    pub fn copy_to_c_memory(data: &[u8]) -> FfiResult<*mut c_void> {
        if data.is_empty() {
            return Ok(ptr::null_mut());
        }
        
        unsafe {
            let ptr = alloc_c_memory(data.len());
            if ptr.is_null() {
                return Err(FfiError::AllocationFailure);
            }
            
            copy_c_memory(ptr, data.as_ptr() as *const c_void, data.len());
            Ok(ptr)
        }
    }
}

/// Type conversion utilities for FFI
pub mod convert {
    use super::*;
    
    /// Converts Rust bool to C int (0 or 1)
    pub fn bool_to_c_int(b: bool) -> c_int {
        if b { 1 } else { 0 }
    }
    
    /// Converts C int to Rust bool (0 is false, anything else is true)
    pub fn c_int_to_bool(i: c_int) -> bool {
        i != 0
    }
    
    /// Safely converts C size_t to Rust usize
    pub fn c_size_to_usize(size: libc::size_t) -> usize {
        size as usize
    }
    
    /// Converts Rust usize to C size_t
    pub fn usize_to_c_size(size: usize) -> libc::size_t {
        size as libc::size_t
    }
}

/// Array handling utilities for C interop
pub struct CArrayWrapper<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
    owns_memory: bool,
}

impl<T> CArrayWrapper<T> {
    /// Creates a new C array with the given capacity
    pub fn with_capacity(capacity: usize) -> FfiResult<Self> {
        if capacity == 0 {
            return Ok(CArrayWrapper {
                ptr: ptr::null_mut(),
                len: 0,
                capacity: 0,
                owns_memory: true,
            });
        }
        
        unsafe {
            let size = std::mem::size_of::<T>() * capacity;
            let ptr = memory::alloc_c_memory(size) as *mut T;
            if ptr.is_null() {
                return Err(FfiError::AllocationFailure);
            }
            
            Ok(CArrayWrapper {
                ptr,
                len: 0,
                capacity,
                owns_memory: true,
            })
        }
    }
    
    /// Wraps an existing C array (doesn't take ownership)
    /// 
    /// # Safety
    /// - ptr must be valid for len elements of type T
    /// - The array must remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut T, len: usize) -> Self {
        CArrayWrapper {
            ptr,
            len,
            capacity: len,
            owns_memory: false,
        }
    }
    
    /// Gets a pointer to the array data
    pub fn as_ptr(&self) -> *const T {
        self.ptr as *const T
    }
    
    /// Gets a mutable pointer to the array data
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.ptr
    }
    
    /// Gets the length of the array
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Checks if the array is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Gets the capacity of the array
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Pushes an element to the array (if there's capacity)
    pub fn push(&mut self, item: T) -> FfiResult<()> {
        if self.len >= self.capacity {
            return Err(FfiError::InvalidParameter);
        }
        
        unsafe {
            ptr::write(self.ptr.add(self.len), item);
        }
        self.len += 1;
        Ok(())
    }
    
    /// Gets an element at the given index
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            None
        } else {
            unsafe { Some(&*self.ptr.add(index)) }
        }
    }
    
    /// Gets a mutable reference to an element at the given index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            None
        } else {
            unsafe { Some(&mut *self.ptr.add(index)) }
        }
    }
    
    /// Converts to a Rust slice
    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }
    
    /// Converts to a mutable Rust slice
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.ptr.is_null() || self.len == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    }
}

impl<T> Drop for CArrayWrapper<T> {
    fn drop(&mut self) {
        if self.owns_memory && !self.ptr.is_null() {
            unsafe {
                // Drop all elements
                for i in 0..self.len {
                    ptr::drop_in_place(self.ptr.add(i));
                }
                // Free the memory
                memory::free_c_memory(self.ptr as *mut c_void);
            }
        }
    }
}

/// Macro for easier C function binding with error handling
#[macro_export]
macro_rules! c_function_wrapper {
    ($fn_name:ident, $c_fn:ident, ($($arg:ident: $arg_type:ty),*) -> $ret_type:ty) => {
        pub fn $fn_name($($arg: $arg_type),*) -> FfiResult<$ret_type> {
            unsafe {
                let result = $c_fn($($arg),*);
                // Add appropriate error checking here based on the function
                Ok(result)
            }
        }
    };
}

/// Common C library function wrappers
pub mod libc_wrappers {
    use super::*;
    
    /// Safe wrapper for strlen
    pub fn string_length(s: &CStrWrapper) -> usize {
        s.to_bytes().len()
    }
    
    /// Safe wrapper for strcmp
    pub fn string_compare(s1: &CStrWrapper, s2: &CStrWrapper) -> i32 {
        unsafe {
            libc::strcmp(s1.inner.as_ptr(), s2.inner.as_ptr()) as i32
        }
    }
    
    /// Safe wrapper for strcpy (destination must have enough space)
    pub fn string_copy(dest: &mut CStringWrapper, src: &CStrWrapper) -> FfiResult<()> {
        if dest.len() < src.to_bytes().len() {
            return Err(FfiError::InvalidParameter);
        }
        
        unsafe {
            libc::strcpy(dest.as_mut_ptr(), src.inner.as_ptr());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_c_string_wrapper() {
        let s = CStringWrapper::new("Hello, World!").unwrap();
        assert_eq!(s.len(), 13);
        assert!(!s.is_empty());
        assert_eq!(s.to_string().as_str(), "Hello, World!");
    }
    
    #[test]
    fn test_c_str_wrapper() {
        let c_string = CStringWrapper::new("Test string").unwrap();
        
        unsafe {
            let c_str = CStrWrapper::from_ptr(c_string.as_ptr()).unwrap();
            assert_eq!(c_str.to_string().as_str(), "Test string");
            assert_eq!(c_str.to_bytes().len(), 11);
        }
    }
    
    #[test]
    fn test_memory_management() {
        unsafe {
            let ptr = memory::alloc_c_memory(1024);
            assert!(!ptr.is_null());
            memory::free_c_memory(ptr);
        }
        
        let data = b"Hello, World!";
        let c_ptr = memory::copy_to_c_memory(data).unwrap();
        assert!(!c_ptr.is_null());
        unsafe { memory::free_c_memory(c_ptr); }
    }
    
    #[test]
    fn test_c_array_wrapper() {
        let mut array = CArrayWrapper::<i32>::with_capacity(10).unwrap();
        assert_eq!(array.capacity(), 10);
        assert_eq!(array.len(), 0);
        
        array.push(42).unwrap();
        array.push(100).unwrap();
        assert_eq!(array.len(), 2);
        assert_eq!(array.get(0), Some(&42));
        assert_eq!(array.get(1), Some(&100));
        
        let slice = array.as_slice();
        assert_eq!(slice, &[42, 100]);
    }
    
    #[test]
    fn test_type_conversions() {
        assert_eq!(convert::bool_to_c_int(true), 1);
        assert_eq!(convert::bool_to_c_int(false), 0);
        assert!(convert::c_int_to_bool(1));
        assert!(convert::c_int_to_bool(42));
        assert!(!convert::c_int_to_bool(0));
    }
    
    #[test]
    fn test_string_operations() {
        let s1 = CStringWrapper::new("hello").unwrap();
        let s2 = CStringWrapper::new("world").unwrap();
        
        unsafe {
            let c_str1 = CStrWrapper::from_ptr(s1.as_ptr()).unwrap();
            let c_str2 = CStrWrapper::from_ptr(s2.as_ptr()).unwrap();
            
            let cmp_result = libc_wrappers::string_compare(&c_str1, &c_str2);
            assert!(cmp_result != 0); // "hello" != "world"
        }
    }
    
    #[test]
    fn test_error_handling() {
        // Test null error
        let result = CStringWrapper::new("hello\0world");
        assert!(result.is_err());
        
        // Test array overflow
        let mut array = CArrayWrapper::<i32>::with_capacity(1).unwrap();
        array.push(1).unwrap();
        let result = array.push(2);
        assert!(result.is_err());
    }
}