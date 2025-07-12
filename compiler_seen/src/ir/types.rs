// compiler_seen/src/ir/types.rs
// Defines the type system used within the Intermediate Representation.

// Based on ir_spec.md

/// Represents a type in the Seen IR.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IrType {
    // Primitive Types
    Void, // Represents the absence of a type, e.g., for functions not returning a value.
    Bool, // Boolean type
    I8,
    I16,
    I32,
    I64,
    I128, // Signed integers
    U8,
    U16,
    U32,
    U64,
    U128, // Unsigned integers
    F32,
    F64,  // Floating-point numbers
    Char, // Unicode character

    Ptr(Box<IrType>), // Pointer to another IrType (e.g., Ptr(Box::new(IrType::I32)) for an i32*)

    // Aggregate Types
    Array {
        element_type: Box<IrType>,
        size: u64, // Statically known size
    },
    Struct {
        name: Option<String>, // Optional name for the struct (for debugging/printing)
        fields: Vec<IrType>,  // Ordered list of field types
    },

    // Function Type
    Function {
        param_types: Vec<IrType>,
        return_type: Box<IrType>,
        // is_var_arg: bool, // For C-style varargs, if ever needed
    },

    // Special type for labels (basic blocks)
    Label,
}

impl IrType {
    /// Returns true if the type is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            IrType::I8
                | IrType::I16
                | IrType::I32
                | IrType::I64
                | IrType::I128
                | IrType::U8
                | IrType::U16
                | IrType::U32
                | IrType::U64
                | IrType::U128
        )
    }

    /// Returns true if the type is a floating-point type.
    pub fn is_float(&self) -> bool {
        matches!(self, IrType::F32 | IrType::F64)
    }

    /// Returns true if the type is a pointer type.
    pub fn is_pointer(&self) -> bool {
        matches!(self, IrType::Ptr(_))
    }
}

// TODO:
// - Methods for getting size and alignment of types (will depend on target architecture later).
// - Consider how to represent Seen's specific string type or other high-level types
//   if they need special handling in IR beyond a struct/pointer combination.
// - `isize` and `usize` might need to be represented as distinct types or resolved to fixed-size integers
//   once the target architecture is known for the IR module.
