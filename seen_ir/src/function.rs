//! IR function representation for the Seen programming language

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};
use crate::instruction::{BasicBlock, ControlFlowGraph, Label};
use crate::value::{IRValue, IRType};

/// Function parameter representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub param_type: IRType,
    pub is_mutable: bool,
}

impl Parameter {
    pub fn new(name: impl Into<String>, param_type: IRType) -> Self {
        Self {
            name: name.into(),
            param_type,
            is_mutable: false,
        }
    }
    
    pub fn mutable(name: impl Into<String>, param_type: IRType) -> Self {
        Self {
            name: name.into(),
            param_type,
            is_mutable: true,
        }
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_mutable {
            write!(f, "mut {}: {}", self.name, self.param_type)
        } else {
            write!(f, "{}: {}", self.name, self.param_type)
        }
    }
}

/// Local variable in a function
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalVariable {
    pub name: String,
    pub var_type: IRType,
    pub is_mutable: bool,
    pub register: Option<u32>, // Virtual register assignment
}

impl LocalVariable {
    pub fn new(name: impl Into<String>, var_type: IRType) -> Self {
        Self {
            name: name.into(),
            var_type,
            is_mutable: false,
            register: None,
        }
    }
    
    pub fn mutable(name: impl Into<String>, var_type: IRType) -> Self {
        Self {
            name: name.into(),
            var_type,
            is_mutable: true,
            register: None,
        }
    }
    
    pub fn assign_register(&mut self, register: u32) {
        self.register = Some(register);
    }
}

/// IR function representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: IRType,
    pub locals: HashMap<String, LocalVariable>,
    pub cfg: ControlFlowGraph,
    pub is_public: bool,
    pub is_extern: bool,
    pub calling_convention: CallingConvention,
    pub stack_size: Option<usize>, // Computed during compilation
    pub register_count: u32, // Number of virtual registers used
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CallingConvention {
    Seen,     // Default Seen calling convention
    C,        // C calling convention for extern functions
    System,   // System calling convention
}

impl IRFunction {
    pub fn new(name: impl Into<String>, return_type: IRType) -> Self {
        Self {
            name: name.into(),
            parameters: Vec::new(),
            return_type,
            locals: HashMap::new(),
            cfg: ControlFlowGraph::new(),
            is_public: false,
            is_extern: false,
            calling_convention: CallingConvention::Seen,
            stack_size: None,
            register_count: 0,
        }
    }
    
    pub fn public(mut self) -> Self {
        self.is_public = true;
        self
    }
    
    pub fn extern_function(mut self, convention: CallingConvention) -> Self {
        self.is_extern = true;
        self.calling_convention = convention;
        self
    }
    
    pub fn add_parameter(&mut self, parameter: Parameter) {
        self.parameters.push(parameter);
    }
    
    pub fn add_local(&mut self, local: LocalVariable) {
        self.locals.insert(local.name.clone(), local);
    }
    
    pub fn get_local(&self, name: &str) -> Option<&LocalVariable> {
        self.locals.get(name)
    }
    
    pub fn get_local_mut(&mut self, name: &str) -> Option<&mut LocalVariable> {
        self.locals.get_mut(name)
    }
    
    /// Allocate a new virtual register
    pub fn allocate_register(&mut self) -> u32 {
        let register = self.register_count;
        self.register_count += 1;
        register
    }
    
    /// Get the function signature as an IRType
    pub fn signature(&self) -> IRType {
        IRType::Function {
            parameters: self.parameters.iter().map(|p| p.param_type.clone()).collect(),
            return_type: Box::new(self.return_type.clone()),
        }
    }
    
    /// Add a basic block to the function
    pub fn add_block(&mut self, block: BasicBlock) {
        self.cfg.add_block(block);
    }
    
    /// Get a basic block by name
    pub fn get_block(&self, name: &str) -> Option<&BasicBlock> {
        self.cfg.get_block(name)
    }
    
    /// Get a mutable reference to a basic block
    pub fn get_block_mut(&mut self, name: &str) -> Option<&mut BasicBlock> {
        self.cfg.get_block_mut(name)
    }
    
