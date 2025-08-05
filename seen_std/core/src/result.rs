//! Result type for error handling

use serde::{Deserialize, Serialize};
use std::fmt;

/// Result type for operations that can fail
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Result<T, E> {
    /// Success case containing a value
    Ok(T),
    /// Error case containing an error
    Err(E),
}

impl<T, E> Result<T, E> {
    /// Returns `true` if the result is `Ok`
    pub fn is_ok(&self) -> bool {
        matches!(self, Result::Ok(_))
    }
    
    /// Returns `true` if the result is `Err`
    pub fn is_err(&self) -> bool {
        matches!(self, Result::Err(_))
    }
    
    /// Converts from `Result<T, E>` to `Option<T>`
    pub fn ok(self) -> Option<T> {
        match self {
            Result::Ok(x) => Option::Some(x),
            Result::Err(_) => Option::None,
        }
    }
    
    /// Converts from `Result<T, E>` to `Option<E>`
    pub fn err(self) -> Option<E> {
        match self {
            Result::Ok(_) => Option::None,
            Result::Err(x) => Option::Some(x),
        }
    }
    
    /// Maps a `Result<T, E>` to `Result<U, E>` by applying a function to a contained `Ok` value
    pub fn map<U, F>(self, f: F) -> Result<U, E>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Result::Ok(x) => Result::Ok(f(x)),
            Result::Err(e) => Result::Err(e),
        }
    }
    
    /// Maps a `Result<T, E>` to `Result<T, F>` by applying a function to a contained `Err` value
    pub fn map_err<F, O>(self, f: O) -> Result<T, F>
    where
        O: FnOnce(E) -> F,
    {
        match self {
            Result::Ok(x) => Result::Ok(x),
            Result::Err(e) => Result::Err(f(e)),
        }
    }
    
    /// Returns the contained `Ok` value or panics
    pub fn unwrap(self) -> T
    where
        E: fmt::Debug,
    {
        match self {
            Result::Ok(x) => x,
            Result::Err(e) => panic!("called `Result::unwrap()` on an `Err` value: {:?}", e),
        }
    }
    
    /// Returns the contained `Ok` value or a provided default
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(x) => x,
            Result::Err(_) => default,
        }
    }
    
    /// Returns the contained `Ok` value or computes it from a closure
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce(E) -> T,
    {
        match self {
            Result::Ok(x) => x,
            Result::Err(e) => f(e),
        }
    }
    
    /// Returns `res` if the result is `Ok`, otherwise returns the `Err` value
    pub fn and<U>(self, res: Result<U, E>) -> Result<U, E> {
        match self {
            Result::Ok(_) => res,
            Result::Err(e) => Result::Err(e),
        }
    }
    
    /// Calls `f` if the result is `Ok`, otherwise returns the `Err` value
    pub fn and_then<U, F>(self, f: F) -> Result<U, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        match self {
            Result::Ok(x) => f(x),
            Result::Err(e) => Result::Err(e),
        }
    }
    
    /// Returns `res` if the result is `Err`, otherwise returns the `Ok` value
    pub fn or<F>(self, res: Result<T, F>) -> Result<T, F> {
        match self {
            Result::Ok(x) => Result::Ok(x),
            Result::Err(_) => res,
        }
    }
    
    /// Calls `f` if the result is `Err`, otherwise returns the `Ok` value
    pub fn or_else<F, O>(self, f: O) -> Result<T, F>
    where
        O: FnOnce(E) -> Result<T, F>,
    {
        match self {
            Result::Ok(x) => Result::Ok(x),
            Result::Err(e) => f(e),
        }
    }
}

impl<T, E: fmt::Display> fmt::Display for Result<T, E>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Result::Ok(x) => write!(f, "Ok({})", x),
            Result::Err(e) => write!(f, "Err({})", e),
        }
    }
}