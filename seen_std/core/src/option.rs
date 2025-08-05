//! Option type for nullable values

use serde::{Deserialize, Serialize};
use std::fmt;

/// Option type for values that may or may not be present
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Option<T> {
    /// Some value of type T
    Some(T),
    /// No value
    None,
}

impl<T> Option<T> {
    /// Returns `true` if the option is `Some`
    pub fn is_some(&self) -> bool {
        matches!(self, Option::Some(_))
    }
    
    /// Returns `true` if the option is `None`
    pub fn is_none(&self) -> bool {
        matches!(self, Option::None)
    }
    
    /// Maps an `Option<T>` to `Option<U>` by applying a function to a contained value
    pub fn map<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Option::Some(x) => Option::Some(f(x)),
            Option::None => Option::None,
        }
    }
    
    /// Returns the provided default result (if none), or applies a function to the contained value (if any)
    pub fn map_or<U, F>(self, default: U, f: F) -> U
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Option::Some(x) => f(x),
            Option::None => default,
        }
    }
    
    /// Computes a default function result (if none), or applies a different function to the contained value (if any)
    pub fn map_or_else<U, D, F>(self, default: D, f: F) -> U
    where
        D: FnOnce() -> U,
        F: FnOnce(T) -> U,
    {
        match self {
            Option::Some(x) => f(x),
            Option::None => default(),
        }
    }
    
    /// Transforms the `Option<T>` into a `Result<T, E>`, mapping `Some(v)` to `Ok(v)` and `None` to `Err(err)`
    pub fn ok_or<E>(self, err: E) -> Result<T, E> {
        match self {
            Option::Some(x) => Result::Ok(x),
            Option::None => Result::Err(err),
        }
    }
    
    /// Transforms the `Option<T>` into a `Result<T, E>`, mapping `Some(v)` to `Ok(v)` and `None` to `Err(err())`
    pub fn ok_or_else<E, F>(self, err: F) -> Result<T, E>
    where
        F: FnOnce() -> E,
    {
        match self {
            Option::Some(x) => Result::Ok(x),
            Option::None => Result::Err(err()),
        }
    }
    
    /// Returns the contained `Some` value or panics
    pub fn unwrap(self) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => panic!("called `Option::unwrap()` on a `None` value"),
        }
    }
    
    /// Returns the contained `Some` value or a provided default
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => default,
        }
    }
    
    /// Returns the contained `Some` value or computes it from a closure
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        match self {
            Option::Some(x) => x,
            Option::None => f(),
        }
    }
    
    /// Returns `None` if the option is `None`, otherwise returns `optb`
    pub fn and<U>(self, optb: Option<U>) -> Option<U> {
        match self {
            Option::Some(_) => optb,
            Option::None => Option::None,
        }
    }
    
    /// Returns `None` if the option is `None`, otherwise calls `f` with the wrapped value and returns the result
    pub fn and_then<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> Option<U>,
    {
        match self {
            Option::Some(x) => f(x),
            Option::None => Option::None,
        }
    }
    
    /// Returns the option if it contains a value, otherwise returns `optb`
    pub fn or(self, optb: Option<T>) -> Option<T> {
        match self {
            Option::Some(_) => self,
            Option::None => optb,
        }
    }
    
    /// Returns the option if it contains a value, otherwise calls `f` and returns the result
    pub fn or_else<F>(self, f: F) -> Option<T>
    where
        F: FnOnce() -> Option<T>,
    {
        match self {
            Option::Some(_) => self,
            Option::None => f(),
        }
    }
    
    /// Returns `Some` if exactly one of `self`, `optb` is `Some`, otherwise returns `None`
    pub fn xor(self, optb: Option<T>) -> Option<T> {
        match (self, optb) {
            (Option::Some(a), Option::None) => Option::Some(a),
            (Option::None, Option::Some(b)) => Option::Some(b),
            _ => Option::None,
        }
    }
    
    /// Returns the option if it satisfies the predicate, otherwise returns `None`
    pub fn filter<P>(self, predicate: P) -> Option<T>
    where
        P: FnOnce(&T) -> bool,
    {
        match self {
            Option::Some(x) => {
                if predicate(&x) {
                    Option::Some(x)
                } else {
                    Option::None
                }
            }
            Option::None => Option::None,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Option<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Option::Some(x) => write!(f, "Some({})", x),
            Option::None => write!(f, "None"),
        }
    }
}

// Re-export the Result type for convenience
use crate::result::Result;