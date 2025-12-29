//! IR Validation for the Seen Programming Language
//!
//! This module provides static validation of IR before execution or codegen.
//! It catches errors like:
//! - Undefined variables
//! - Type mismatches
//! - Invalid control flow (unreachable code, missing returns)
//! - Invalid array/struct accesses

use crate::instruction::{Instruction, BinaryOp, UnaryOp, BasicBlock};
use crate::function::IRFunction;
use crate::module::IRModule;
use crate::value::{IRValue, IRType};
use std::collections::{HashSet, HashMap};

/// Result of IR validation
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub kind: ValidationErrorKind,
    pub message: String,
    pub location: Option<ValidationLocation>,
}

/// Validation warning (non-fatal)
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
    pub location: Option<ValidationLocation>,
}

/// Location in IR for error reporting
#[derive(Debug, Clone)]
pub struct ValidationLocation {
    pub function_name: String,
    pub block_name: Option<String>,
    pub instruction_index: Option<usize>,
}

/// Categories of validation errors
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationErrorKind {
    UndefinedVariable,
    UndefinedFunction,
    UndefinedLabel,
    TypeMismatch,
    InvalidArrayAccess,
    InvalidFieldAccess,
    MissingReturn,
    UnreachableCode,
    InvalidInstruction,
    DuplicateDefinition,
    InvalidControlFlow,
    UnresolvedGeneric,
    ArgumentTypeMismatch,
    ReturnTypeMismatch,
}

/// Categories of validation warnings
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationWarningKind {
    UnusedVariable,
    UnusedFunction,
    DeadCode,
    PossibleNullDereference,
    ImplicitConversion,
}

/// The IR validator
pub struct IRValidator {
    /// Track defined variables per function
    defined_vars: HashSet<String>,
    /// Track used variables per function
    used_vars: HashSet<String>,
    /// Track variable types for type checking
    var_types: HashMap<String, IRType>,
    /// Track register types for type checking
    reg_types: HashMap<u32, IRType>,
    /// Track defined labels per function
    defined_labels: HashSet<String>,
    /// Track used labels per function
    used_labels: HashSet<String>,
    /// Current function name for error reporting
    current_function: String,
    /// Current block name for error reporting
    current_block: Option<String>,
    /// Expected return type for current function
    expected_return_type: Option<IRType>,
    /// Whether to perform strict type checking
    strict_type_checking: bool,
}

impl IRValidator {
    pub fn new() -> Self {
        Self {
            defined_vars: HashSet::new(),
            used_vars: HashSet::new(),
            var_types: HashMap::new(),
            reg_types: HashMap::new(),
            defined_labels: HashSet::new(),
            used_labels: HashSet::new(),
            current_function: String::new(),
            current_block: None,
            expected_return_type: None,
            strict_type_checking: false,
        }
    }
    
    /// Create a validator with strict type checking enabled
    pub fn strict() -> Self {
        let mut v = Self::new();
        v.strict_type_checking = true;
        v
    }

    /// Validate an entire module
    pub fn validate_module(&mut self, module: &IRModule) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Collect all function names for call validation
        let function_names: HashSet<String> = module.functions_iter()
            .map(|f| f.name.clone())
            .collect();

        // Validate each function
        for function in module.functions_iter() {
            let func_result = self.validate_function(function, &function_names);
            result.merge(func_result);
        }

