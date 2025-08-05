//! Source location tracking for the Seen compiler

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a position in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

impl Position {
    pub fn new(line: u32, column: u32, offset: u32) -> Self {
        Self { line, column, offset }
    }
    
    pub fn start() -> Self {
        Self::new(1, 1, 0)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Represents a span of source code from start to end position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: Position,
    pub end: Position,
    pub file_id: u32,
}

impl Span {
    pub fn new(start: Position, end: Position, file_id: u32) -> Self {
        Self { start, end, file_id }
    }
    
    pub fn single(pos: Position, file_id: u32) -> Self {
        Self::new(pos, pos, file_id)
    }
    
    pub fn combine(self, other: Span) -> Span {
        assert_eq!(self.file_id, other.file_id, "Cannot combine spans from different files");
        Span::new(
            if self.start.offset <= other.start.offset { self.start } else { other.start },
            if self.end.offset >= other.end.offset { self.end } else { other.end },
            self.file_id,
        )
    }
    
    pub fn contains(&self, pos: Position) -> bool {
        self.start.offset <= pos.offset && pos.offset <= self.end.offset
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start.line == self.end.line {
            write!(f, "{}:{}-{}", self.start.line, self.start.column, self.end.column)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

/// A value with associated source location information
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
    
    pub fn map<U, F>(self, f: F) -> Spanned<U>
    where
        F: FnOnce(T) -> U,
    {
        Spanned::new(f(self.value), self.span)
    }
    
    pub fn as_ref(&self) -> Spanned<&T> {
        Spanned::new(&self.value, self.span)
    }
}

impl<T: fmt::Display> fmt::Display for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}", self.value, self.span)
    }
}