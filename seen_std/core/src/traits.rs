//! Core traits for the Seen language
//! 
//! This module defines fundamental traits that will be used
//! throughout the Seen standard library and user code.

use serde::{Deserialize, Serialize};

/// Trait for types that can be displayed as strings
pub trait Display {
    fn display(&self) -> String;
}

/// Trait for types that can be debugged
pub trait Debug {
    fn debug(&self) -> String;
}

/// Trait for types that can be cloned
pub trait Clone {
    fn clone(&self) -> Self;
}

/// Trait for types that can be compared for equality
pub trait Eq {
    fn eq(&self, other: &Self) -> bool;
    
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

/// Trait for types that can be ordered
pub trait Ord: Eq {
    fn cmp(&self, other: &Self) -> Ordering;
    
    fn lt(&self, other: &Self) -> bool {
        matches!(self.cmp(other), Ordering::Less)
    }
    
    fn le(&self, other: &Self) -> bool {
        !matches!(self.cmp(other), Ordering::Greater)
    }
    
    fn gt(&self, other: &Self) -> bool {
        matches!(self.cmp(other), Ordering::Greater)
    }
    
    fn ge(&self, other: &Self) -> bool {
        !matches!(self.cmp(other), Ordering::Less)
    }
}

/// Ordering result for comparisons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ordering {
    Less,
    Equal,
    Greater,
}

/// Trait for types that can be hashed
pub trait Hash {
    fn hash(&self) -> u64;
}

/// Trait for types that can be converted from other types
pub trait From<T> {
    fn from(value: T) -> Self;
}

/// Trait for types that can be converted into other types
pub trait Into<T> {
    fn into(self) -> T;
}

/// Automatic implementation of Into when From is implemented
impl<T, U> Into<U> for T
where
    U: From<T>,
{
    fn into(self) -> U {
        U::from(self)
    }
}

/// Trait for iterators
pub trait Iterator {
    type Item;
    
    fn next(&mut self) -> Option<Self::Item>;
    
    fn collect<C: FromIterator<Self::Item>>(self) -> C
    where
        Self: Sized,
    {
        C::from_iter(self)
    }
}

/// Trait for types that can be created from an iterator
pub trait FromIterator<T> {
    fn from_iter<I: Iterator<Item = T>>(iter: I) -> Self;
}