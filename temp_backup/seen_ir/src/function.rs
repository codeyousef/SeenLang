//! IR function representation for the Seen programming language

use crate::arena::{Arena, ArenaIndex};
use crate::instruction::{BasicBlock, ControlFlowGraph};
use crate::value::{IRType, IRValue};
use crate::SimdPolicy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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

/// Hint describing how aggressively LLVM should inline a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineHint {
    Auto,
    AlwaysInline,
    NeverInline,
}

impl Default for InlineHint {
    fn default() -> Self {
        InlineHint::Auto
    }
}

impl InlineHint {
    pub fn as_str(&self) -> &'static str {
        match self {
            InlineHint::Auto => "auto",
            InlineHint::AlwaysInline => "always",
            InlineHint::NeverInline => "never",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(InlineHint::Auto),
            "always" | "alwaysinline" => Some(InlineHint::AlwaysInline),
            "never" | "noinline" => Some(InlineHint::NeverInline),
            _ => None,
        }
    }
}

/// Classification of register pressure discovered by ML heuristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegisterPressureClass {
    Unknown,
    Low,
    Medium,
    High,
}

impl Default for RegisterPressureClass {
    fn default() -> Self {
        RegisterPressureClass::Unknown
    }
}

impl RegisterPressureClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            RegisterPressureClass::Unknown => "unknown",
            RegisterPressureClass::Low => "low",
            RegisterPressureClass::Medium => "medium",
            RegisterPressureClass::High => "high",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "unknown" => Some(RegisterPressureClass::Unknown),
            "low" => Some(RegisterPressureClass::Low),
            "medium" => Some(RegisterPressureClass::Medium),
            "high" => Some(RegisterPressureClass::High),
            _ => None,
        }
    }
}

/// Whether a function should execute as scalar or vectorized code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimdMode {
    Scalar,
    Vectorized,
}

impl SimdMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SimdMode::Scalar => "scalar",
            SimdMode::Vectorized => "vectorized",
        }
    }
}

/// Reason recorded for the chosen SIMD mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimdDecisionReason {
    Unknown,
    PolicyOff,
    ForcedMax,
    NoOpportunities,
    AutoNoHotLoops,
    AutoLowArithmeticIntensity,
    AutoMemoryBound,
    AutoHighRegisterPressure,
    AutoHotLoop,
}

impl SimdDecisionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SimdDecisionReason::Unknown => "unknown",
            SimdDecisionReason::PolicyOff => "policy_off",
            SimdDecisionReason::ForcedMax => "policy_forced_max",
            SimdDecisionReason::NoOpportunities => "no_vector_opportunities",
            SimdDecisionReason::AutoNoHotLoops => "auto_missing_hot_loops",
            SimdDecisionReason::AutoLowArithmeticIntensity => "auto_low_arithmetic_intensity",
            SimdDecisionReason::AutoMemoryBound => "auto_memory_bound",
            SimdDecisionReason::AutoHighRegisterPressure => "auto_high_register_pressure",
            SimdDecisionReason::AutoHotLoop => "auto_hot_loop_vectorized",
        }
    }
}

/// Metadata used for SIMD reporting downstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimdMetadata {
    pub policy: SimdPolicy,
    pub mode: SimdMode,
    pub reason: SimdDecisionReason,
    pub hot_loops: usize,
    pub arithmetic_ops: usize,
    pub memory_ops: usize,
    pub existing_vector_ops: usize,
    pub estimated_speedup: f32,
    pub vector_width_bits: Option<u32>,
}

impl Default for SimdMetadata {
    fn default() -> Self {
        Self {
            policy: SimdPolicy::Auto,
            mode: SimdMode::Scalar,
            reason: SimdDecisionReason::Unknown,
            hot_loops: 0,
            arithmetic_ops: 0,
            memory_ops: 0,
            existing_vector_ops: 0,
            estimated_speedup: 1.0,
            vector_width_bits: None,
        }
    }
}

/// IR function representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: IRType,
    pub locals: Vec<LocalVariable>,
    #[serde(skip)]
    local_lookup: HashMap<String, u32>,
    pub cfg: ControlFlowGraph,
    pub is_public: bool,
    pub is_extern: bool,
    pub calling_convention: CallingConvention,
    pub stack_size: Option<usize>, // Computed during compilation
    pub register_count: u32,       // Number of virtual registers used
    #[serde(default)]
    pub inline_hint: InlineHint,
    #[serde(default)]
    pub register_pressure: RegisterPressureClass,
    #[serde(default)]
    pub simd_metadata: SimdMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CallingConvention {
    Seen,   // Default Seen calling convention
    C,      // C calling convention for extern functions
    System, // System calling convention
}

