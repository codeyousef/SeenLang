//! IR Interpreter for the Seen Programming Language
//!
//! This module provides a reference interpreter for the Seen IR, enabling:
//! - Isolated testing of semantic correctness without codegen complexity
//! - Memory safety validation with bounds checking and use-after-free detection
//! - Differential testing against LLVM-compiled code
//! - Step-by-step debugging with full state inspection
//!
//! The interpreter is designed for correctness over performance, making it
//! ideal for debugging and validation purposes.

mod memory;
mod runtime;
mod value;
mod executor;
mod error;
mod validation;

#[cfg(test)]
mod integration_tests;

pub use memory::{Memory, MemoryRegion, Address, AllocationId};
pub use runtime::{Runtime, RuntimeConfig};
pub use value::{InterpreterValue, ValueType};
pub use executor::{Executor, ExecutionResult, StepResult};
pub use error::{InterpreterError, InterpreterErrorKind};
pub use validation::{IRValidator, ValidationResult, ValidationError, ValidationWarning};

use crate::module::IRModule;
use crate::IRProgram;

/// Main entry point for interpreting IR modules
pub struct Interpreter {
    runtime: Runtime,
    config: InterpreterConfig,
    trace_enabled: bool,
    breakpoints: Vec<Breakpoint>,
}

/// Configuration options for the interpreter
#[derive(Debug, Clone)]
pub struct InterpreterConfig {
    /// Enable memory safety checks (bounds, use-after-free, etc.)
    pub memory_checks: bool,
    /// Enable type checking at runtime
    pub type_checks: bool,
    /// Maximum stack depth before stack overflow error
    pub max_stack_depth: usize,
    /// Maximum number of instructions to execute (infinite loop protection)
    pub max_instructions: Option<u64>,
    /// Enable detailed execution trace output
    pub trace_execution: bool,
    /// Enable memory access logging
    pub trace_memory: bool,
}

impl Default for InterpreterConfig {
    fn default() -> Self {
        Self {
            memory_checks: true,
            type_checks: true,
            max_stack_depth: 1024,
            max_instructions: Some(10_000_000), // 10M instructions max
            trace_execution: false,
            trace_memory: false,
        }
    }
}

impl InterpreterConfig {
    /// Create a config optimized for debugging
    pub fn debug() -> Self {
        Self {
            memory_checks: true,
            type_checks: true,
            max_stack_depth: 256,
            max_instructions: Some(100_000),
            trace_execution: true,
            trace_memory: true,
        }
    }

    /// Create a config for production-like execution
    pub fn production() -> Self {
        Self {
            memory_checks: false,
            type_checks: false,
            max_stack_depth: 4096,
            max_instructions: None,
            trace_execution: false,
            trace_memory: false,
        }
    }
}

/// Breakpoint for debugging
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub function_name: Option<String>,
    pub instruction_index: Option<usize>,
    pub condition: Option<BreakpointCondition>,
}

/// Conditional breakpoint triggers
#[derive(Debug, Clone)]
pub enum BreakpointCondition {
    /// Break when a variable equals a specific value
    VariableEquals { name: String, value: InterpreterValue },
    /// Break on any memory write to a specific address
    MemoryWrite { address: Address },
    /// Break after N hits
    HitCount { count: u32 },
}

impl Interpreter {
    /// Create a new interpreter with default configuration
    pub fn new() -> Self {
        Self::with_config(InterpreterConfig::default())
    }

    /// Create a new interpreter with custom configuration
    pub fn with_config(config: InterpreterConfig) -> Self {
        let runtime_config = RuntimeConfig {
            memory_checks: config.memory_checks,
            trace_memory: config.trace_memory,
        };
        
        Self {
            runtime: Runtime::new(runtime_config),
            config,
            trace_enabled: false,
            breakpoints: Vec::new(),
        }
    }

    /// Enable execution tracing
    pub fn enable_trace(&mut self) {
        self.trace_enabled = true;
    }

    /// Add a breakpoint
    pub fn add_breakpoint(&mut self, breakpoint: Breakpoint) {
        self.breakpoints.push(breakpoint);
    }

    /// Execute a module starting from the specified function
    pub fn execute_function(
        &mut self,
        module: &IRModule,
        function_name: &str,
        args: Vec<InterpreterValue>,
    ) -> Result<InterpreterValue, InterpreterError> {
        let executor = Executor::new(&self.config, &mut self.runtime);
        executor.execute_function(module, function_name, args)
    }

    /// Execute an IR program starting from the entry point or main function
    pub fn execute_program(
        &mut self,
        program: &IRProgram,
        args: Vec<InterpreterValue>,
    ) -> Result<InterpreterValue, InterpreterError> {
        // Find the entry point function name
        let entry_point = program.entry_point.clone()
            .unwrap_or_else(|| "main".to_string());

        // Find the module containing the entry point
        for module in program.modules.iter() {
            if module.get_function(&entry_point).is_some() {
                return self.execute_function(module, &entry_point, args);
            }
        }

        // If not found in any module, try the first module
        if let Some(module) = program.modules.iter().next() {
            return self.execute_function(module, &entry_point, args);
        }

        Err(InterpreterError::from(format!(
            "No entry point '{}' found in program",
            entry_point
        )))
    }

    /// Execute a single instruction (for step debugging)
    pub fn step(
        &mut self,
        module: &IRModule,
        executor: &mut Executor,
    ) -> Result<StepResult, InterpreterError> {
        executor.step(module)
    }

    /// Get the current state of the interpreter for inspection
    pub fn get_state(&self) -> InterpreterState {
        let mem_stats = self.runtime.memory_stats();
        InterpreterState {
            memory_stats: MemoryStats {
                total_allocated: mem_stats.total_allocated,
                active_allocations: mem_stats.active_allocations,
                freed_allocations: mem_stats.freed_allocations,
                peak_usage: mem_stats.peak_usage,
            },
            call_stack_depth: self.runtime.call_stack_depth(),
            instructions_executed: self.runtime.instructions_executed(),
        }
    }

    /// Reset the interpreter to initial state
    pub fn reset(&mut self) {
        let runtime_config = RuntimeConfig {
            memory_checks: self.config.memory_checks,
            trace_memory: self.config.trace_memory,
        };
        self.runtime = Runtime::new(runtime_config);
    }

    /// Validate IR before execution using the full validator
    pub fn validate_module(&self, module: &IRModule) -> ValidationResult {
        let mut validator = IRValidator::new();
        validator.validate_module(module)
    }
}

/// Snapshot of interpreter state for debugging
#[derive(Debug, Clone)]
pub struct InterpreterState {
    pub memory_stats: MemoryStats,
    pub call_stack_depth: usize,
    pub instructions_executed: u64,
}

/// Memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub active_allocations: usize,
    pub freed_allocations: usize,
    pub peak_usage: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpreter_creation() {
        let interp = Interpreter::new();
        let state = interp.get_state();
        assert_eq!(state.call_stack_depth, 0);
        assert_eq!(state.instructions_executed, 0);
    }

    #[test]
    fn test_debug_config() {
        let config = InterpreterConfig::debug();
        assert!(config.memory_checks);
        assert!(config.trace_execution);
        assert!(config.trace_memory);
    }
}
