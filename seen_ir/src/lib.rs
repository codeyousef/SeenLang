//! Intermediate representation for the Seen programming language
//!
//! This module provides the IR (Intermediate Representation) for the Seen language,
//! which serves as an intermediary between the AST and the target code generation.
//! The IR is designed to be platform-agnostic and suitable for optimization passes.

use crate::arena::{Arena, ArenaIndex};
use seen_support::{SeenError, SeenErrorKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// IR system modules
pub mod arena;
pub mod cfg_builder;
pub mod function;
pub mod generator;
pub mod instruction;
#[cfg(feature = "llvm")]
pub mod llvm_backend;
pub mod module;
pub mod optimizer;
pub mod value;

// Re-export main types
pub use function::{IRFunction, Parameter};
pub use generator::IRGenerator;
pub use instruction::{BasicBlock, Instruction, Label};
pub use module::IRModule;
pub use optimizer::{IROptimizer, OptimizationLevel};
pub use value::{IRType, IRValue};

/// Scheduler preference derived from the active hardware profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareSchedulerHint {
    Balanced,
    Throughput,
    Vector,
}

impl HardwareSchedulerHint {
    pub fn as_str(&self) -> &'static str {
        match self {
            HardwareSchedulerHint::Balanced => "balanced",
            HardwareSchedulerHint::Throughput => "throughput",
            HardwareSchedulerHint::Vector => "vector",
        }
    }
}

/// Hardware profile describing vector widths and ISA hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub cpu_features: Vec<String>,
    pub max_vector_bits: Option<u32>,
    pub apx_enabled: bool,
    pub sve_enabled: bool,
}

impl Default for HardwareProfile {
    fn default() -> Self {
        Self {
            cpu_features: Vec::new(),
            max_vector_bits: None,
            apx_enabled: false,
            sve_enabled: false,
        }
    }
}

impl HardwareProfile {
    /// Returns a scheduler hint that downstream backends can use to reorder blocks.
    pub fn scheduler_hint(&self) -> HardwareSchedulerHint {
        let vector_bits = self.max_vector_bits.unwrap_or(0);
        if self.sve_enabled || vector_bits >= 512 {
            HardwareSchedulerHint::Vector
        } else if self.apx_enabled || vector_bits >= 256 {
            HardwareSchedulerHint::Throughput
        } else {
            HardwareSchedulerHint::Balanced
        }
    }

    /// Compute a register budget hint based on the available ISA/feature set.
    pub fn register_budget_hint(&self) -> u32 {
        let mut budget = 64u32;
        if self.apx_enabled {
            budget += 32;
        }
        if self.sve_enabled {
            budget += 16;
        }
        if let Some(bits) = self.max_vector_bits {
            if bits >= 512 {
                budget += 16;
            } else if bits >= 256 {
                budget += 8;
            }
        }
        budget
    }

    /// True when the hardware profile carries any explicit feature overrides.
    pub fn has_explicit_features(&self) -> bool {
        self.apx_enabled
            || self.sve_enabled
            || self.max_vector_bits.is_some()
            || !self.cpu_features.is_empty()
    }
}

/// A complete IR program consisting of multiple modules
#[derive(Debug, Clone)]
pub struct IRProgram {
    pub modules: Arena<IRModule>,
    pub entry_point: Option<String>, // Function name to start execution
    pub global_variables: Arena<GlobalVariableEntry>,
    global_lookup: HashMap<String, ArenaIndex>,
    pub string_table: Vec<String>, // For string constants
    pub hardware_profile: HardwareProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalVariableEntry {
    pub name: String,
    pub value: IRValue,
}

impl IRProgram {
    pub fn new() -> Self {
        Self {
            modules: Arena::new(),
            entry_point: None,
            global_variables: Arena::new(),
            global_lookup: HashMap::new(),
            string_table: Vec::new(),
            hardware_profile: HardwareProfile::default(),
        }
    }

    pub fn add_module(&mut self, module: IRModule) {
        self.modules.push(module);
    }

    pub fn set_entry_point(&mut self, function_name: String) {
        self.entry_point = Some(function_name);
    }

    pub fn add_global(&mut self, name: String, value: IRValue) {
        if let Some(index) = self.global_lookup.get(&name).copied() {
            if let Some(entry) = self.global_variables.get_mut(index) {
                entry.value = value;
            }
        } else {
            let index = self.global_variables.push(GlobalVariableEntry {
                name: name.clone(),
                value,
            });
            self.global_lookup.insert(name, index);
        }
    }

    pub fn add_string_constant(&mut self, s: String) -> usize {
        let index = self.string_table.len();
        self.string_table.push(s);
        index
    }

    pub fn get_string(&self, index: usize) -> Option<&String> {
        self.string_table.get(index)
    }

    pub fn get_global(&self, name: &str) -> Option<&IRValue> {
        self.global_lookup
            .get(name)
            .and_then(|idx| self.global_variables.get(*idx))
            .map(|entry| &entry.value)
    }

    pub fn globals_iter(&self) -> impl Iterator<Item = &GlobalVariableEntry> {
        self.global_variables.iter()
    }

    pub fn has_global(&self, name: &str) -> bool {
        self.global_lookup.contains_key(name)
    }

