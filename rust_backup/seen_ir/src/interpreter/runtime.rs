//! Runtime environment for the IR interpreter
//!
//! Manages memory, call stack, global variables, and execution state.

use super::memory::{Memory, MemoryConfig, MemoryStats, Address};
use super::value::{InterpreterValue, ValueType};
use super::error::{InterpreterError, InterpreterErrorKind, StackFrame, ErrorLocation};
use std::collections::HashMap;

/// Configuration for the runtime
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub memory_checks: bool,
    pub trace_memory: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            memory_checks: true,
            trace_memory: false,
        }
    }
}

/// Stack frame for function execution
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Function name
    pub function_name: String,
    /// Local variables
    pub locals: HashMap<String, InterpreterValue>,
    /// Current instruction index
    pub instruction_pointer: usize,
    /// Return value destination (variable name)
    pub return_dest: Option<String>,
    /// Base pointer for this frame's stack allocations
    pub stack_base: Address,
}

impl CallFrame {
    pub fn new(function_name: String, return_dest: Option<String>) -> Self {
        Self {
            function_name,
            locals: HashMap::new(),
            instruction_pointer: 0,
            return_dest,
            stack_base: Address::NULL,
        }
    }

    /// Get a local variable
    pub fn get_local(&self, name: &str) -> Option<&InterpreterValue> {
        self.locals.get(name)
    }

    /// Set a local variable
    pub fn set_local(&mut self, name: String, value: InterpreterValue) {
        self.locals.insert(name, value);
    }

    /// Check if a local variable exists
    pub fn has_local(&self, name: &str) -> bool {
        self.locals.contains_key(name)
    }
}

/// The runtime environment
pub struct Runtime {
    /// Memory manager
    pub memory: Memory,
    /// Call stack
    call_stack: Vec<CallFrame>,
    /// Global variables
    globals: HashMap<String, InterpreterValue>,
    /// String table for interned strings
    string_table: Vec<String>,
    /// Total instructions executed
    instructions_executed: u64,
    /// Configuration
    config: RuntimeConfig,
    /// Standard output capture (for testing)
    stdout_capture: Vec<String>,
    /// Whether to capture stdout
    capture_stdout: bool,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Self {
        let memory_config = MemoryConfig {
            redzones: config.memory_checks,
            use_after_free_detection: config.memory_checks,
            trace: config.trace_memory,
            ..Default::default()
        };

        Self {
            memory: Memory::with_config(memory_config),
            call_stack: Vec::new(),
            globals: HashMap::new(),
            string_table: Vec::new(),
            instructions_executed: 0,
            config,
            stdout_capture: Vec::new(),
            capture_stdout: false,
        }
    }

    /// Get call stack depth
    pub fn call_stack_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Get instructions executed count
    pub fn instructions_executed(&self) -> u64 {
        self.instructions_executed
    }

