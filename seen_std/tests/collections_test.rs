//! Tests for high-performance collections

use seen_std::collections::{Vec, HashMap, HashSet};

#[test]
fn test_vec_basic_operations() {
    let mut vec = Vec::new();
    assert_eq!(vec.len(), 0);
    assert!(vec.is_empty());
    
    vec.push(10);
    vec.push(20);
    vec.push(30);
    
    assert_eq!(vec.len(), 3);
    assert!(!vec.is_empty());
    assert_eq!(vec[0], 10);
    assert_eq!(vec[1], 20);
    assert_eq!(vec[2], 30);
    
    assert_eq!(vec.pop(), Some(30));
    assert_eq!(vec.len(), 2);
}

#[test]
fn test_vec_capacity() {
    let mut vec = Vec::with_capacity(10);
    assert_eq!(vec.capacity(), 10);
    assert_eq!(vec.len(), 0);
    
    for i in 0..10 {
        vec.push(i);
    }
    assert_eq!(vec.capacity(), 10);
    
    vec.push(10); // Should trigger growth
    assert!(vec.capacity() > 10);
}

#[test]
fn test_vec_insert_remove() {
    let mut vec = Vec::new();
    vec.push(1);
    vec.push(3);
    vec.push(4);
    
    vec.insert(1, 2);
    assert_eq!(vec.as_slice(), &[1, 2, 3, 4]);
    
    let removed = vec.remove(2);
    assert_eq!(removed, 3);
    assert_eq!(vec.as_slice(), &[1, 2, 4]);
}

#[test]
fn test_vec_extend() {
    let mut vec = Vec::new();
    vec.extend_from_slice(&[1, 2, 3]);
    assert_eq!(vec.as_slice(), &[1, 2, 3]);
    
    vec.extend_from_slice(&[4, 5, 6]);
    assert_eq!(vec.as_slice(), &[1, 2, 3, 4, 5, 6]);
}

#[test]
fn test_vec_iteration() {
    let mut vec = Vec::new();
    for i in 0..5 {
        vec.push(i);
    }
    
    let mut sum = 0;
    for &item in vec.as_slice() {
        sum += item;
    }
    assert_eq!(sum, 10);
    
    // Test into_iter
    let vec2 = vec.clone();
    let collected: std::vec::Vec<_> = vec2.into_iter().collect();
    assert_eq!(collected, std::vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_vec_clone() {
    let mut vec1 = Vec::new();
    vec1.push(1);
    vec1.push(2);
    vec1.push(3);
    
    let vec2 = vec1.clone();
    assert_eq!(vec1.as_slice(), vec2.as_slice());
    
    // Ensure they're independent
    vec1.push(4);
    assert_eq!(vec1.len(), 4);
    assert_eq!(vec2.len(), 3);
}

#[test]
fn test_hashmap_basic_operations() {
    let mut map = HashMap::new();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    
    map.insert("key1", 100);
    map.insert("key2", 200);
    map.insert("key3", 300);
    
    assert_eq!(map.len(), 3);
    assert!(!map.is_empty());
    
    assert_eq!(map.get("key1"), Some(&100));
    assert_eq!(map.get("key2"), Some(&200));
    assert_eq!(map.get("key3"), Some(&300));
    assert_eq!(map.get("key4"), None);
}

#[test]
fn test_hashmap_update() {
    let mut map = HashMap::new();
    map.insert("key", 100);
    assert_eq!(map.get("key"), Some(&100));
    
    map.insert("key", 200); // Update existing key
    assert_eq!(map.get("key"), Some(&200));
    assert_eq!(map.len(), 1);
}

#[test]
fn test_hashmap_remove() {
    let mut map = HashMap::new();
    map.insert("key1", 100);
    map.insert("key2", 200);
    
    assert_eq!(map.remove("key1"), Some(100));
    assert_eq!(map.len(), 1);
    assert_eq!(map.get("key1"), None);
    assert_eq!(map.get("key2"), Some(&200));
}

#[test]
fn test_hashmap_capacity() {
    let mut map = HashMap::with_capacity(100);
    assert!(map.capacity() >= 100);
    
    for i in 0..50 {
        map.insert(i, i * 10);
    }
    
    assert_eq!(map.len(), 50);
    assert!(map.capacity() >= 100);
}

#[test]
fn test_hashmap_iteration() {
    let mut map = HashMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    map.insert("c", 3);
    
    let mut sum = 0;
    for (_, &value) in map.iter() {
        sum += value;
    }
    assert_eq!(sum, 6);
    
    let mut keys: std::vec::Vec<_> = map.keys().collect();
    keys.sort();
    assert_eq!(keys, vec![&"a", &"b", &"c"]);
}

#[test]
fn test_hashset_basic_operations() {
    let mut set = HashSet::new();
    assert!(set.is_empty());
    
    assert!(set.insert(10));
    assert!(set.insert(20));
    assert!(set.insert(30));
    assert!(!set.insert(10)); // Duplicate
    
    assert_eq!(set.len(), 3);
    assert!(set.contains(&10));
    assert!(set.contains(&20));
    assert!(set.contains(&30));
    assert!(!set.contains(&40));
}

#[test]
fn test_hashset_remove() {
    let mut set = HashSet::new();
    set.insert(10);
    set.insert(20);
    
    assert!(set.remove(&10));
    assert!(!set.remove(&10)); // Already removed
    assert_eq!(set.len(), 1);
    assert!(!set.contains(&10));
    assert!(set.contains(&20));
}

#[test]
fn test_hashset_operations() {
    let mut set1 = HashSet::new();
    set1.insert(1);
    set1.insert(2);
    set1.insert(3);
    
    let mut set2 = HashSet::new();
    set2.insert(2);
    set2.insert(3);
    set2.insert(4);
    
    // Union
    let union: HashSet<_> = set1.union(&set2).collect();
    assert_eq!(union.len(), 4);
    
    // Intersection
    let intersection: HashSet<_> = set1.intersection(&set2).collect();
    assert_eq!(intersection.len(), 2);
    
    // Difference
    let difference: HashSet<_> = set1.difference(&set2).collect();
    assert_eq!(difference.len(), 1);
    assert!(difference.contains(&1));
}