impl IRFunction {
    pub fn new(name: impl Into<String>, return_type: IRType) -> Self {
        Self {
            name: name.into(),
            parameters: Vec::new(),
            return_type,
            locals: Vec::new(),
            local_lookup: HashMap::new(),
            cfg: ControlFlowGraph::new(),
            is_public: false,
            is_extern: false,
            calling_convention: CallingConvention::Seen,
            stack_size: None,
            register_count: 0,
            inline_hint: InlineHint::Auto,
            register_pressure: RegisterPressureClass::Unknown,
            simd_metadata: SimdMetadata::default(),
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
        if let Some(index) = self.local_lookup.get(&local.name).cloned() {
            self.locals[index as usize] = local;
        } else {
            let index = self.locals.len() as u32;
            self.local_lookup.insert(local.name.clone(), index);
            self.locals.push(local);
        }
    }

    pub fn get_local(&self, name: &str) -> Option<&LocalVariable> {
        self.local_lookup
            .get(name)
            .and_then(|idx| self.locals.get(*idx as usize))
    }

    pub fn get_local_mut(&mut self, name: &str) -> Option<&mut LocalVariable> {
        if let Some(idx) = self.local_lookup.get(name).cloned() {
            self.locals.get_mut(idx as usize)
        } else {
            None
        }
    }

    pub fn locals_iter(&self) -> impl Iterator<Item = &LocalVariable> {
        self.locals.iter()
    }

    pub fn locals_iter_mut(&mut self) -> impl Iterator<Item = &mut LocalVariable> {
        self.locals.iter_mut()
    }

    pub fn has_local(&self, name: &str) -> bool {
        self.local_lookup.contains_key(name)
    }

    pub fn rebuild_local_lookup(&mut self) {
        self.local_lookup.clear();
        for (idx, local) in self.locals.iter().enumerate() {
            self.local_lookup.insert(local.name.clone(), idx as u32);
        }
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
            parameters: self
                .parameters
                .iter()
                .map(|p| p.param_type.clone())
                .collect(),
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
        for local in &self.locals {
            if param_names.contains(&local.name) {
                return Err(format!(
                    "Local variable {} conflicts with parameter",
                    local.name
                ));
            }
        }

        // Additional validation: check register usage and type consistency
        self.validate_register_usage()?;
        self.validate_type_consistency()?;

        Ok(())
    }

    /// Calculate the stack frame size needed for this function
    pub fn calculate_stack_size(&mut self) {
        let mut size = 0;

        // Add size for local variables
        for local in self.locals.iter() {
            size += local.var_type.size_bytes();
        }

        // Add alignment padding
        size = (size + 15) & !15; // Align to 16 bytes

        self.stack_size = Some(size);
    }

    /// Check if this function is a leaf function (doesn't call other functions)
    pub fn is_leaf(&self) -> bool {
        for block in self.cfg.blocks_iter() {
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

    /// Validate register usage throughout the function
    fn validate_register_usage(&self) -> Result<(), String> {
        use std::collections::HashSet;
        let mut used_registers = HashSet::new();

        for block in self.cfg.blocks_iter() {
            for instruction in &block.instructions {
                self.collect_instruction_registers(instruction, &mut used_registers);
            }
        }

        // Check for register conflicts and validate usage patterns
        for register in used_registers {
            if register > 1000 {
                return Err(format!("Register r{} exceeds reasonable limit", register));
            }
        }

        Ok(())
    }

    /// Validate type consistency across instructions
    fn validate_type_consistency(&self) -> Result<(), String> {
        for block in self.cfg.blocks.iter() {
            for instruction in &block.instructions {
                self.validate_instruction_types(instruction)?;
            }
        }

        Ok(())
    }

    /// Collect all registers used by an instruction
    fn collect_instruction_registers(
        &self,
        instruction: &crate::Instruction,
        registers: &mut std::collections::HashSet<u32>,
    ) {
        use crate::Instruction;

        match instruction {
            Instruction::Binary {
                result,
                left,
                right,
                ..
            } => {
                self.collect_value_registers(result, registers);
                self.collect_value_registers(left, registers);
                self.collect_value_registers(right, registers);
            }
            Instruction::Unary {
                result, operand, ..
            } => {
                self.collect_value_registers(result, registers);
                self.collect_value_registers(operand, registers);
            }
            Instruction::Move { dest, source, .. } => {
                self.collect_value_registers(dest, registers);
                self.collect_value_registers(source, registers);
            }
            // Add other instruction types as needed
            _ => {}
        }
    }

    /// Collect registers from an IR value
    fn collect_value_registers(
        &self,
        value: &crate::IRValue,
        registers: &mut std::collections::HashSet<u32>,
    ) {
        if let crate::IRValue::Register(reg_id) = value {
            registers.insert(*reg_id);
        }
    }

    /// Validate types for a specific instruction
    fn validate_instruction_types(&self, instruction: &crate::Instruction) -> Result<(), String> {
        use crate::Instruction;

        match instruction {
            Instruction::Binary { left, right, .. } => {
                // Ensure binary operation operands have compatible types
                if !self.types_are_compatible(left, right) {
                    return Err("Binary operation with incompatible types".to_string());
                }
            }
            // Add more type checking as needed
            _ => {}
        }

        Ok(())
    }

    /// Check if two values have compatible types for operations
    fn types_are_compatible(&self, left: &crate::IRValue, right: &crate::IRValue) -> bool {
        let left_type = left.get_type();
        let right_type = right.get_type();

        // Check type compatibility based on IR type system
        match (&left_type, &right_type) {
            // Same types are compatible
            (l, r) if l == r => true,

            // Numeric compatibility
            (crate::IRType::Integer, crate::IRType::Float) => true,
            (crate::IRType::Float, crate::IRType::Integer) => true,

            // Pointer compatibility
            (crate::IRType::Pointer(_), crate::IRType::Pointer(_)) => true,
            (crate::IRType::Reference(_), crate::IRType::Reference(_)) => true,

            // Optional compatibility
            (crate::IRType::Optional(inner), other) => {
                self.types_are_compatible_inner(inner, &other)
            }
            (other, crate::IRType::Optional(inner)) => {
                self.types_are_compatible_inner(&other, inner)
            }

            // Generic types are compatible with anything during IR generation
            (crate::IRType::Generic(_), _) => true,
            (_, crate::IRType::Generic(_)) => true,

            // Arrays with compatible element types
            (crate::IRType::Array(left_elem), crate::IRType::Array(right_elem)) => {
                left_elem == right_elem
            }

            _ => false,
        }
    }

    fn types_are_compatible_inner(&self, a: &crate::IRType, b: &crate::IRType) -> bool {
        a == b || matches!(a, crate::IRType::Generic(_)) || matches!(b, crate::IRType::Generic(_))
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
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", param)?;
        }

        write!(f, ") -> {} {{", self.return_type)?;

        // Local variables
        if !self.locals.is_empty() {
            writeln!(f)?;
            writeln!(f, "  ; Local variables")?;
            let mut locals_sorted: Vec<&LocalVariable> = self.locals.iter().collect();
            locals_sorted.sort_by(|a, b| a.name.cmp(&b.name));
            for local in locals_sorted {
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
        block_label: impl Into<String>,
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
#[derive(Debug, Clone)]
pub struct CallGraph {
    pub functions: Arena<IRFunction>,
    function_lookup: HashMap<String, ArenaIndex>,
    pub call_sites: Vec<CallSite>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            functions: Arena::new(),
            function_lookup: HashMap::new(),
            call_sites: Vec::new(),
        }
    }

    pub fn add_function(&mut self, mut function: IRFunction) {
        function.rebuild_local_lookup();
        if let Some(index) = self.function_lookup.get(&function.name).copied() {
            if let Some(slot) = self.functions.get_mut(index) {
                *slot = function;
            }
            return;
        }
        let index = self.functions.push(function);
        let stored = self.functions[index.as_usize()].name.clone();
        self.function_lookup.insert(stored, index);
    }

    pub fn add_call_site(&mut self, call_site: CallSite) {
        self.call_sites.push(call_site);
    }

    pub fn get_function(&self, name: &str) -> Option<&IRFunction> {
        self.function_lookup
            .get(name)
            .and_then(|index| self.functions.get(*index))
    }

    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IRFunction> {
        if let Some(index) = self.function_lookup.get(name).copied() {
            self.functions.get_mut(index)
        } else {
            None
        }
    }

    pub fn functions_iter(&self) -> impl Iterator<Item = &IRFunction> {
        self.functions.iter()
    }

    pub fn functions_iter_mut(&mut self) -> impl Iterator<Item = &mut IRFunction> {
        self.functions.iter_mut()
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

        for function in &self.functions {
            if self.has_cycle_dfs(&function.name, &mut visited, &mut rec_stack) {
                return true;
            }
        }

        false
    }

    fn has_cycle_dfs(
        &self,
        function: &str,
        visited: &mut std::collections::HashSet<String>,
        rec_stack: &mut std::collections::HashSet<String>,
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

#[derive(Serialize, Deserialize)]
struct CallGraphSerde {
    functions: Vec<IRFunction>,
    call_sites: Vec<CallSite>,
}

impl Serialize for CallGraph {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CallGraphSerde {
            functions: self.functions.clone().into_vec(),
            call_sites: self.call_sites.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CallGraph {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = CallGraphSerde::deserialize(deserializer)?;
        let mut graph = CallGraph {
            functions: Arena::from(data.functions),
            function_lookup: HashMap::new(),
            call_sites: data.call_sites,
        };
        for (idx, function) in graph.functions.iter_mut().enumerate() {
            function.rebuild_local_lookup();
            graph
                .function_lookup
                .insert(function.name.clone(), ArenaIndex::from(idx));
        }
        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        if let IRType::Function {
            parameters,
            return_type,
        } = sig
        {
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

    #[test]
    fn function_locals_display_deterministic() {
        let mut func_a = IRFunction::new("demo", IRType::Void);
        func_a.add_local(LocalVariable::new("beta", IRType::Integer));
        func_a.add_local(LocalVariable::new("alpha", IRType::Integer));

        let mut func_b = IRFunction::new("demo", IRType::Void);
        func_b.add_local(LocalVariable::new("alpha", IRType::Integer));
        func_b.add_local(LocalVariable::new("beta", IRType::Integer));

        let display_a = format!("{}", func_a);
        let display_b = format!("{}", func_b);
        assert_eq!(display_a, display_b);
    }
}
