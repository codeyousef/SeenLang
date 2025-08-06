//! Rope data structure for efficient text manipulation
//!
//! Optimized for incremental compilation where large files need frequent
//! insertions, deletions, and substring operations. Uses tree-based approach
//! with leaf size optimization for cache efficiency.

use crate::string::String;
use std::fmt;

/// Minimum size for leaf nodes - optimized for cache lines
const MIN_LEAF_SIZE: usize = 64;
/// Maximum size for leaf nodes before splitting
const MAX_LEAF_SIZE: usize = 4096;
/// Minimum size for branches before merging
const MIN_BRANCH_SIZE: usize = 128;

/// A rope node - either a leaf containing text or a branch with children
#[derive(Clone)]
pub enum RopeNode {
    Leaf {
        text: String,
        byte_len: usize,
        char_len: usize,
    },
    Branch {
        left: Box<RopeNode>,
        right: Box<RopeNode>,
        byte_len: usize,
        char_len: usize,
    },
}

impl RopeNode {
    /// Creates a new leaf node from text
    fn new_leaf(text: String) -> Self {
        let byte_len = text.len();
        let char_len = text.char_count();
        RopeNode::Leaf { text, byte_len, char_len }
    }
    
    /// Creates a new branch node from two children
    fn new_branch(left: Box<RopeNode>, right: Box<RopeNode>) -> Self {
        let byte_len = left.byte_len() + right.byte_len();
        let char_len = left.char_len() + right.char_len();
        RopeNode::Branch { left, right, byte_len, char_len }
    }
    
    /// Returns the byte length of this node
    pub fn byte_len(&self) -> usize {
        match self {
            RopeNode::Leaf { byte_len, .. } => *byte_len,
            RopeNode::Branch { byte_len, .. } => *byte_len,
        }
    }
    
    /// Returns the character length of this node
    pub fn char_len(&self) -> usize {
        match self {
            RopeNode::Leaf { char_len, .. } => *char_len,
            RopeNode::Branch { char_len, .. } => *char_len,
        }
    }
    
    /// Checks if this node needs rebalancing
    fn needs_rebalance(&self) -> bool {
        match self {
            RopeNode::Leaf { byte_len, .. } => *byte_len > MAX_LEAF_SIZE,
            RopeNode::Branch { left, right, .. } => {
                // Check if children are extremely unbalanced
                let left_len = left.byte_len();
                let right_len = right.byte_len();
                if left_len == 0 || right_len == 0 {
                    true
                } else {
                    let ratio = left_len.max(right_len) / left_len.min(right_len);
                    ratio > 8 // Rebalance if ratio exceeds 8:1
                }
            }
        }
    }
    
    /// Splits a leaf node if it's too large
    fn split_if_needed(self) -> RopeNode {
        match self {
            RopeNode::Leaf { text, .. } if text.len() > MAX_LEAF_SIZE => {
                // Split at middle character boundary
                let split_point = MAX_LEAF_SIZE / 2;
                let mut actual_split = split_point;
                
                // Find character boundary
                while actual_split > 0 && !text.as_str().is_char_boundary(actual_split) {
                    actual_split -= 1;
                }
                
                if actual_split == 0 {
                    actual_split = split_point;
                    while actual_split < text.len() && !text.as_str().is_char_boundary(actual_split) {
                        actual_split += 1;
                    }
                }
                
                let left_text = String::from(&text.as_str()[..actual_split]);
                let right_text = String::from(&text.as_str()[actual_split..]);
                
                RopeNode::new_branch(
                    Box::new(RopeNode::new_leaf(left_text)),
                    Box::new(RopeNode::new_leaf(right_text)),
                )
            }
            node => node,
        }
    }
    
    /// Merges small adjacent leaf nodes
    fn merge_if_needed(self) -> RopeNode {
        match self {
            RopeNode::Branch { left, right, .. } => {
                match (left.as_ref(), right.as_ref()) {
                    (RopeNode::Leaf { text: left_text, .. }, 
                     RopeNode::Leaf { text: right_text, .. }) => {
                        let total_len = left_text.len() + right_text.len();
                        if total_len < MIN_BRANCH_SIZE {
                            // Merge the leaves
                            let mut merged = left_text.clone();
                            merged.push_str(right_text.as_str());
                            RopeNode::new_leaf(merged)
                        } else {
                            RopeNode::new_branch(left, right)
                        }
                    }
                    _ => RopeNode::new_branch(left, right),
                }
            }
            node => node,
        }
    }
}

/// High-performance rope data structure for text manipulation
#[derive(Clone)]
pub struct Rope {
    root: Option<Box<RopeNode>>,
}

impl Rope {
    /// Creates a new empty rope
    pub fn new() -> Self {
        Rope { root: None }
    }
    
