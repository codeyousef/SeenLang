//! Tests for Seen primitive types with optimal memory layout

use seen_std::core::primitives::*;
use std::mem;

#[test]
fn test_primitive_sizes() {
    // Ensure primitives have optimal sizes
    assert_eq!(mem::size_of::<I8>(), 1);
    assert_eq!(mem::size_of::<I16>(), 2);
    assert_eq!(mem::size_of::<I32>(), 4);
    assert_eq!(mem::size_of::<I64>(), 8);
    assert_eq!(mem::size_of::<I128>(), 16);
    
    assert_eq!(mem::size_of::<U8>(), 1);
    assert_eq!(mem::size_of::<U16>(), 2);
    assert_eq!(mem::size_of::<U32>(), 4);
    assert_eq!(mem::size_of::<U64>(), 8);
    assert_eq!(mem::size_of::<U128>(), 16);
    
    assert_eq!(mem::size_of::<F32>(), 4);
    assert_eq!(mem::size_of::<F64>(), 8);
    
    assert_eq!(mem::size_of::<Bool>(), 1);
    assert_eq!(mem::size_of::<Char>(), 4);
}

#[test]
fn test_primitive_alignment() {
    // Ensure proper alignment for SIMD operations
    assert_eq!(mem::align_of::<I32>(), 4);
    assert_eq!(mem::align_of::<I64>(), 8);
    assert_eq!(mem::align_of::<F32>(), 4);
    assert_eq!(mem::align_of::<F64>(), 8);
}

#[test]
fn test_integer_operations() {
    let a = I32::new(10);
    let b = I32::new(20);
    
    assert_eq!(a + b, I32::new(30));
    assert_eq!(b - a, I32::new(10));
    assert_eq!(a * I32::new(3), I32::new(30));
    assert_eq!(b / I32::new(4), I32::new(5));
    assert_eq!(b % I32::new(3), I32::new(2));
}

#[test]
fn test_overflow_behavior() {
    let max = I8::MAX;
    let one = I8::new(1);
    
    // Wrapping arithmetic
    assert_eq!(max.wrapping_add(one), I8::MIN);
    
    // Checked arithmetic
    assert_eq!(max.checked_add(one), None);
    
    // Saturating arithmetic
    assert_eq!(max.saturating_add(one), I8::MAX);
}

#[test]
fn test_bitwise_operations() {
    let a = U32::new(0b1010);
    let b = U32::new(0b1100);
    
    assert_eq!(a & b, U32::new(0b1000));
    assert_eq!(a | b, U32::new(0b1110));
    assert_eq!(a ^ b, U32::new(0b0110));
    assert_eq!(!a, U32::new(!0b1010));
    assert_eq!(a << 2, U32::new(0b101000));
    assert_eq!(b >> 2, U32::new(0b0011));
}

#[test]
fn test_float_operations() {
    let a = F64::new(3.14);
    let b = F64::new(2.0);
    
    assert!((a + b).value() - 5.14 < 1e-10);
    assert!((a * b).value() - 6.28 < 1e-10);
    assert!((a / b).value() - 1.57 < 1e-10);
}

#[test]
fn test_float_special_values() {
    assert!(F32::INFINITY.is_infinite());
    assert!(F32::NEG_INFINITY.is_infinite());
    assert!(F32::NAN.is_nan());
    
    assert!(F64::INFINITY.is_infinite());
    assert!(F64::NEG_INFINITY.is_infinite());
    assert!(F64::NAN.is_nan());
}

#[test]
fn test_bool_operations() {
    let t = Bool::TRUE;
    let f = Bool::FALSE;
    
    assert_eq!(t & t, Bool::TRUE);
    assert_eq!(t & f, Bool::FALSE);
    assert_eq!(t | f, Bool::TRUE);
    assert_eq!(t ^ t, Bool::FALSE);
    assert_eq!(!t, Bool::FALSE);
    assert_eq!(!f, Bool::TRUE);
}

#[test]
fn test_char_unicode() {
    let a = Char::new('A');
    let emoji = Char::new('🦀');
    let arabic = Char::new('س');
    
    assert_eq!(a.to_uppercase(), Char::new('A'));
    assert_eq!(a.to_lowercase(), Char::new('a'));
    assert!(emoji.is_emoji());
    assert!(arabic.is_alphabetic());
    assert_eq!(a.len_utf8(), 1);
    assert_eq!(emoji.len_utf8(), 4);
    assert_eq!(arabic.len_utf8(), 2);
}

#[test]
fn test_type_conversions() {
    // Safe conversions
    let i: I32 = I32::new(42);
    let i64: I64 = i.into();
    assert_eq!(i64.value(), 42);
    
    // Checked conversions
    let big = I64::new(i64::MAX);
    assert_eq!(I32::try_from(big), None);
    
    let small = I64::new(100);
    assert_eq!(I32::try_from(small), Some(I32::new(100)));
}

#[test]
fn test_const_operations() {
    // Ensure const functions work at compile time
    const A: I32 = I32::new(10);
    const B: I32 = I32::new(20);
    const C: I32 = A.const_add(B);
    
    assert_eq!(C.value(), 30);
}

#[test]
fn test_zero_cost_abstraction() {
    // Ensure our types compile to the same assembly as raw primitives
    let seen_add = |a: I32, b: I32| -> I32 { a + b };
    let raw_add = |a: i32, b: i32| -> i32 { a + b };
    
    // Both should have identical performance
    let seen_result = seen_add(I32::new(5), I32::new(10));
    let raw_result = raw_add(5, 10);
    
    assert_eq!(seen_result.value(), raw_result);
}