    /// Validate the function's IR
    pub fn validate(&self) -> Result<(), String> {
        // Validate the control flow graph
        self.cfg.validate()?;
        
        // Check that all parameters have unique names
        let mut param_names = std::collections::HashSet::new();
        for param in &self.parameters {
            if !param_names.insert(&param.name) {
                return Err(format!("Duplicate parameter name: {}", param.name));
            }
        }
        
        // Check that local variables don't conflict with parameters
        for local_name in self.locals.keys() {
            if param_names.contains(local_name) {
                return Err(format!("Local variable {} conflicts with parameter", local_name));
            }
        }
        
        // TODO: Add more validation (type checking, register usage, etc.)
        
        Ok(())
    }
    
    /// Calculate the stack frame size needed for this function
    pub fn calculate_stack_size(&mut self) {
        let mut size = 0;
        
        // Add size for local variables
        for local in self.locals.values() {
            size += local.var_type.size_bytes();
        }
        
        // Add alignment padding
        size = (size + 15) & !15; // Align to 16 bytes
        
        self.stack_size = Some(size);
    }
    
    /// Check if this function is a leaf function (doesn't call other functions)
    pub fn is_leaf(&self) -> bool {
        for block in self.cfg.blocks.values() {
            for instruction in &block.instructions {
                if matches!(instruction, crate::instruction::Instruction::Call { .. }) {
                    return false;
                }
            }
            if let Some(terminator) = &block.terminator {
                if matches!(terminator, crate::instruction::Instruction::Call { .. }) {
                    return false;
                }
            }
        }
        true
    }
}

impl fmt::Display for IRFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Function signature
        write!(f, "fn {}", self.name)?;
        if self.is_public {
            write!(f, " [public]")?;
        }
        if self.is_extern {
            write!(f, " [extern]")?;
        }
        write!(f, "(")?;
        
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", param)?;
        }
        
        write!(f, ") -> {} {{", self.return_type)?;
        
        // Local variables
        if !self.locals.is_empty() {
            writeln!(f)?;
            writeln!(f, "  ; Local variables")?;
            for local in self.locals.values() {
                write!(f, "  ")?;
                if local.is_mutable {
                    write!(f, "mut ")?;
                }
                write!(f, "{}: {}", local.name, local.var_type)?;
                if let Some(reg) = local.register {
                    write!(f, " = %r{}", reg)?;
                }
                writeln!(f)?;
            }
        }
        
        // Stack size information
        if let Some(size) = self.stack_size {
            writeln!(f, "  ; Stack size: {} bytes", size)?;
        }
        
        // Control flow graph
        if !self.cfg.blocks.is_empty() {
            writeln!(f)?;
            write!(f, "{}", self.cfg)?;
        }
        
        writeln!(f, "}}")?;
        Ok(())
    }
}

/// Function call information for optimization and analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSite {
    pub caller: String,
    pub callee: String,
    pub block_label: String,
    pub arguments: Vec<IRValue>,
    pub return_value: Option<IRValue>,
    pub is_tail_call: bool,
}

impl CallSite {
    pub fn new(
        caller: impl Into<String>,
        callee: impl Into<String>,
        block_label: impl Into<String>
    ) -> Self {
        Self {
            caller: caller.into(),
            callee: callee.into(),
            block_label: block_label.into(),
            arguments: Vec::new(),
            return_value: None,
            is_tail_call: false,
        }
    }
    
    pub fn with_args(mut self, args: Vec<IRValue>) -> Self {
        self.arguments = args;
        self
    }
    
    pub fn with_return(mut self, return_val: IRValue) -> Self {
        self.return_value = Some(return_val);
        self
    }
    
    pub fn tail_call(mut self) -> Self {
        self.is_tail_call = true;
        self
    }
}