    /// Creates a rope from a string
    pub fn from_string(text: String) -> Self {
        if text.is_empty() {
            Rope::new()
        } else {
            let node = RopeNode::new_leaf(text).split_if_needed();
            Rope { root: Some(Box::new(node)) }
        }
    }
    
    /// Creates a rope from a string slice
    pub fn from_str(text: &str) -> Self {
        Self::from_string(String::from(text))
    }
    
    /// Returns the byte length of the rope
    pub fn byte_len(&self) -> usize {
        self.root.as_ref().map_or(0, |node| node.byte_len())
    }
    
    /// Returns the character length of the rope
    pub fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |node| node.char_len())
    }
    
    /// Checks if the rope is empty
    pub fn is_empty(&self) -> bool {
        self.root.is_none() || self.byte_len() == 0
    }
    
    /// Inserts text at the given character position
    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        
        let insert_rope = Rope::from_str(text);
        
        if self.is_empty() {
            *self = insert_rope;
            return;
        }
        
        if pos == 0 {
            *self = insert_rope.append(self);
            return;
        }
        
        if pos >= self.len() {
            *self = self.append(&insert_rope);
            return;
        }
        
        // Split at position and insert
        let (left, right) = self.split(pos);
        *self = left.append(&insert_rope).append(&right);
    }
    
    /// Removes text from start to end character positions
    pub fn remove(&mut self, start: usize, end: usize) {
        if start >= end || start >= self.len() {
            return;
        }
        
        let actual_end = end.min(self.len());
        
        if start == 0 && actual_end >= self.len() {
            *self = Rope::new();
            return;
        }
        
        if start == 0 {
            let (_, right) = self.split(actual_end);
            *self = right;
            return;
        }
        
        if actual_end >= self.len() {
            let (left, _) = self.split(start);
            *self = left;
            return;
        }
        
        // Remove middle section
        let (left, temp) = self.split(start);
        let (_, right) = temp.split(actual_end - start);
        *self = left.append(&right);
    }
    
    /// Concatenates two ropes
    pub fn append(&self, other: &Rope) -> Rope {
        match (&self.root, &other.root) {
            (None, None) => Rope::new(),
            (Some(_left), None) => self.clone(),
            (None, Some(_right)) => other.clone(),
            (Some(left), Some(right)) => {
                let node = RopeNode::new_branch(left.clone(), right.clone());
                let balanced_node = node.merge_if_needed();
                Rope { root: Some(Box::new(balanced_node)) }
            }
        }
    }
    
    /// Splits the rope at the given character position
    pub fn split(&self, pos: usize) -> (Rope, Rope) {
        if pos == 0 {
            return (Rope::new(), self.clone());
        }
        
        if pos >= self.len() {
            return (self.clone(), Rope::new());
        }
        
        match &self.root {
            None => (Rope::new(), Rope::new()),
            Some(root) => {
                let (left_node, right_node) = Self::split_node(root, pos);
                let left_rope = left_node.map(|n| Rope { root: Some(Box::new(n)) })
                    .unwrap_or_else(Rope::new);
                let right_rope = right_node.map(|n| Rope { root: Some(Box::new(n)) })
                    .unwrap_or_else(Rope::new);
                (left_rope, right_rope)
            }
        }
    }
    
    /// Recursively splits a node at the given position
    fn split_node(node: &RopeNode, pos: usize) -> (Option<RopeNode>, Option<RopeNode>) {
        match node {
            RopeNode::Leaf { text, .. } => {
                if pos == 0 {
                    (None, Some(node.clone()))
                } else if pos >= text.char_count() {
                    (Some(node.clone()), None)
                } else {
                    // Find byte position for character position
                    let mut byte_pos = 0;
                    let mut char_idx = 0;
                    
                    for (byte_idx, _) in text.as_str().char_indices() {
                        if char_idx == pos {
                            byte_pos = byte_idx;
                            break;
                        }
                        char_idx += 1;
                    }
                    
                    let left_text = String::from(&text.as_str()[..byte_pos]);
                    let right_text = String::from(&text.as_str()[byte_pos..]);
                    
                    let left = if left_text.is_empty() { None } else { Some(RopeNode::new_leaf(left_text)) };
                    let right = if right_text.is_empty() { None } else { Some(RopeNode::new_leaf(right_text)) };
                    
                    (left, right)
                }
            }
            RopeNode::Branch { left, right, .. } => {
                let left_len = left.char_len();
                
                if pos <= left_len {
                    let (left_left, left_right) = Self::split_node(left, pos);
                    
                    let new_left = left_left;
                    let new_right = match left_right {
                        Some(lr) => {
                            let new_branch = RopeNode::new_branch(Box::new(lr), right.clone());
                            Some(new_branch.merge_if_needed())
                        }
                        None => Some((**right).clone()),
                    };
                    
                    (new_left, new_right)
                } else {
                    let (right_left, right_right) = Self::split_node(right, pos - left_len);
                    
                    let new_left = match right_left {
                        Some(rl) => {
                            let new_branch = RopeNode::new_branch(left.clone(), Box::new(rl));
                            Some(new_branch.merge_if_needed())
                        }
                        None => Some((**left).clone()),
                    };
                    let new_right = right_right;
                    
                    (new_left, new_right)
                }
            }
        }
    }
    
    /// Returns a substring from start to end character positions
    pub fn slice(&self, start: usize, end: usize) -> Rope {
        if start >= end || start >= self.len() {
            return Rope::new();
        }
        
        let actual_end = end.min(self.len());
        let (_, temp) = self.split(start);
        let (result, _) = temp.split(actual_end - start);
        result
    }
    
    /// Gets the character at the given position
    pub fn char_at(&self, pos: usize) -> Option<char> {
        if pos >= self.len() {
            return None;
        }
        
        match &self.root {
            None => None,
            Some(root) => Self::char_at_node(root, pos),
        }
    }
    
    /// Recursively finds character at position in node
    fn char_at_node(node: &RopeNode, pos: usize) -> Option<char> {
        match node {
            RopeNode::Leaf { text, .. } => {
                text.as_str().chars().nth(pos)
            }
            RopeNode::Branch { left, right, .. } => {
                let left_len = left.char_len();
                if pos < left_len {
                    Self::char_at_node(left, pos)
                } else {
                    Self::char_at_node(right, pos - left_len)
                }
            }
        }
    }
    
    /// Converts the rope to a string
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        self.collect_string(&mut result);
        result
    }
    
    /// Helper to collect string from rope
    fn collect_string(&self, result: &mut String) {
        match &self.root {
            None => {}
            Some(root) => Self::collect_string_from_node(root, result),
        }
    }
    
    /// Recursively collects string from node
    fn collect_string_from_node(node: &RopeNode, result: &mut String) {
        match node {
            RopeNode::Leaf { text, .. } => {
                result.push_str(text.as_str());
            }
            RopeNode::Branch { left, right, .. } => {
                Self::collect_string_from_node(left, result);
                Self::collect_string_from_node(right, result);
            }
        }
    }
    
    /// Creates an iterator over characters
    pub fn chars(&self) -> RopeCharIterator {
        RopeCharIterator::new(self)
    }
    
    /// Creates an iterator over lines
    pub fn lines(&self) -> RopeLineIterator {
        RopeLineIterator::new(self)
    }
    
    /// Finds the first occurrence of a substring
    pub fn find(&self, pattern: &str) -> Option<usize> {
        let text = self.to_string();
        text.as_str().find(pattern).map(|byte_pos| {
            // Convert byte position to character position
            text.as_str()[..byte_pos].chars().count()
        })
    }
    
    /// Replaces all occurrences of a pattern
    pub fn replace_all(&self, pattern: &str, replacement: &str) -> Rope {
        let text = self.to_string();
        let replaced = text.replace(pattern, replacement);
        Rope::from_string(String::from(&replaced))
    }
    
    /// Returns memory usage statistics
    pub fn memory_usage(&self) -> RopeMemoryStats {
        let mut stats = RopeMemoryStats::default();
        if let Some(root) = &self.root {
            Self::collect_memory_stats(root, &mut stats);
        }
        stats
    }
    
    /// Recursively collects memory statistics
    fn collect_memory_stats(node: &RopeNode, stats: &mut RopeMemoryStats) {
        match node {
            RopeNode::Leaf { text, .. } => {
                stats.leaf_count += 1;
                stats.leaf_bytes += text.len();
                stats.total_bytes += std::mem::size_of::<RopeNode>() + text.len();
            }
            RopeNode::Branch { left, right, .. } => {
                stats.branch_count += 1;
                stats.total_bytes += std::mem::size_of::<RopeNode>();
                Self::collect_memory_stats(left, stats);
                Self::collect_memory_stats(right, stats);
            }
        }
    }
}

