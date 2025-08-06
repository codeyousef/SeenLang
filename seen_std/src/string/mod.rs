//! UTF-8 native string handling with small string optimization
//!
//! Provides high-performance string types optimized for common operations

use std::alloc::{alloc, dealloc, realloc, Layout};
use std::borrow::Borrow;
use std::cmp::{Ordering, PartialEq, Eq, PartialOrd, Ord};
use std::fmt::{self, Display, Debug};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::ptr;
use std::slice;
use std::str;

// Small String Optimization (SSO) - strings up to 22 bytes stored inline
const SSO_CAPACITY: usize = 22;

/// A UTF-8 encoded, growable string with small string optimization
pub struct String {
    data: StringData,
}

enum StringData {
    Small([u8; SSO_CAPACITY], u8), // data, len
    Heap(*mut u8, usize, usize),    // ptr, len, cap
}

impl String {
    /// Creates a new empty string
    #[inline]
    pub fn new() -> Self {
        String {
            data: StringData::Small([0; SSO_CAPACITY], 0),
        }
    }

    /// Creates a string with specified capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity <= SSO_CAPACITY {
            Self::new()
        } else {
            let layout = Layout::array::<u8>(capacity).unwrap();
            let ptr = unsafe { alloc(layout) };
            
            String {
                data: StringData::Heap(ptr, 0, capacity),
            }
        }
    }

    /// Creates a string from a string slice
    pub fn from(s: &str) -> Self {
        let len = s.len();
        
        if len <= SSO_CAPACITY {
            let mut data = [0u8; SSO_CAPACITY];
            unsafe {
                ptr::copy_nonoverlapping(s.as_ptr(), data.as_mut_ptr(), len);
            }
            String {
                data: StringData::Small(data, len as u8),
            }
        } else {
            let capacity = len.next_power_of_two();
            let layout = Layout::array::<u8>(capacity).unwrap();
            let ptr = unsafe { 
                let p = alloc(layout);
                ptr::copy_nonoverlapping(s.as_ptr(), p, len);
                p
            };
            
            String {
                data: StringData::Heap(ptr, len, capacity),
            }
        }
    }

    /// Returns the length of the string in bytes
    #[inline]
    pub fn len(&self) -> usize {
        match &self.data {
            StringData::Small(_, len) => *len as usize,
            StringData::Heap(_, len, _) => *len,
        }
    }

    /// Returns true if the string is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the capacity of the string
    #[inline]
    pub fn capacity(&self) -> usize {
        match &self.data {
            StringData::Small(_, _) => SSO_CAPACITY,
            StringData::Heap(_, _, cap) => *cap,
        }
    }

    /// Returns a string slice
    #[inline]
    pub fn as_str(&self) -> &str {
        unsafe {
            let (ptr, len) = match &self.data {
                StringData::Small(data, len) => (data.as_ptr(), *len as usize),
                StringData::Heap(ptr, len, _) => (*ptr as *const u8, *len),
            };
            
            str::from_utf8_unchecked(slice::from_raw_parts(ptr, len))
        }
    }

    /// Returns a pointer to the string data
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        match &self.data {
            StringData::Small(data, _) => data.as_ptr(),
            StringData::Heap(ptr, _, _) => *ptr as *const u8,
        }
    }

    /// Pushes a character to the end of the string
    pub fn push(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.push_str(s);
    }

    /// Appends a string slice to the end
    pub fn push_str(&mut self, s: &str) {
        let additional = s.len();
        if additional == 0 {
            return;
        }

        let new_len = self.len() + additional;
        
        if new_len > self.capacity() {
            self.reserve(additional);
        }

        unsafe {
            let dst = match &mut self.data {
                StringData::Small(data, len) => {
                    let old_len = *len as usize;
                    *len = new_len as u8;
                    data.as_mut_ptr().add(old_len)
                }
                StringData::Heap(ptr, len, _) => {
                    let old_len = *len;
                    *len = new_len;
                    ptr.add(old_len)
                }
            };
            
            ptr::copy_nonoverlapping(s.as_ptr(), dst, additional);
        }
    }

    /// Appends another string
    pub fn append(&mut self, other: &String) {
        self.push_str(other.as_str());
    }

    /// Reserves capacity for at least `additional` more bytes
    pub fn reserve(&mut self, additional: usize) {
        let required = self.len() + additional;
        let current_cap = self.capacity();
        
        if required <= current_cap {
            return;
        }

        let new_cap = required.next_power_of_two();
        
        match &mut self.data {
            StringData::Small(data, len) => {
                // Transition from small to heap
                let old_len = *len as usize;
                let layout = Layout::array::<u8>(new_cap).unwrap();
                let new_ptr = unsafe {
                    let p = alloc(layout);
                    ptr::copy_nonoverlapping(data.as_ptr(), p, old_len);
                    p
                };
                
                self.data = StringData::Heap(new_ptr, old_len, new_cap);
            }
            StringData::Heap(ptr, _, cap) => {
                // Resize heap allocation
                unsafe {
                    let old_layout = Layout::array::<u8>(*cap).unwrap();
                    let new_layout = Layout::array::<u8>(new_cap).unwrap();
                    *ptr = realloc(*ptr, old_layout, new_layout.size());
                    *cap = new_cap;
                }
            }
        }
    }

    /// Inserts a character at the given byte position
    pub fn insert(&mut self, idx: usize, ch: char) {
        assert!(idx <= self.len());
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.insert_str(idx, s);
    }

    /// Inserts a string slice at the given byte position
    pub fn insert_str(&mut self, idx: usize, s: &str) {
        assert!(idx <= self.len());
        
        let len = s.len();
        if len == 0 {
            return;
        }

        let old_len = self.len();
        let new_len = old_len + len;
        
        if new_len > self.capacity() {
            self.reserve(len);
        }

        unsafe {
            let ptr = match &mut self.data {
                StringData::Small(data, slen) => {
                    *slen = new_len as u8;
                    data.as_mut_ptr()
                }
                StringData::Heap(ptr, hlen, _) => {
                    *hlen = new_len;
                    *ptr
                }
            };
            
            // Shift existing data
            ptr::copy(ptr.add(idx), ptr.add(idx + len), old_len - idx);
            
            // Insert new data
            ptr::copy_nonoverlapping(s.as_ptr(), ptr.add(idx), len);
        }
    }

    /// Removes a character at the given byte position
    pub fn remove(&mut self, idx: usize) -> char {
        let ch = self.as_str()[idx..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        
        let len = self.len();
        assert!(idx + ch_len <= len);
        
        unsafe {
            let ptr = match &mut self.data {
                StringData::Small(data, slen) => {
                    *slen = (len - ch_len) as u8;
                    data.as_mut_ptr()
                }
                StringData::Heap(ptr, hlen, _) => {
                    *hlen = len - ch_len;
                    *ptr
                }
            };
            
            ptr::copy(ptr.add(idx + ch_len), ptr.add(idx), len - idx - ch_len);
        }
        
        ch
    }

    /// Truncates the string to the given byte length
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len() {
            return;
        }
        
        match &mut self.data {
            StringData::Small(_, len) => *len = new_len as u8,
            StringData::Heap(_, len, _) => *len = new_len,
        }
    }

    /// Returns a substring from start to end byte positions
    pub fn slice(&self, start: usize, end: usize) -> StringRef {
        assert!(start <= end && end <= self.len());
        let s = &self.as_str()[start..end];
        StringRef::from(s)
    }

    /// Returns a substring from start to end character positions (UTF-8 safe)
    pub fn slice_chars(&self, start: usize, end: usize) -> StringRef {
        let s = self.as_str();
        let mut chars = s.char_indices();
        
        let start_byte = chars.nth(start).map(|(i, _)| i).unwrap_or(s.len());
        let end_byte = if end > start {
            chars.nth(end - start - 1).map(|(i, _)| i).unwrap_or(s.len())
        } else {
            start_byte
        };
        
        StringRef::from(&s[start_byte..end_byte])
    }

    /// Returns the number of characters (not bytes)
    pub fn char_count(&self) -> usize {
        self.as_str().chars().count()
    }

    /// Returns an iterator over the characters
    pub fn chars(&self) -> str::Chars {
        self.as_str().chars()
    }

    /// Checks if the string contains a substring
    pub fn contains(&self, pat: &str) -> bool {
        self.as_str().contains(pat)
    }

    /// Finds the first occurrence of a substring
    pub fn find(&self, pat: &str) -> Option<usize> {
        self.as_str().find(pat)
    }

    /// Finds the last occurrence of a substring
    pub fn rfind(&self, pat: &str) -> Option<usize> {
        self.as_str().rfind(pat)
    }

    /// Checks if the string starts with a pattern
    pub fn starts_with(&self, pat: &str) -> bool {
        self.as_str().starts_with(pat)
    }

    /// Checks if the string ends with a pattern
    pub fn ends_with(&self, pat: &str) -> bool {
        self.as_str().ends_with(pat)
    }

    /// Converts to lowercase
    pub fn to_lowercase(&self) -> String {
        String::from(&self.as_str().to_lowercase())
    }

    /// Converts to uppercase
    pub fn to_uppercase(&self) -> String {
        String::from(&self.as_str().to_uppercase())
    }

    /// Returns a trimmed string
    pub fn trim(&self) -> StringRef {
        StringRef::from(self.as_str().trim())
    }

    /// Returns a string with leading whitespace removed
    pub fn trim_start(&self) -> StringRef {
        StringRef::from(self.as_str().trim_start())
    }

    /// Returns a string with trailing whitespace removed
    pub fn trim_end(&self) -> StringRef {
        StringRef::from(self.as_str().trim_end())
    }

    /// Splits the string by a separator
    pub fn split<'a>(&'a self, sep: char) -> impl Iterator<Item = StringRef<'a>> {
        self.as_str().split(sep).map(StringRef::from)
    }

    /// Splits the string by whitespace
    pub fn split_whitespace<'a>(&'a self) -> impl Iterator<Item = StringRef<'a>> {
        self.as_str().split_whitespace().map(StringRef::from)
    }

    /// Replaces all occurrences of a pattern
    pub fn replace(&self, from: &str, to: &str) -> String {
        String::from(&self.as_str().replace(from, to))
    }

    /// Replaces the first occurrence of a pattern
    pub fn replace_first(&self, from: &str, to: &str) -> String {
        String::from(&self.as_str().replacen(from, to, 1))
    }

    /// Formats a string with placeholders
    pub fn format(fmt: &str, args: &[&str]) -> String {
        let mut result = String::from(fmt);
        for (_i, arg) in args.iter().enumerate() {
            let placeholder = String::from("{}");
            result = result.replace_first(placeholder.as_str(), arg);
        }
        result
    }

    /// Parses the string into a value
    pub fn parse<T: str::FromStr>(&self) -> Result<T, T::Err> {
        self.as_str().parse()
    }

    /// Checks if the string is heap allocated
    #[inline]
    pub fn heap_allocated(&self) -> bool {
        matches!(self.data, StringData::Heap(_, _, _))
    }
}