/// Call graph for the entire program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraph {
    pub functions: HashMap<String, IRFunction>,
    pub call_sites: Vec<CallSite>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            call_sites: Vec::new(),
        }
    }
    
    pub fn add_function(&mut self, function: IRFunction) {
        self.functions.insert(function.name.clone(), function);
    }
    
    pub fn add_call_site(&mut self, call_site: CallSite) {
        self.call_sites.push(call_site);
    }
    
    pub fn get_function(&self, name: &str) -> Option<&IRFunction> {
        self.functions.get(name)
    }
    
    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IRFunction> {
        self.functions.get_mut(name)
    }
    
    /// Get all functions called by the given function
    pub fn callees(&self, caller: &str) -> Vec<&str> {
        self.call_sites
            .iter()
            .filter(|cs| cs.caller == caller)
            .map(|cs| cs.callee.as_str())
            .collect()
    }
    
    /// Get all functions that call the given function
    pub fn callers(&self, callee: &str) -> Vec<&str> {
        self.call_sites
            .iter()
            .filter(|cs| cs.callee == callee)
            .map(|cs| cs.caller.as_str())
            .collect()
    }
    
    /// Check if the call graph has cycles (recursive functions)
    pub fn has_cycles(&self) -> bool {
        // Simple cycle detection using DFS
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        
        for function_name in self.functions.keys() {
            if self.has_cycle_dfs(function_name, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        
        false
    }
    
    fn has_cycle_dfs(
        &self,
        function: &str,
        visited: &mut std::collections::HashSet<String>,
        rec_stack: &mut std::collections::HashSet<String>
    ) -> bool {
        if rec_stack.contains(function) {
            return true;
        }
        
        if visited.contains(function) {
            return false;
        }
        
        visited.insert(function.to_string());
        rec_stack.insert(function.to_string());
        
        for callee in self.callees(function) {
            if self.has_cycle_dfs(callee, visited, rec_stack) {
                return true;
            }
        }
        
        rec_stack.remove(function);
        false
    }
}

impl Default for CallGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction::Instruction;

    #[test]
    fn test_function_creation() {
        let mut func = IRFunction::new("test_func", IRType::Integer);
        
        func.add_parameter(Parameter::new("x", IRType::Integer));
        func.add_parameter(Parameter::new("y", IRType::Float));
        
        assert_eq!(func.name, "test_func");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.return_type, IRType::Integer);
    }
    
    #[test]
    fn test_local_variables() {
        let mut func = IRFunction::new("test", IRType::Void);
        
        let local = LocalVariable::mutable("temp", IRType::Integer);
        func.add_local(local);
        
        assert!(func.get_local("temp").is_some());
        assert!(func.get_local("temp").unwrap().is_mutable);
        assert_eq!(func.get_local("temp").unwrap().var_type, IRType::Integer);
    }
    
    #[test]
    fn test_register_allocation() {
        let mut func = IRFunction::new("test", IRType::Void);
        
        let reg1 = func.allocate_register();
        let reg2 = func.allocate_register();
        
        assert_eq!(reg1, 0);
        assert_eq!(reg2, 1);
        assert_eq!(func.register_count, 2);
    }
    
    #[test]
    fn test_function_signature() {
        let mut func = IRFunction::new("add", IRType::Integer);
        func.add_parameter(Parameter::new("x", IRType::Integer));
        func.add_parameter(Parameter::new("y", IRType::Integer));
        
        let sig = func.signature();
        if let IRType::Function { parameters, return_type } = sig {
            assert_eq!(parameters.len(), 2);
            assert_eq!(*return_type, IRType::Integer);
        } else {
            panic!("Expected function type");
        }
    }
    
    #[test]
    fn test_call_graph() {
        let mut graph = CallGraph::new();
        
        let func1 = IRFunction::new("main", IRType::Void);
        let func2 = IRFunction::new("helper", IRType::Integer);
        
        graph.add_function(func1);
        graph.add_function(func2);
        
        let call_site = CallSite::new("main", "helper", "main_block");
        graph.add_call_site(call_site);
        
        assert_eq!(graph.callees("main"), vec!["helper"]);
        assert_eq!(graph.callers("helper"), vec!["main"]);
    }
}