/// Memory usage statistics for a rope
#[derive(Debug, Default)]
pub struct RopeMemoryStats {
    pub leaf_count: usize,
    pub branch_count: usize,
    pub leaf_bytes: usize,
    pub total_bytes: usize,
}

/// Iterator over characters in a rope
pub struct RopeCharIterator<'a> {
    rope: &'a Rope,
    position: usize,
}

impl<'a> RopeCharIterator<'a> {
    fn new(rope: &'a Rope) -> Self {
        RopeCharIterator { rope, position: 0 }
    }
}

impl<'a> Iterator for RopeCharIterator<'a> {
    type Item = char;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.rope.len() {
            None
        } else {
            let ch = self.rope.char_at(self.position);
            self.position += 1;
            ch
        }
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rope.len() - self.position;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for RopeCharIterator<'a> {}

/// Iterator over lines in a rope
pub struct RopeLineIterator<'a> {
    rope: &'a Rope,
    position: usize,
}

impl<'a> RopeLineIterator<'a> {
    fn new(rope: &'a Rope) -> Self {
        RopeLineIterator { rope, position: 0 }
    }
}

impl<'a> Iterator for RopeLineIterator<'a> {
    type Item = Rope;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.rope.len() {
            return None;
        }
        
        // Find next newline
        let mut end = self.position;
        while end < self.rope.len() {
            if let Some('\n') = self.rope.char_at(end) {
                break;
            }
            end += 1;
        }
        
