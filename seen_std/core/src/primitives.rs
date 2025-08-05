//! Primitive types for the Seen language

use serde::{Deserialize, Serialize};
use std::fmt;

/// 8-bit signed integer
pub type i8 = std::primitive::i8;
/// 16-bit signed integer  
pub type i16 = std::primitive::i16;
/// 32-bit signed integer
pub type i32 = std::primitive::i32;
/// 64-bit signed integer
pub type i64 = std::primitive::i64;
/// 128-bit signed integer
pub type i128 = std::primitive::i128;

/// 8-bit unsigned integer
pub type u8 = std::primitive::u8;
/// 16-bit unsigned integer
pub type u16 = std::primitive::u16;
/// 32-bit unsigned integer
pub type u32 = std::primitive::u32;
/// 64-bit unsigned integer
pub type u64 = std::primitive::u64;
/// 128-bit unsigned integer
pub type u128 = std::primitive::u128;

/// 32-bit floating point
pub type f32 = std::primitive::f32;
/// 64-bit floating point
pub type f64 = std::primitive::f64;

/// Boolean type
pub type bool = std::primitive::bool;
/// Character type (Unicode scalar value)
pub type char = std::primitive::char;
/// String slice type
pub type str = std::primitive::str;

/// Unit type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unit;

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

/// Never type (for functions that never return)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Never {}

impl fmt::Display for Never {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

/// Dynamic string type
pub type String = std::string::String;

/// Dynamic array type
pub type Vec<T> = std::vec::Vec<T>;

/// Hash map type
pub type HashMap<K, V> = std::collections::HashMap<K, V>;

/// Hash set type
pub type HashSet<T> = std::collections::HashSet<T>;

/// Box type for heap allocation
pub type Box<T> = std::boxed::Box<T>;

/// Reference counted pointer
pub type Rc<T> = std::rc::Rc<T>;

/// Atomic reference counted pointer
pub type Arc<T> = std::sync::Arc<T>;

/// Constants for common values
pub const TRUE: bool = true;
pub const FALSE: bool = false;
pub const UNIT: Unit = Unit;