    /// Increment instruction counter
    pub fn increment_instructions(&mut self) {
        self.instructions_executed += 1;
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> MemoryStats {
        self.memory.stats.clone()
    }

    // === Call Stack Management ===

    /// Push a new call frame
    pub fn push_frame(&mut self, frame: CallFrame) -> Result<(), InterpreterError> {
        self.call_stack.push(frame);
        Ok(())
    }

    /// Pop the current call frame
    pub fn pop_frame(&mut self) -> Option<CallFrame> {
        self.call_stack.pop()
    }

    /// Get the current call frame
    pub fn current_frame(&self) -> Option<&CallFrame> {
        self.call_stack.last()
    }

    /// Get the current call frame mutably
    pub fn current_frame_mut(&mut self) -> Option<&mut CallFrame> {
        self.call_stack.last_mut()
    }

    /// Get the backtrace as stack frames
    pub fn get_backtrace(&self) -> Vec<StackFrame> {
        self.call_stack
            .iter()
            .rev()
            .map(|frame| StackFrame {
                function_name: frame.function_name.clone(),
                instruction_index: frame.instruction_pointer,
                source_location: None,
            })
            .collect()
    }

    // === Variable Management ===

    /// Get a variable (checks locals first, then globals)
    pub fn get_variable(&self, name: &str) -> Result<&InterpreterValue, InterpreterError> {
        // Check current frame's locals first
        if let Some(frame) = self.current_frame() {
            if let Some(value) = frame.get_local(name) {
                return Ok(value);
            }
        }

        // Check globals
        self.globals.get(name)
            .ok_or_else(|| InterpreterError::undefined_variable(name))
    }

    /// Set a local variable in the current frame
    pub fn set_local(&mut self, name: String, value: InterpreterValue) -> Result<(), InterpreterError> {
        let frame = self.current_frame_mut()
            .ok_or_else(|| InterpreterError::internal("No active call frame"))?;
        frame.set_local(name, value);
        Ok(())
    }

    /// Set a global variable
    pub fn set_global(&mut self, name: String, value: InterpreterValue) {
        self.globals.insert(name, value);
    }

    /// Get a global variable
    pub fn get_global(&self, name: &str) -> Option<&InterpreterValue> {
        self.globals.get(name)
    }

    // === String Table ===

    /// Intern a string and return its index
    pub fn intern_string(&mut self, s: String) -> usize {
        // Check if already interned
        if let Some(idx) = self.string_table.iter().position(|x| x == &s) {
            return idx;
        }
        let idx = self.string_table.len();
        self.string_table.push(s);
        idx
    }

    /// Get a string by its intern index
    pub fn get_interned_string(&self, index: usize) -> Option<&str> {
        self.string_table.get(index).map(|s| s.as_str())
    }

    // === Memory Operations ===

    /// Allocate memory for a value
    pub fn allocate(&mut self, size: usize, type_info: Option<String>) -> Result<Address, InterpreterError> {
        self.memory.allocate(size, type_info)
            .map_err(|e| InterpreterError::internal(e))
    }

    /// Free memory
    pub fn free(&mut self, address: Address) -> Result<(), InterpreterError> {
        let location = self.current_frame()
            .map(|f| format!("{}[{}]", f.function_name, f.instruction_pointer));
        self.memory.free(address, location)
            .map_err(|e| {
                if e.contains("Double free") {
                    InterpreterError::double_free(address)
                } else if e.contains("Use-after-free") {
                    InterpreterError::use_after_free(address, None)
                } else {
                    InterpreterError::internal(e)
                }
            })
    }

    /// Read an integer from memory
    pub fn read_i64(&mut self, address: Address) -> Result<i64, InterpreterError> {
        self.memory.read_i64(address)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Write an integer to memory
    pub fn write_i64(&mut self, address: Address, value: i64) -> Result<(), InterpreterError> {
        self.memory.write_i64(address, value)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Read a float from memory
    pub fn read_f64(&mut self, address: Address) -> Result<f64, InterpreterError> {
        self.memory.read_f64(address)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Write a float to memory
    pub fn write_f64(&mut self, address: Address, value: f64) -> Result<(), InterpreterError> {
        self.memory.write_f64(address, value)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Read a pointer from memory
    pub fn read_ptr(&mut self, address: Address) -> Result<Address, InterpreterError> {
        self.memory.read_ptr(address)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Write a pointer to memory
    pub fn write_ptr(&mut self, address: Address, ptr: Address) -> Result<(), InterpreterError> {
        self.memory.write_ptr(address, ptr)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Read raw bytes from memory
    pub fn read_bytes(&mut self, address: Address, size: usize) -> Result<Vec<u8>, InterpreterError> {
        self.memory.read(address, size)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Write raw bytes to memory
    pub fn write_bytes(&mut self, address: Address, bytes: &[u8]) -> Result<(), InterpreterError> {
        self.memory.write(address, bytes)
            .map_err(|e| self.memory_error_to_interpreter_error(e, address))
    }

    /// Convert memory error string to InterpreterError
    fn memory_error_to_interpreter_error(&self, error: String, address: Address) -> InterpreterError {
        let kind = if error.contains("Null pointer") {
            InterpreterErrorKind::NullPointerDereference
        } else if error.contains("Use-after-free") {
            InterpreterErrorKind::UseAfterFree
        } else if error.contains("Out of bounds") {
            InterpreterErrorKind::BufferOverflow
        } else if error.contains("Buffer overflow") {
            InterpreterErrorKind::BufferOverflow
        } else if error.contains("Buffer underflow") {
            InterpreterErrorKind::BufferUnderflow
        } else {
            InterpreterErrorKind::InvalidPointer
        };

        let mut err = InterpreterError::new(kind, error);
        err.backtrace = self.get_backtrace();
        if let Some(frame) = self.current_frame() {
            err.location = Some(ErrorLocation {
                function_name: frame.function_name.clone(),
                instruction_index: frame.instruction_pointer,
                source_line: None,
                source_column: None,
                source_file: None,
            });
        }
        err
    }

    // === I/O Operations ===

    /// Enable stdout capture
    pub fn enable_stdout_capture(&mut self) {
        self.capture_stdout = true;
    }

    /// Get captured stdout
    pub fn get_captured_stdout(&self) -> &[String] {
        &self.stdout_capture
    }

    /// Clear captured stdout
    pub fn clear_stdout_capture(&mut self) {
        self.stdout_capture.clear();
    }

    /// Print to stdout (or capture)
    pub fn print(&mut self, message: &str) {
        if self.capture_stdout {
            self.stdout_capture.push(message.to_string());
        } else {
            print!("{}", message);
        }
    }

    /// Print line to stdout (or capture)
    pub fn println(&mut self, message: &str) {
        if self.capture_stdout {
            self.stdout_capture.push(format!("{}\n", message));
        } else {
            println!("{}", message);
        }
    }

    // === Debugging ===

    /// Dump the current state for debugging
    pub fn dump_state(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Runtime State ===\n\n");

        // Call stack
        out.push_str("Call Stack:\n");
        for (i, frame) in self.call_stack.iter().enumerate() {
            out.push_str(&format!(
                "  {}: {} @ instruction {}\n",
                i, frame.function_name, frame.instruction_pointer
            ));
            for (name, value) in &frame.locals {
                out.push_str(&format!("    {} = {}\n", name, value));
            }
        }

        // Globals
        out.push_str("\nGlobals:\n");
        for (name, value) in &self.globals {
            out.push_str(&format!("  {} = {}\n", name, value));
        }

        // Memory
        out.push_str("\n");
        out.push_str(&self.memory.dump());

        // Stats
        out.push_str(&format!("\nInstructions executed: {}\n", self.instructions_executed));

        out
    }

    /// Verify memory integrity
    pub fn verify_memory(&self) -> Result<(), Vec<String>> {
        self.memory.verify_all()
    }

    /// Check for memory leaks
    pub fn check_leaks(&self) -> Vec<String> {
        let leaks = self.memory.get_leaks();
        leaks.iter().map(|region| {
            format!(
                "Memory leak: {} bytes at {} (type: {:?}, allocated at: {:?})",
                region.size, region.base, region.type_info, region.allocation_site
            )
        }).collect()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(RuntimeConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::new(RuntimeConfig::default());
        assert_eq!(runtime.call_stack_depth(), 0);
        assert_eq!(runtime.instructions_executed(), 0);
    }

    #[test]
    fn test_call_frame_management() {
        let mut runtime = Runtime::new(RuntimeConfig::default());
        
        let frame = CallFrame::new("test_func".to_string(), None);
        runtime.push_frame(frame).unwrap();
        
        assert_eq!(runtime.call_stack_depth(), 1);
        assert_eq!(runtime.current_frame().unwrap().function_name, "test_func");
        
        runtime.pop_frame();
        assert_eq!(runtime.call_stack_depth(), 0);
    }

    #[test]
    fn test_variable_management() {
        let mut runtime = Runtime::new(RuntimeConfig::default());
        
        // Set global
        runtime.set_global("global_var".to_string(), InterpreterValue::integer(42));
        
        // Push frame and set local
        let frame = CallFrame::new("test".to_string(), None);
        runtime.push_frame(frame).unwrap();
        runtime.set_local("local_var".to_string(), InterpreterValue::integer(100)).unwrap();
        
        // Local takes precedence
        assert_eq!(
            runtime.get_variable("local_var").unwrap().as_integer().unwrap(),
            100
        );
        
        // Can still access global
        assert_eq!(
            runtime.get_variable("global_var").unwrap().as_integer().unwrap(),
            42
        );
    }

    #[test]
    fn test_stdout_capture() {
        let mut runtime = Runtime::new(RuntimeConfig::default());
        runtime.enable_stdout_capture();
        
        runtime.println("Hello");
        runtime.println("World");
        
        let captured = runtime.get_captured_stdout();
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0], "Hello\n");
        assert_eq!(captured[1], "World\n");
    }

    #[test]
    fn test_memory_operations() {
        let mut runtime = Runtime::new(RuntimeConfig::default());
        
        let addr = runtime.allocate(16, Some("test".to_string())).unwrap();
        assert!(!addr.is_null());
        
        runtime.write_i64(addr, 12345).unwrap();
        assert_eq!(runtime.read_i64(addr).unwrap(), 12345);
        
        runtime.free(addr).unwrap();
    }
}