        let line = self.rope.slice(self.position, end);
        self.position = if end < self.rope.len() { end + 1 } else { self.rope.len() };
        Some(line)
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string().as_str())
    }
}

impl fmt::Debug for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stats = self.memory_usage();
        write!(f, "Rope(len={}, leaves={}, branches={}, bytes={})", 
               self.len(), stats.leaf_count, stats.branch_count, stats.total_bytes)
    }
}

impl PartialEq for Rope {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            false
        } else {
            self.to_string() == other.to_string()
        }
    }
}

impl Eq for Rope {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_creation() {
        let empty_rope = Rope::new();
        assert!(empty_rope.is_empty());
        assert_eq!(empty_rope.len(), 0);
        assert_eq!(empty_rope.byte_len(), 0);

        let rope = Rope::from_str("Hello, World!");
        assert!(!rope.is_empty());
        assert_eq!(rope.len(), 13);
        assert_eq!(rope.to_string().as_str(), "Hello, World!");

        let string_rope = Rope::from_string(String::from("Test string"));
        assert_eq!(string_rope.len(), 11);
        assert_eq!(string_rope.to_string().as_str(), "Test string");
    }

    #[test]
    fn test_rope_insertion() {
        let mut rope = Rope::new();
        
        // Insert into empty rope
        rope.insert(0, "Hello");
        assert_eq!(rope.to_string().as_str(), "Hello");
        assert_eq!(rope.len(), 5);

        // Insert at beginning
        rope.insert(0, "Hi, ");
        assert_eq!(rope.to_string().as_str(), "Hi, Hello");

        // Insert at end
        rope.insert(rope.len(), "!");
        assert_eq!(rope.to_string().as_str(), "Hi, Hello!");

        // Insert in middle
        rope.insert(4, "there ");
        assert_eq!(rope.to_string().as_str(), "Hi, there Hello!");
    }

    #[test]
    fn test_rope_splitting() {
        let rope = Rope::from_str("Hello, World!");
        
        let (left, right) = rope.split(7);
        assert_eq!(left.to_string().as_str(), "Hello, ");
        assert_eq!(right.to_string().as_str(), "World!");

        // Split at beginning
        let (left, right) = rope.split(0);
        assert!(left.is_empty());
        assert_eq!(right.to_string().as_str(), "Hello, World!");

        // Split at end
        let (left, right) = rope.split(rope.len());
        assert_eq!(left.to_string().as_str(), "Hello, World!");
        assert!(right.is_empty());
    }

    #[test]
    fn test_rope_append() {
        let rope1 = Rope::from_str("Hello, ");
        let rope2 = Rope::from_str("World!");
        
        let combined = rope1.append(&rope2);
        assert_eq!(combined.to_string().as_str(), "Hello, World!");
        
        // Append to empty rope
        let empty = Rope::new();
        let result = empty.append(&rope1);
        assert_eq!(result.to_string().as_str(), "Hello, ");
    }

    #[test]
    fn test_rope_char_access() {
        let rope = Rope::from_str("Hello, 世界!");
        
        assert_eq!(rope.char_at(0), Some('H'));
        assert_eq!(rope.char_at(7), Some('世'));
        assert_eq!(rope.char_at(8), Some('界'));
        assert_eq!(rope.char_at(9), Some('!'));
        assert_eq!(rope.char_at(10), None);
    }

    #[test]
    fn test_rope_large_operations() {
        let chunk = "The quick brown fox jumps over the lazy dog. ";
        let large_text = chunk.repeat(100);
        let rope = Rope::from_str(&large_text);
        
        assert_eq!(rope.len(), large_text.chars().count());
        
        // Test operations on large rope
        let mut rope = rope;
        rope.insert(1000, "INSERTED ");
        assert!(rope.to_string().contains("INSERTED"));
    }
}