    pub fn rebuild_global_lookup(&mut self) {
        self.global_lookup.clear();
        for (idx, entry) in self.global_variables.iter().enumerate() {
            self.global_lookup
                .insert(entry.name.clone(), ArenaIndex::from(idx));
        }
    }
}

impl Default for IRProgram {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct IRProgramSerde {
    modules: Vec<IRModule>,
    entry_point: Option<String>,
    global_variables: Vec<GlobalVariableEntry>,
    string_table: Vec<String>,
    #[serde(default)]
    hardware_profile: HardwareProfile,
}

impl Serialize for IRProgram {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IRProgramSerde {
            modules: self.modules.clone().into_vec(),
            entry_point: self.entry_point.clone(),
            global_variables: self.global_variables.clone().into_vec(),
            string_table: self.string_table.clone(),
            hardware_profile: self.hardware_profile.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IRProgram {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = IRProgramSerde::deserialize(deserializer)?;
        let mut program = IRProgram {
            modules: Arena::from(data.modules),
            entry_point: data.entry_point,
            global_variables: Arena::from(data.global_variables),
            global_lookup: HashMap::new(),
            string_table: data.string_table,
            hardware_profile: data.hardware_profile,
        };
        program.rebuild_global_lookup();
        Ok(program)
    }
}

impl fmt::Display for IRProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; Seen IR Program")?;

        if let Some(entry) = &self.entry_point {
            writeln!(f, "; Entry point: {}", entry)?;
        }

        if !self.string_table.is_empty() {
            writeln!(f, "\n; String Constants")?;
            for (i, s) in self.string_table.iter().enumerate() {
                writeln!(f, "@str.{} = \"{}\"", i, s.escape_default())?;
            }
        }

        if !self.global_variables.is_empty() {
            writeln!(f, "\n; Global Variables")?;
            let mut entries: Vec<&GlobalVariableEntry> = self.global_variables.iter().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for entry in entries {
                writeln!(f, "@{} = {}", entry.name, entry.value)?;
            }
        }

        for module in &self.modules {
            writeln!(f, "\n{}", module)?;
        }

        Ok(())
    }
}

/// Errors that can occur during IR generation
#[derive(Debug, Clone, PartialEq)]
pub enum IRError {
    UndefinedVariable(String),
    UndefinedFunction(String),
    TypeMismatch {
        expected: IRType,
        found: IRType,
    },
    InvalidOperation {
        operation: String,
        operand_types: Vec<IRType>,
    },
    InvalidJump(String),
    InvalidLabel(String),
    TooManyArguments {
        expected: usize,
        found: usize,
    },
    TooFewArguments {
        expected: usize,
        found: usize,
    },
    RecursionDepthExceeded,
    InvalidMemoryAccess,
    DivisionByZero,
    Other(String),
}

impl fmt::Display for IRError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRError::UndefinedVariable(name) => write!(f, "Undefined variable: {}", name),
            IRError::UndefinedFunction(name) => write!(f, "Undefined function: {}", name),
            IRError::TypeMismatch { expected, found } => {
                write!(
                    f,
                    "Type mismatch: expected {:?}, found {:?}",
                    expected, found
                )
            }
            IRError::InvalidOperation {
                operation,
                operand_types,
            } => {
                write!(
                    f,
                    "Invalid operation '{}' for types: {:?}",
                    operation, operand_types
                )
            }
            IRError::InvalidJump(label) => write!(f, "Invalid jump to label: {}", label),
            IRError::InvalidLabel(label) => write!(f, "Invalid label: {}", label),
            IRError::TooManyArguments { expected, found } => {
                write!(
                    f,
                    "Too many arguments: expected {}, found {}",
                    expected, found
                )
            }
            IRError::TooFewArguments { expected, found } => {
                write!(
                    f,
                    "Too few arguments: expected {}, found {}",
                    expected, found
                )
            }
            IRError::RecursionDepthExceeded => write!(f, "Recursion depth exceeded"),
            IRError::InvalidMemoryAccess => write!(f, "Invalid memory access"),
            IRError::DivisionByZero => write!(f, "Division by zero"),
            IRError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for IRError {}

pub type IRResult<T> = Result<T, IRError>;

impl From<IRError> for SeenError {
    fn from(error: IRError) -> Self {
        SeenError::new(SeenErrorKind::Ir, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_program_creation() {
        let program = IRProgram::new();
        assert!(program.modules.is_empty());
        assert!(program.entry_point.is_none());
        assert!(program.global_variables.is_empty());
        assert!(program.string_table.is_empty());
    }

    #[test]
    fn test_string_constant_management() {
        let mut program = IRProgram::new();
        let index1 = program.add_string_constant("Hello".to_string());
        let index2 = program.add_string_constant("World".to_string());

        assert_eq!(index1, 0);
        assert_eq!(index2, 1);
        assert_eq!(program.get_string(0), Some(&"Hello".to_string()));
        assert_eq!(program.get_string(1), Some(&"World".to_string()));
        assert_eq!(program.get_string(2), None);
    }

    #[test]
    fn test_ir_error_display() {
        let error = IRError::UndefinedVariable("x".to_string());
        assert_eq!(error.to_string(), "Undefined variable: x");

        let error = IRError::TypeMismatch {
            expected: IRType::Integer,
            found: IRType::String,
        };
        assert!(error.to_string().contains("Type mismatch"));
    }

    #[test]
    fn ir_program_globals_display_deterministic() {
        let mut program_a = IRProgram::new();
        program_a.add_global("beta".to_string(), IRValue::Integer(2));
        program_a.add_global("alpha".to_string(), IRValue::Integer(1));

        let mut program_b = IRProgram::new();
        program_b.add_global("alpha".to_string(), IRValue::Integer(1));
        program_b.add_global("beta".to_string(), IRValue::Integer(2));

        let display_a = format!("{}", program_a);
        let display_b = format!("{}", program_b);
        assert_eq!(display_a, display_b);
    }
}