        result
    }

    /// Validate an entire program (multiple modules)
    pub fn validate_program(&mut self, program: &crate::IRProgram) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Collect all function names across all modules
        let mut all_function_names: HashSet<String> = HashSet::new();
        for module in program.modules.iter() {
            for func in module.functions_iter() {
                all_function_names.insert(func.name.clone());
            }
        }

        // Validate each module
        for module in program.modules.iter() {
            let module_result = self.validate_module(module);
            result.merge(module_result);
        }

        // Check that entry point exists if specified
        if let Some(entry) = &program.entry_point {
            if !all_function_names.contains(entry) {
                result.add_error(ValidationError {
                    kind: ValidationErrorKind::UndefinedFunction,
                    message: format!("Entry point function '{}' not found", entry),
                    location: None,
                });
            }
        }

        result
    }

    /// Validate a single function
    pub fn validate_function(&mut self, function: &IRFunction, known_functions: &HashSet<String>) -> ValidationResult {
        let mut result = ValidationResult::new();
        
        // Reset state for this function
        self.defined_vars.clear();
        self.used_vars.clear();
        self.var_types.clear();
        self.reg_types.clear();
        self.defined_labels.clear();
        self.used_labels.clear();
        self.current_function = function.name.clone();
        self.expected_return_type = Some(function.return_type.clone());

        // Add parameters as defined variables with their types
        for param in &function.parameters {
            self.defined_vars.insert(param.name.clone());
            self.var_types.insert(param.name.clone(), param.param_type.clone());
        }
        
        // Check for unresolved generic types in parameters
        for param in &function.parameters {
            if self.contains_unresolved_generic(&param.param_type) {
                result.add_error(ValidationError {
                    kind: ValidationErrorKind::UnresolvedGeneric,
                    message: format!(
                        "Unresolved generic type in parameter '{}': {:?}",
                        param.name, param.param_type
                    ),
                    location: Some(ValidationLocation {
                        function_name: function.name.clone(),
                        block_name: None,
                        instruction_index: None,
                    }),
                });
            }
        }
        
        // Check for unresolved generic in return type
        if self.contains_unresolved_generic(&function.return_type) {
            result.add_error(ValidationError {
                kind: ValidationErrorKind::UnresolvedGeneric,
                message: format!(
                    "Unresolved generic type in return type: {:?}",
                    function.return_type
                ),
                location: Some(ValidationLocation {
                    function_name: function.name.clone(),
                    block_name: None,
                    instruction_index: None,
                }),
            });
        }

        // First pass: collect all labels
        for block_name in &function.cfg.block_order {
            if let Some(block) = function.cfg.get_block(block_name) {
                self.defined_labels.insert(block.label.0.clone());
            }
        }

        // Second pass: validate instructions
        for block_name in &function.cfg.block_order {
            if let Some(block) = function.cfg.get_block(block_name) {
                self.current_block = Some(block.label.0.clone());
                
                for (idx, instruction) in block.instructions.iter().enumerate() {
                    if let Some(err) = self.validate_instruction(instruction, known_functions, idx) {
                        result.add_error(err);
                    }
                }

                // Validate terminator
                if let Some(ref term) = block.terminator {
                    if let Some(err) = self.validate_instruction(term, known_functions, block.instructions.len()) {
                        result.add_error(err);
                    }
                }
            }
        }

        // Check for undefined label usage
        for label in &self.used_labels {
            if !self.defined_labels.contains(label) {
                result.add_error(ValidationError {
                    kind: ValidationErrorKind::UndefinedLabel,
                    message: format!("Undefined label: {}", label),
                    location: Some(ValidationLocation {
                        function_name: self.current_function.clone(),
                        block_name: None,
                        instruction_index: None,
                    }),
                });
            }
        }

        // Check for unused variables (warning)
        for var in &self.defined_vars {
            if !self.used_vars.contains(var) && !var.starts_with('_') {
                result.add_warning(ValidationWarning {
                    kind: ValidationWarningKind::UnusedVariable,
                    message: format!("Unused variable: {}", var),
                    location: Some(ValidationLocation {
                        function_name: self.current_function.clone(),
                        block_name: None,
                        instruction_index: None,
                    }),
                });
            }
        }

        result
    }

    /// Validate a single instruction
    fn validate_instruction(
        &mut self,
        instruction: &Instruction,
        known_functions: &HashSet<String>,
        idx: usize,
    ) -> Option<ValidationError> {
        match instruction {
            Instruction::Label(_) => None,

            Instruction::Jump(label) => {
                self.used_labels.insert(label.0.clone());
                None
            }

            Instruction::JumpIf { condition, target } |
            Instruction::JumpIfNot { condition, target } => {
                self.check_value_defined(condition);
                self.used_labels.insert(target.0.clone());
                None
            }

            Instruction::Return(value) => {
                if let Some(v) = value {
                    self.check_value_defined(v);
                    
                    // Type check return value if strict mode is on
                    if self.strict_type_checking {
                        if let Some(ref expected_type) = self.expected_return_type {
                            if let Some(actual_type) = self.get_value_type(v) {
                                if !self.types_compatible(expected_type, &actual_type) {
                                    return Some(ValidationError {
                                        kind: ValidationErrorKind::ReturnTypeMismatch,
                                        message: format!(
                                            "Return type mismatch: expected {:?}, got {:?}",
                                            expected_type, actual_type
                                        ),
                                        location: self.make_location(idx),
                                    });
                                }
                            }
                        }
                    }
                } else if self.strict_type_checking {
                    // Returning nothing when function expects a value
                    if let Some(ref expected_type) = self.expected_return_type {
                        if !matches!(expected_type, IRType::Void) {
                            return Some(ValidationError {
                                kind: ValidationErrorKind::ReturnTypeMismatch,
                                message: format!(
                                    "Function expects return type {:?}, but returns nothing",
                                    expected_type
                                ),
                                location: self.make_location(idx),
                            });
                        }
                    }
                }
                None
            }

            Instruction::Load { source, dest } => {
                self.check_value_defined(source);
                self.define_value(dest);
                None
            }

            Instruction::Store { value, dest } => {
                self.check_value_defined(value);
                self.check_value_defined(dest);
                None
            }

            Instruction::Move { source, dest } => {
                self.check_value_defined(source);
                self.define_value(dest);
                None
            }

            Instruction::Binary { op, left, right, result } => {
                self.check_value_defined(left);
                self.check_value_defined(right);
                self.define_value(result);
                
                // Type-check operands if strict type checking is enabled
                if self.strict_type_checking {
                    let left_type = self.get_value_type(left);
                    let right_type = self.get_value_type(right);
                    
                    if let (Some(lt), Some(rt)) = (&left_type, &right_type) {
                        if !self.types_compatible(lt, rt) {
                            return Some(ValidationError {
                                kind: ValidationErrorKind::TypeMismatch,
                                message: format!(
                                    "Binary operation {:?}: type mismatch between {:?} and {:?}",
                                    op, lt, rt
                                ),
                                location: self.make_location(idx),
                            });
                        }
                    }
                }
                
                None
            }

            Instruction::Unary { op: _, operand, result } => {
                self.check_value_defined(operand);
                self.define_value(result);
                None
            }

            Instruction::Allocate { size, result } => {
                self.check_value_defined(size);
                self.define_value(result);
                None
            }

            Instruction::Deallocate { pointer } => {
                self.check_value_defined(pointer);
                None
            }

            Instruction::ArrayAccess { array, index, result, .. } => {
                self.check_value_defined(array);
                self.check_value_defined(index);
                self.define_value(result);
                None
            }

            Instruction::ArraySet { array, index, value, .. } => {
                self.check_value_defined(array);
                self.check_value_defined(index);
                self.check_value_defined(value);
                None
            }

            Instruction::ArrayLength { array, result } => {
                self.check_value_defined(array);
                self.define_value(result);
                None
            }

            Instruction::FieldAccess { struct_val, field: _, result, .. } => {
                self.check_value_defined(struct_val);
                self.define_value(result);
                None
            }

            Instruction::FieldSet { struct_val, field: _, value, .. } => {
                self.check_value_defined(struct_val);
                self.check_value_defined(value);
                None
            }

            Instruction::Call { target, args, result, arg_types, return_type } => {
                // Check if function is known
                if let IRValue::Function { name, .. } = target {
                    if !known_functions.contains(name) && !self.is_builtin(name) {
                        return Some(ValidationError {
                            kind: ValidationErrorKind::UndefinedFunction,
                            message: format!("Undefined function: {}", name),
                            location: self.make_location(idx),
                        });
                    }
                }
                for arg in args {
                    self.check_value_defined(arg);
                }
                
                // Type check arguments if arg_types are provided and strict mode is on
                if self.strict_type_checking {
                    if let Some(ref expected_types) = arg_types {
                        if args.len() != expected_types.len() {
                            return Some(ValidationError {
                                kind: ValidationErrorKind::ArgumentTypeMismatch,
                                message: format!(
                                    "Expected {} arguments, got {}",
                                    expected_types.len(), args.len()
                                ),
                                location: self.make_location(idx),
                            });
                        }
                        
                        for (i, (arg, expected_type)) in args.iter().zip(expected_types.iter()).enumerate() {
                            if let Some(actual_type) = self.get_value_type(arg) {
                                if !self.types_compatible(expected_type, &actual_type) {
                                    return Some(ValidationError {
                                        kind: ValidationErrorKind::ArgumentTypeMismatch,
                                        message: format!(
                                            "Argument {} has type {:?}, expected {:?}",
                                            i, actual_type, expected_type
                                        ),
                                        location: self.make_location(idx),
                                    });
                                }
                            }
                        }
                    }
                }
                
                // Define result with return type if provided
                if let Some(res) = result {
                    self.define_value(res);
                    if let Some(ref ret_type) = return_type {
                        self.define_value_with_type(res, ret_type.clone());
                    }
                }
                None
            }

            Instruction::VirtualCall { receiver, method_name: _, args, result, .. } => {
                self.check_value_defined(receiver);
                for arg in args {
                    self.check_value_defined(arg);
                }
                if let Some(res) = result {
                    self.define_value(res);
                }
                None
            }

            Instruction::StaticCall { class_name: _, method_name: _, args, result, .. } => {
                for arg in args {
                    self.check_value_defined(arg);
                }
                if let Some(res) = result {
                    self.define_value(res);
                }
                None
            }

            Instruction::Print(value) => {
                self.check_value_defined(value);
                None
            }

            Instruction::Debug { message: _, value } => {
                if let Some(v) = value {
                    self.check_value_defined(v);
                }
                None
            }

            Instruction::Cast { value, target_type: _, result } => {
                self.check_value_defined(value);
                self.define_value(result);
                None
            }

            Instruction::TypeCheck { value, target_type: _, result } => {
                self.check_value_defined(value);
                self.define_value(result);
                None
            }

            Instruction::StringConcat { left, right, result } => {
                self.check_value_defined(left);
                self.check_value_defined(right);
                self.define_value(result);
                None
            }

            Instruction::StringLength { string, result } => {
                self.check_value_defined(string);
                self.define_value(result);
                None
            }

            Instruction::ConstructObject { class_name: _, args, result, .. } => {
                for arg in args {
                    self.check_value_defined(arg);
                }
                self.define_value(result);
                None
            }

            Instruction::ConstructEnum { enum_name: _, variant_name: _, fields, result, .. } => {
                for field in fields {
                    self.check_value_defined(field);
                }
                self.define_value(result);
                None
            }

            Instruction::GetEnumTag { enum_value, result } => {
                self.check_value_defined(enum_value);
                self.define_value(result);
                None
            }

            Instruction::GetEnumField { enum_value, field_index: _, result } => {
                self.check_value_defined(enum_value);
                self.define_value(result);
                None
            }

            // Frame management (no validation needed)
            Instruction::PushFrame | Instruction::PopFrame | Instruction::Nop => None,

            // SIMD operations
            Instruction::SimdSplat { scalar, lane_type: _, lanes: _, result } => {
                self.check_value_defined(scalar);
                self.define_value(result);
                None
            }

            Instruction::SimdReduceAdd { vector, lane_type: _, result } => {
                self.check_value_defined(vector);
                self.define_value(result);
                None
            }

            // Concurrency (limited validation)
            Instruction::Scoped { kind: _, body: _, result } => {
                self.define_value(result);
                None
            }

            Instruction::Spawn { body: _, detached: _, result } => {
                self.define_value(result);
                None
            }

            Instruction::ChannelSelect { cases, payload_result, index_result, status_result } => {
                for case in cases {
                    self.check_value_defined(&case.channel);
                }
                self.define_value(payload_result);
                self.define_value(index_result);
                self.define_value(status_result);
                None
            }
        }
    }

    /// Check if a value is defined (if it's a variable)
    fn check_value_defined(&mut self, value: &IRValue) {
        match value {
            IRValue::Variable(name) => {
                self.used_vars.insert(name.clone());
            }
            IRValue::Register(n) => {
                let name = format!("_r{}", n);
                self.used_vars.insert(name);
            }
            _ => {}
        }
    }

    /// Mark a value as defined
    fn define_value(&mut self, value: &IRValue) {
        match value {
            IRValue::Variable(name) => {
                self.defined_vars.insert(name.clone());
            }
            IRValue::Register(n) => {
                let name = format!("_r{}", n);
                self.defined_vars.insert(name);
            }
            _ => {}
        }
    }

    /// Check if a function name is a builtin
    fn is_builtin(&self, name: &str) -> bool {
        matches!(name, 
            "print" | "println" | "len" | "length" | 
            "malloc" | "alloc" | "free" |
            "push" | "pop" | "get" | "set" |
            "charAt" | "substring" | "concat" |
            "parseInt" | "parseFloat" | "toString"
        )
    }

    /// Create a location for error reporting
    fn make_location(&self, instruction_index: usize) -> Option<ValidationLocation> {
        Some(ValidationLocation {
            function_name: self.current_function.clone(),
            block_name: self.current_block.clone(),
            instruction_index: Some(instruction_index),
        })
    }
    
    /// Check if a type contains unresolved generic parameters (like "T")
    fn contains_unresolved_generic(&self, ir_type: &IRType) -> bool {
        match ir_type {
            IRType::Generic(name) => {
                // Single letter generics like "T", "U", "V" are unresolved
                name.len() == 1 && name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            }
            IRType::Array(inner) => self.contains_unresolved_generic(inner),
            IRType::Optional(inner) => self.contains_unresolved_generic(inner),
            IRType::Pointer(inner) => self.contains_unresolved_generic(inner),
            IRType::Reference(inner) => self.contains_unresolved_generic(inner),
            IRType::Function { parameters, return_type } => {
                parameters.iter().any(|p| self.contains_unresolved_generic(p)) ||
                self.contains_unresolved_generic(return_type)
            }
            IRType::Struct { fields, .. } => {
                fields.iter().any(|(_, t)| self.contains_unresolved_generic(t))
            }
            IRType::Vector { lane_type, .. } => self.contains_unresolved_generic(lane_type),
            IRType::Enum { variants, .. } => {
                variants.iter().any(|(_, fields)| {
                    fields.as_ref().map_or(false, |f| 
                        f.iter().any(|t| self.contains_unresolved_generic(t))
                    )
                })
            }
            // Non-generic types
            IRType::Void | IRType::Integer | IRType::Float | IRType::Boolean | 
            IRType::String | IRType::Char => false,
        }
    }
    
    /// Get the type of an IRValue if known
    fn get_value_type(&self, value: &IRValue) -> Option<IRType> {
        match value {
            IRValue::Integer(_) => Some(IRType::Integer),
            IRValue::Float(_) => Some(IRType::Float),
            IRValue::Boolean(_) => Some(IRType::Boolean),
            IRValue::String(_) | IRValue::StringConstant(_) => Some(IRType::String),
            IRValue::Char(_) => Some(IRType::Char),
            IRValue::Null => Some(IRType::Void),
            IRValue::Variable(name) => self.var_types.get(name).cloned(),
            IRValue::Register(r) => self.reg_types.get(r).cloned(),
            IRValue::Array(elements) => {
                // Infer element type from first element
                if let Some(first) = elements.first() {
                    if let Some(elem_type) = self.get_value_type(first) {
                        return Some(IRType::Array(Box::new(elem_type)));
                    }
                }
                Some(IRType::Array(Box::new(IRType::Void)))
            }
            _ => None,
        }
    }
    
    /// Track the type of a result value
    fn define_value_with_type(&mut self, value: &IRValue, ir_type: IRType) {
        self.define_value(value);
        match value {
            IRValue::Variable(name) => {
                self.var_types.insert(name.clone(), ir_type);
            }
            IRValue::Register(r) => {
                self.reg_types.insert(*r, ir_type);
            }
            _ => {}
        }
    }
    
    /// Check if two types are compatible (for assignment/passing)
    fn types_compatible(&self, expected: &IRType, actual: &IRType) -> bool {
        // Exact match
        if expected == actual {
            return true;
        }
        
        // Generic matches everything (for now - monomorphization should resolve this)
        if matches!(expected, IRType::Generic(_)) || matches!(actual, IRType::Generic(_)) {
            return true;
        }
        
        // Void is compatible with everything for optional returns
        if matches!(expected, IRType::Void) || matches!(actual, IRType::Void) {
            return true;
        }
        
        // Array compatibility (covariant)
        if let (IRType::Array(expected_elem), IRType::Array(actual_elem)) = (expected, actual) {
            return self.types_compatible(expected_elem, actual_elem);
        }
        
        // Optional compatibility
        if let (IRType::Optional(expected_inner), IRType::Optional(actual_inner)) = (expected, actual) {
            return self.types_compatible(expected_inner, actual_inner);
        }
        
        // T? is compatible with T (auto-wrap)
        if let IRType::Optional(inner) = expected {
            if self.types_compatible(inner, actual) {
                return true;
            }
        }
        
        false
    }
}

impl Default for IRValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = IRValidator::new();
        assert!(validator.defined_vars.is_empty());
    }

    #[test]
    fn test_empty_module_validation() {
        let mut validator = IRValidator::new();
        let module = IRModule::new("test");
        let result = validator.validate_module(&module);
        assert!(result.is_valid());
    }
}
