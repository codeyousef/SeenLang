//! Primitive types with optimal memory layout and zero-cost abstractions
//!
//! These types provide the same performance as raw primitives while
//! adding safety and convenience methods.

use std::ops::{Add, Sub, Mul, Div, Rem, BitAnd, BitOr, BitXor, Not, Shl, Shr};
use std::fmt::{self, Display, Debug};

// Macro to generate integer types with common operations
macro_rules! define_integer {
    ($name:ident, $inner:ty) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name($inner);

        impl $name {
            pub const MIN: Self = Self(<$inner>::MIN);
            pub const MAX: Self = Self(<$inner>::MAX);
            pub const BITS: u32 = <$inner>::BITS;
            
            #[inline(always)]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }
            
            #[inline(always)]
            pub const fn value(self) -> $inner {
                self.0
            }
            
            #[inline(always)]
            pub const fn const_add(self, other: Self) -> Self {
                Self(self.0.wrapping_add(other.0))
            }
            
            #[inline(always)]
            pub fn wrapping_add(self, other: Self) -> Self {
                Self(self.0.wrapping_add(other.0))
            }
            
            #[inline(always)]
            pub fn checked_add(self, other: Self) -> Option<Self> {
                self.0.checked_add(other.0).map(Self)
            }
            
            #[inline(always)]
            pub fn saturating_add(self, other: Self) -> Self {
                Self(self.0.saturating_add(other.0))
            }
            
            #[inline(always)]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }
            
            #[inline(always)]
            pub fn pow(self, exp: u32) -> Self {
                Self(self.0.pow(exp))
            }
        }

        impl Add for $name {
            type Output = Self;
            
            #[inline(always)]
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            
            #[inline(always)]
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl Mul for $name {
            type Output = Self;
            
            #[inline(always)]
            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl Div for $name {
            type Output = Self;
            
            #[inline(always)]
            fn div(self, rhs: Self) -> Self::Output {
                Self(self.0 / rhs.0)
            }
        }

        impl Rem for $name {
            type Output = Self;
            
            #[inline(always)]
            fn rem(self, rhs: Self) -> Self::Output {
                Self(self.0 % rhs.0)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
        
        impl From<$inner> for $name {
            #[inline(always)]
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }
        
        impl From<$name> for $inner {
            #[inline(always)]
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

// Macro for unsigned integers with bitwise operations
macro_rules! define_unsigned {
    ($name:ident, $inner:ty) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name($inner);

        impl $name {
            pub const MIN: Self = Self(<$inner>::MIN);
            pub const MAX: Self = Self(<$inner>::MAX);
            pub const BITS: u32 = <$inner>::BITS;
            
            #[inline(always)]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }
            
            #[inline(always)]
            pub const fn value(self) -> $inner {
                self.0
            }
            
            #[inline(always)]
            pub const fn const_add(self, other: Self) -> Self {
                Self(self.0.wrapping_add(other.0))
            }
            
            #[inline(always)]
            pub fn wrapping_add(self, other: Self) -> Self {
                Self(self.0.wrapping_add(other.0))
            }
            
            #[inline(always)]
            pub fn checked_add(self, other: Self) -> Option<Self> {
                self.0.checked_add(other.0).map(Self)
            }
            
            #[inline(always)]
            pub fn saturating_add(self, other: Self) -> Self {
                Self(self.0.saturating_add(other.0))
            }
            
            // Unsigned integers don't have abs since they're always positive
            
            #[inline(always)]
            pub fn pow(self, exp: u32) -> Self {
                Self(self.0.pow(exp))
            }
        }

        impl Add for $name {
            type Output = Self;
            
            #[inline(always)]
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            
            #[inline(always)]
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl Mul for $name {
            type Output = Self;
            
            #[inline(always)]
            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl Div for $name {
            type Output = Self;
            
            #[inline(always)]
            fn div(self, rhs: Self) -> Self::Output {
                Self(self.0 / rhs.0)
            }
        }

        impl Rem for $name {
            type Output = Self;
            
            #[inline(always)]
            fn rem(self, rhs: Self) -> Self::Output {
                Self(self.0 % rhs.0)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
        
        impl From<$inner> for $name {
            #[inline(always)]
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }
        
        impl From<$name> for $inner {
            #[inline(always)]
            fn from(value: $name) -> Self {
                value.0
            }
        }
        
        impl BitAnd for $name {
            type Output = Self;
            
            #[inline(always)]
            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl BitOr for $name {
            type Output = Self;
            
            #[inline(always)]
            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl BitXor for $name {
            type Output = Self;
            
            #[inline(always)]
            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.0 ^ rhs.0)
            }
        }

        impl Not for $name {
            type Output = Self;
            
            #[inline(always)]
            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl Shl<u32> for $name {
            type Output = Self;
            
            #[inline(always)]
            fn shl(self, rhs: u32) -> Self::Output {
                Self(self.0 << rhs)
            }
        }

        impl Shr<u32> for $name {
            type Output = Self;
            
            #[inline(always)]
            fn shr(self, rhs: u32) -> Self::Output {
                Self(self.0 >> rhs)
            }
        }
    };
}

// Define all integer types
define_integer!(I8, i8);
define_integer!(I16, i16);
define_integer!(I32, i32);
define_integer!(I64, i64);
define_integer!(I128, i128);

define_unsigned!(U8, u8);
define_unsigned!(U16, u16);
define_unsigned!(U32, u32);
define_unsigned!(U64, u64);
define_unsigned!(U128, u128);

// Floating point types
macro_rules! define_float {
    ($name:ident, $inner:ty) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, PartialEq, PartialOrd, Default)]
        pub struct $name($inner);

        impl $name {
            pub const INFINITY: Self = Self(<$inner>::INFINITY);
            pub const NEG_INFINITY: Self = Self(<$inner>::NEG_INFINITY);
            pub const NAN: Self = Self(<$inner>::NAN);
            pub const EPSILON: Self = Self(<$inner>::EPSILON);
            
            #[inline(always)]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }
            
            #[inline(always)]
            pub fn value(self) -> $inner {
                self.0
            }
            
            #[inline(always)]
            pub fn is_nan(self) -> bool {
                self.0.is_nan()
            }
            
            #[inline(always)]
            pub fn is_infinite(self) -> bool {
                self.0.is_infinite()
            }
            
            #[inline(always)]
            pub fn is_finite(self) -> bool {
                self.0.is_finite()
            }
            
            #[inline(always)]
            pub fn sqrt(self) -> Self {
                Self(self.0.sqrt())
            }
            
            #[inline(always)]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }
            
            #[inline(always)]
            pub fn sin(self) -> Self {
                Self(self.0.sin())
            }
            
            #[inline(always)]
            pub fn cos(self) -> Self {
                Self(self.0.cos())
            }
        }

        impl Add for $name {
            type Output = Self;
            
            #[inline(always)]
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            
            #[inline(always)]
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl Mul for $name {
            type Output = Self;
            
            #[inline(always)]
            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }

        impl Div for $name {
            type Output = Self;
            
            #[inline(always)]
            fn div(self, rhs: Self) -> Self::Output {
                Self(self.0 / rhs.0)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

define_float!(F32, f32);
define_float!(F64, f64);

// Boolean type
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Bool(bool);

impl Bool {
    pub const TRUE: Self = Self(true);
    pub const FALSE: Self = Self(false);
    
    #[inline(always)]
    pub const fn new(value: bool) -> Self {
        Self(value)
    }
    
    #[inline(always)]
    pub fn value(self) -> bool {
        self.0
    }
}

impl BitAnd for Bool {
    type Output = Self;
    
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitOr for Bool {
    type Output = Self;
    
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitXor for Bool {
    type Output = Self;
    
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl Not for Bool {
    type Output = Self;
    
    #[inline(always)]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

// Character type with Unicode support
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Char(char);

impl Char {
    #[inline(always)]
    pub const fn new(value: char) -> Self {
        Self(value)
    }
    
    #[inline(always)]
    pub fn value(self) -> char {
        self.0
    }
    
    #[inline(always)]
    pub fn is_alphabetic(self) -> bool {
        self.0.is_alphabetic()
    }
    
    #[inline(always)]
    pub fn is_numeric(self) -> bool {
        self.0.is_numeric()
    }
    
    #[inline(always)]
    pub fn is_whitespace(self) -> bool {
        self.0.is_whitespace()
    }
    
    #[inline(always)]
    pub fn is_emoji(self) -> bool {
        // Simplified emoji detection
        matches!(self.0 as u32, 0x1F300..=0x1F9FF)
    }
    
    #[inline(always)]
    pub fn to_uppercase(self) -> Self {
        // Simplified - returns first uppercase char
        Self(self.0.to_uppercase().next().unwrap_or(self.0))
    }
    
    #[inline(always)]
    pub fn to_lowercase(self) -> Self {
        // Simplified - returns first lowercase char
        Self(self.0.to_lowercase().next().unwrap_or(self.0))
    }
    
    #[inline(always)]
    pub fn len_utf8(self) -> usize {
        self.0.len_utf8()
    }
}

impl Display for Char {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

// Type conversions between integer sizes
impl From<I32> for I64 {
    #[inline(always)]
    fn from(value: I32) -> Self {
        I64::new(value.0 as i64)
    }
}

impl I32 {
    #[inline(always)]
    pub fn try_from(value: I64) -> Option<Self> {
        if value.0 >= i32::MIN as i64 && value.0 <= i32::MAX as i64 {
            Some(I32::new(value.0 as i32))
        } else {
            None
        }
    }
}

// Unit type
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Unit;

impl Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}