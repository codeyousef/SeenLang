//! Error handling types for Seen
//!
//! Provides Result and Option types with zero-cost abstractions

use std::fmt::{self, Display, Debug};

/// Option type for handling nullable values
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    #[inline(always)]
    pub const fn some(value: T) -> Self {
        Option::Some(value)
    }
    
    #[inline(always)]
    pub const fn none() -> Self {
        Option::None
    }
    
    #[inline(always)]
    pub fn is_some(&self) -> bool {
        matches!(self, Option::Some(_))
    }
    
    #[inline(always)]
    pub fn is_none(&self) -> bool {
        !self.is_some()
    }
    
    #[inline(always)]
    pub fn unwrap(self) -> T {
        match self {
            Option::Some(val) => val,
            Option::None => panic!("called `Option::unwrap()` on a `None` value"),
        }
    }
    
    #[inline(always)]
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Option::Some(val) => val,
            Option::None => default,
        }
    }
    
    #[inline(always)]
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        match self {
            Option::Some(val) => val,
            Option::None => f(),
        }
    }
    
    #[inline(always)]
    pub fn map<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Option::Some(val) => Option::Some(f(val)),
            Option::None => Option::None,
        }
    }
    
    #[inline(always)]
    pub fn and_then<U, F>(self, f: F) -> Option<U>
    where
        F: FnOnce(T) -> Option<U>,
    {
        match self {
            Option::Some(val) => f(val),
            Option::None => Option::None,
        }
    }
    
    #[inline(always)]
    pub fn filter<P>(self, predicate: P) -> Self
    where
        P: FnOnce(&T) -> bool,
    {
        match self {
            Option::Some(val) if predicate(&val) => Option::Some(val),
            _ => Option::None,
        }
    }
    
    #[inline(always)]
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Option::Some(ref val) => Option::Some(val),
            Option::None => Option::None,
        }
    }
    
    #[inline(always)]
    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Option::Some(ref mut val) => Option::Some(val),
            Option::None => Option::None,
        }
    }
}

impl<T> Default for Option<T> {
    #[inline(always)]
    fn default() -> Self {
        Option::None
    }
}

impl<T: Debug> Debug for Option<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Option::Some(val) => write!(f, "Some({:?})", val),
            Option::None => write!(f, "None"),
        }
    }
}

impl<T: Display> Display for Option<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Option::Some(val) => write!(f, "Some({})", val),
            Option::None => write!(f, "None"),
        }
    }
}

/// Result type for error handling
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    #[inline(always)]
    pub const fn ok(value: T) -> Self {
        Result::Ok(value)
    }
    
    #[inline(always)]
    pub const fn err(error: E) -> Self {
        Result::Err(error)
    }
    
    #[inline(always)]
    pub fn is_ok(&self) -> bool {
        matches!(self, Result::Ok(_))
    }
    
    #[inline(always)]
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }
    
    #[inline(always)]
    pub fn unwrap(self) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => panic!("called `Result::unwrap()` on an `Err` value"),
        }
    }
    
    #[inline(always)]
    pub fn unwrap_err(self) -> E {
        match self {
            Result::Ok(_) => panic!("called `Result::unwrap_err()` on an `Ok` value"),
            Result::Err(err) => err,
        }
    }
    
    #[inline(always)]
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => default,
        }
    }
    
    #[inline(always)]
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce(E) -> T,
    {
        match self {
            Result::Ok(val) => val,
            Result::Err(err) => f(err),
        }
    }
    
    #[inline(always)]
    pub fn map<U, F>(self, f: F) -> Result<U, E>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Result::Ok(val) => Result::Ok(f(val)),
            Result::Err(err) => Result::Err(err),
        }
    }
    
    #[inline(always)]
    pub fn map_err<F, O>(self, f: F) -> Result<T, O>
    where
        F: FnOnce(E) -> O,
    {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => Result::Err(f(err)),
        }
    }
    
    #[inline(always)]
    pub fn and_then<U, F>(self, f: F) -> Result<U, E>
    where
        F: FnOnce(T) -> Result<U, E>,
    {
        match self {
            Result::Ok(val) => f(val),
            Result::Err(err) => Result::Err(err),
        }
    }
    
    #[inline(always)]
    pub fn or_else<F, O>(self, f: F) -> Result<T, O>
    where
        F: FnOnce(E) -> Result<T, O>,
    {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => f(err),
        }
    }
    
    #[inline(always)]
    pub fn as_ref(&self) -> Result<&T, &E> {
        match self {
            Result::Ok(ref val) => Result::Ok(val),
            Result::Err(ref err) => Result::Err(err),
        }
    }
    
    #[inline(always)]
    pub fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match self {
            Result::Ok(ref mut val) => Result::Ok(val),
            Result::Err(ref mut err) => Result::Err(err),
        }
    }
}

impl<T: Debug, E: Debug> Debug for Result<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Result::Ok(val) => write!(f, "Ok({:?})", val),
            Result::Err(err) => write!(f, "Err({:?})", err),
        }
    }
}

impl<T: Display, E: Display> Display for Result<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Result::Ok(val) => write!(f, "Ok({})", val),
            Result::Err(err) => write!(f, "Err({})", err),
        }
    }
}

// Question mark operator support
impl<T, E> Result<T, E> {
    #[inline(always)]
    pub fn into_result(self) -> std::result::Result<T, E> {
        match self {
            Result::Ok(val) => std::result::Result::Ok(val),
            Result::Err(err) => std::result::Result::Err(err),
        }
    }
}

impl<T> Option<T> {
    #[inline(always)]
    pub fn into_option(self) -> std::option::Option<T> {
        match self {
            Option::Some(val) => std::option::Option::Some(val),
            Option::None => std::option::Option::None,
        }
    }
}