impl Drop for String {
    fn drop(&mut self) {
        if let StringData::Heap(ptr, _, cap) = self.data {
            unsafe {
                let layout = Layout::array::<u8>(cap).unwrap();
                dealloc(ptr, layout);
            }
        }
    }
}

impl Clone for String {
    fn clone(&self) -> Self {
        String::from(self.as_str())
    }
}

impl Default for String {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for String {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for String {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for String {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for String {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for String {}

impl PartialOrd for String {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for String {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for String {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Display for String {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl Debug for String {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

/// A lightweight, non-owning reference to a UTF-8 string
#[derive(Copy, Clone)]
pub struct StringRef<'a> {
    data: &'a str,
}

impl<'a> StringRef<'a> {
    /// Creates a StringRef from a string slice
    #[inline]
    pub fn from(s: &'a str) -> Self {
        StringRef { data: s }
    }

    /// Returns the underlying string slice
    #[inline]
    pub fn as_str(&self) -> &'a str {
        self.data
    }

    /// Returns the length in bytes
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<'a> Deref for StringRef<'a> {
    type Target = str;

    fn deref(&self) -> &str {
        self.data
    }
}

impl<'a> Display for StringRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.data, f)
    }
}

impl<'a> Debug for StringRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self.data, f)
    }
}

/// A string builder for efficient string concatenation
pub struct StringBuilder {
    buffer: Vec<u8>,
}

impl StringBuilder {
    /// Creates a new empty string builder
    #[inline]
    pub fn new() -> Self {
        StringBuilder {
            buffer: Vec::new(),
        }
    }

    /// Creates a string builder with specified capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        StringBuilder {
            buffer: Vec::with_capacity(capacity),
        }
    }

    /// Appends a string slice
    #[inline]
    pub fn append(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
    }

    /// Appends a character
    #[inline]
    pub fn append_char(&mut self, ch: char) {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        self.append(s);
    }

    /// Appends a formatted string
    pub fn append_format(&mut self, _fmt: &str, arg: impl Display) {
        use std::fmt::Write;
        write!(self, "{}", arg).unwrap();
    }

    /// Returns the current length
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Builds the final string
    pub fn build(self) -> String {
        String::from(unsafe { str::from_utf8_unchecked(&self.buffer) })
    }
}

impl Default for StringBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Write for StringBuilder {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.append(s);
        Ok(())
    }
}

// Send and Sync implementations
unsafe impl Send for String {}
unsafe impl Sync for String {}
unsafe impl<'a> Send for StringRef<'a> {}
unsafe impl<'a> Sync for StringRef<'a> {}