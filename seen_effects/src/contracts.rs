//! Design by contract system for Seen Language
//!
//! This module implements contracts according to Seen's syntax design:
//! - fun Divide(a: Int, b: Int): Int requires b != 0 ensures result * b == a { return a / b }
//! - invariant for structs and classes
//! - pre/post-condition checking with proper error handling
//! - Formal verification support for critical functions
//! - Runtime contract validation with optimization modes

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fmt;
use seen_lexer::position::Position;
use seen_parser::ast::{Expression, Type, BinaryOperator};
use crate::types::{AsyncValue, AsyncError, AsyncResult};

/// Unique identifier for contracts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContractId(u64);

impl ContractId {
    /// Create a new contract ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Contract definition containing preconditions, postconditions, and invariants
#[derive(Debug, Clone)]
pub struct Contract {
    /// Unique contract identifier
    pub id: ContractId,
    /// Contract name
    pub name: String,
    /// Preconditions that must be true when function is called
    pub preconditions: Vec<Precondition>,
    /// Postconditions that must be true when function returns
    pub postconditions: Vec<Postcondition>,
    /// Invariants that must always be true
    pub invariants: Vec<Invariant>,
    /// Contract metadata
    pub metadata: ContractMetadata,
    /// Whether contract checking is enabled
    pub is_enabled: bool,
}

/// Precondition that must be satisfied before function execution
#[derive(Debug, Clone)]
pub struct Precondition {
    /// Unique precondition identifier
    pub id: PreconditionId,
    /// Precondition expression (e.g., b != 0)
    pub condition: Expression,
    /// Error message if precondition fails
    pub error_message: String,
    /// Precondition metadata
    pub metadata: ConditionMetadata,
}

/// Postcondition that must be satisfied after function execution
#[derive(Debug, Clone)]
pub struct Postcondition {
    /// Unique postcondition identifier
    pub id: PostconditionId,
    /// Postcondition expression (e.g., result * b == a)
    pub condition: Expression,
    /// Error message if postcondition fails
    pub error_message: String,
    /// Postcondition metadata
    pub metadata: ConditionMetadata,
}

/// Invariant that must always be true for a type or function
#[derive(Debug, Clone)]
pub struct Invariant {
    /// Unique invariant identifier
    pub id: InvariantId,
    /// Invariant expression
    pub condition: Expression,
    /// Error message if invariant fails
    pub error_message: String,
    /// Invariant scope (struct, class, function, global)
    pub scope: InvariantScope,
    /// Invariant metadata
    pub metadata: ConditionMetadata,
}

/// Unique identifier for preconditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreconditionId(u64);

impl PreconditionId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for postconditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostconditionId(u64);

impl PostconditionId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Unique identifier for invariants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvariantId(u64);

impl InvariantId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
    
    pub fn id(&self) -> u64 {
        self.0
    }
}

/// Scope where an invariant applies
#[derive(Debug, Clone, PartialEq)]
pub enum InvariantScope {
    /// Invariant applies to a specific struct
    Struct(String),
    /// Invariant applies to a specific class
    Class(String),
    /// Invariant applies to a specific function
    Function(String),
    /// Global invariant
    Global,
}

/// Metadata for contracts
#[derive(Debug, Clone)]
pub struct ContractMetadata {
    /// Position where contract is defined
    pub position: Position,
    /// Contract documentation
    pub documentation: Option<String>,
    /// Whether contract is public
    pub is_public: bool,
    /// Contract verification level
    pub verification_level: VerificationLevel,
    /// Performance impact estimate
    pub performance_impact: PerformanceImpact,
}

/// Metadata for individual conditions
#[derive(Debug, Clone)]
pub struct ConditionMetadata {
    /// Position where condition is defined
    pub position: Position,
    /// Condition documentation
    pub documentation: Option<String>,
    /// Condition complexity level
    pub complexity: ConditionComplexity,
    /// Whether condition can be statically verified
    pub is_statically_verifiable: bool,
}

/// Verification levels for contracts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VerificationLevel {
    /// No verification (contracts are comments)
    None,
    /// Runtime checking only
    Runtime,
    /// Static analysis where possible, runtime fallback
    Hybrid,
    /// Full formal verification required
    Formal,
}

/// Performance impact of contract checking
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceImpact {
    /// Minimal performance impact
    Minimal,
    /// Low performance impact
    Low,
    /// Moderate performance impact
    Moderate,
    /// High performance impact
    High,
    /// Very high performance impact
    VeryHigh,
}

/// Complexity levels for conditions
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionComplexity {
    /// Simple boolean expression
    Simple,
    /// Multiple conditions with logic operators
    Moderate,
    /// Complex expressions with function calls
    Complex,
    /// Very complex with loops or recursion
    VeryComplex,
}

/// Contract violation error
#[derive(Debug, Clone)]
pub struct ContractViolation {
    /// Type of violation
    pub violation_type: ViolationType,
    /// Contract that was violated
    pub contract_id: ContractId,
    /// Specific condition that failed
    pub condition_id: ConditionId,
    /// Error message
    pub message: String,
    /// Position where violation occurred
    pub position: Position,
    /// Values at time of violation
    pub context: ContractContext,
}

/// Types of contract violations
#[derive(Debug, Clone, PartialEq)]
pub enum ViolationType {
    /// Precondition failed
    PreconditionFailed,
    /// Postcondition failed
    PostconditionFailed,
    /// Invariant violated
    InvariantViolated,
}

/// Universal condition identifier
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionId {
    Precondition(PreconditionId),
    Postcondition(PostconditionId),
    Invariant(InvariantId),
}

/// Context information when contract violation occurs
#[derive(Debug, Clone)]
pub struct ContractContext {
    /// Function parameters at time of call
    pub parameters: HashMap<String, AsyncValue>,
    /// Return value (for postconditions)
    pub return_value: Option<AsyncValue>,
    /// Local variables in scope
    pub local_variables: HashMap<String, AsyncValue>,
    /// Stack trace
    pub stack_trace: Vec<String>,
}

/// Contract checking mode
#[derive(Debug, Clone, PartialEq)]
pub enum ContractMode {
    /// Contracts disabled (production mode)
    Disabled,
    /// Check preconditions only
    PreconditionsOnly,
    /// Check postconditions only
    PostconditionsOnly,
    /// Check all contracts
    All,
    /// Debug mode with detailed logging
    Debug,
}

/// Contract evaluation result
#[derive(Debug, Clone)]
pub enum ContractResult {
    /// Contract satisfied
    Satisfied,
    /// Contract violated
    Violated(ContractViolation),
    /// Contract evaluation failed
    EvaluationFailed(AsyncError),
}

/// Contract system for managing all contracts
#[derive(Debug)]
pub struct ContractSystem {
    /// All registered contracts
    contracts: HashMap<ContractId, Contract>,
    /// Contracts by function name for quick lookup
    function_contracts: HashMap<String, Vec<ContractId>>,
    /// Global invariants
    global_invariants: Vec<InvariantId>,
    /// Contract checking mode
    mode: ContractMode,
    /// Next available IDs
    next_contract_id: u64,
    next_precondition_id: u64,
    next_postcondition_id: u64,
    next_invariant_id: u64,
    /// System configuration
    config: ContractSystemConfig,
    /// Statistics
    stats: ContractSystemStats,
}

/// Configuration for contract system
#[derive(Debug, Clone)]
pub struct ContractSystemConfig {
    /// Maximum contract evaluation time
    pub max_evaluation_time_ms: u64,
    /// Enable contract optimization
    pub enable_optimization: bool,
    /// Enable static verification
    pub enable_static_verification: bool,
    /// Log all contract evaluations
    pub log_evaluations: bool,
    /// Fail fast on first violation
    pub fail_fast: bool,
}

impl Default for ContractSystemConfig {
    fn default() -> Self {
        Self {
            max_evaluation_time_ms: 1000,
            enable_optimization: true,
            enable_static_verification: true,
            log_evaluations: false,
            fail_fast: true,
        }
    }
}

/// Statistics for contract system
#[derive(Debug, Clone)]
pub struct ContractSystemStats {
    /// Total contracts registered
    pub total_contracts: usize,
    /// Total preconditions checked
    pub preconditions_checked: u64,
    /// Total postconditions checked
    pub postconditions_checked: u64,
    /// Total invariants checked
    pub invariants_checked: u64,
    /// Total violations found
    pub violations_found: u64,
    /// Total evaluation time
    pub total_evaluation_time_ms: u64,
    /// Contracts by verification level
    pub contracts_by_level: HashMap<VerificationLevel, usize>,
}

impl Default for ContractSystemStats {
    fn default() -> Self {
        Self {
            total_contracts: 0,
            preconditions_checked: 0,
            postconditions_checked: 0,
            invariants_checked: 0,
            violations_found: 0,
            total_evaluation_time_ms: 0,
            contracts_by_level: HashMap::new(),
        }
    }
}

impl Contract {
    /// Create a new contract
    pub fn new(name: String, position: Position) -> Self {
        let id = ContractId::new(rand::random());
        let is_public = name.chars().next().map_or(false, |c| c.is_uppercase());
        
        Self {
            id,
            name,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            invariants: Vec::new(),
            metadata: ContractMetadata {
                position,
                documentation: None,
                is_public,
                verification_level: VerificationLevel::Runtime,
                performance_impact: PerformanceImpact::Low,
            },
            is_enabled: true,
        }
    }
    
    /// Add a precondition
    pub fn add_precondition(&mut self, condition: Expression, error_message: String, position: Position) -> PreconditionId {
        let id = PreconditionId::new(rand::random());
        let precondition = Precondition {
            id,
            condition,
            error_message,
            metadata: ConditionMetadata {
                position,
                documentation: None,
                complexity: ConditionComplexity::Simple,
                is_statically_verifiable: false,
            },
        };
        self.preconditions.push(precondition);
        id
    }
    
    /// Add a postcondition
    pub fn add_postcondition(&mut self, condition: Expression, error_message: String, position: Position) -> PostconditionId {
        let id = PostconditionId::new(rand::random());
        let postcondition = Postcondition {
            id,
            condition,
            error_message,
            metadata: ConditionMetadata {
                position,
                documentation: None,
                complexity: ConditionComplexity::Simple,
                is_statically_verifiable: false,
            },
        };
        self.postconditions.push(postcondition);
        id
    }
    
    /// Add an invariant
    pub fn add_invariant(&mut self, condition: Expression, error_message: String, scope: InvariantScope, position: Position) -> InvariantId {
        let id = InvariantId::new(rand::random());
        let invariant = Invariant {
            id,
            condition,
            error_message,
            scope,
            metadata: ConditionMetadata {
                position,
                documentation: None,
                complexity: ConditionComplexity::Simple,
                is_statically_verifiable: false,
            },
        };
        self.invariants.push(invariant);
        id
    }
    
    /// Set verification level
    pub fn with_verification_level(mut self, level: VerificationLevel) -> Self {
        self.metadata.verification_level = level;
        self
    }
    
    /// Set performance impact
    pub fn with_performance_impact(mut self, impact: PerformanceImpact) -> Self {
        self.metadata.performance_impact = impact;
        self
    }
    
    /// Get contract signature for display
    pub fn signature(&self) -> String {
        let mut parts = Vec::new();
        
        if !self.preconditions.is_empty() {
            let requires: Vec<String> = self.preconditions.iter()
                .map(|p| format!("requires {}", expression_to_string(&p.condition)))
                .collect();
            parts.extend(requires);
        }
        
        if !self.postconditions.is_empty() {
            let ensures: Vec<String> = self.postconditions.iter()
                .map(|p| format!("ensures {}", expression_to_string(&p.condition)))
                .collect();
            parts.extend(ensures);
        }
        
        if !self.invariants.is_empty() {
            let invariants: Vec<String> = self.invariants.iter()
                .map(|i| format!("invariant {}", expression_to_string(&i.condition)))
                .collect();
            parts.extend(invariants);
        }
        
        if parts.is_empty() {
            format!("contract {}", self.name)
        } else {
            format!("contract {} {}", self.name, parts.join(" "))
        }
    }
}

impl ContractSystem {
    /// Create a new contract system
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
            function_contracts: HashMap::new(),
            global_invariants: Vec::new(),
            mode: ContractMode::All,
            next_contract_id: 1,
            next_precondition_id: 1,
            next_postcondition_id: 1,
            next_invariant_id: 1,
            config: ContractSystemConfig::default(),
            stats: ContractSystemStats::default(),
        }
    }
    
    /// Create contract system with custom configuration
    pub fn with_config(config: ContractSystemConfig) -> Self {
        Self {
            contracts: HashMap::new(),
            function_contracts: HashMap::new(),
            global_invariants: Vec::new(),
            mode: ContractMode::All,
            next_contract_id: 1,
            next_precondition_id: 1,
            next_postcondition_id: 1,
            next_invariant_id: 1,
            config,
            stats: ContractSystemStats::default(),
        }
    }
    
    /// Register a contract
    pub fn register_contract(&mut self, contract: Contract) -> Result<ContractId, AsyncError> {
        let contract_id = contract.id;
        let function_name = contract.name.clone();
        
        // Update statistics
        self.stats.total_contracts += 1;
        *self.stats.contracts_by_level.entry(contract.metadata.verification_level.clone()).or_insert(0) += 1;
        
        // Store contract
        self.contracts.insert(contract_id, contract);
        
        // Index by function name
        self.function_contracts.entry(function_name).or_insert_with(Vec::new).push(contract_id);
        
        Ok(contract_id)
    }
    
    /// Check preconditions before function call
    pub fn check_preconditions(
        &mut self,
        function_name: &str,
        parameters: HashMap<String, AsyncValue>,
        position: Position,
    ) -> Result<(), ContractViolation> {
        if self.mode == ContractMode::Disabled || self.mode == ContractMode::PostconditionsOnly {
            return Ok(());
        }
        
        let start_time = std::time::Instant::now();
        
        if let Some(contract_ids) = self.function_contracts.get(function_name) {
            for &contract_id in contract_ids {
                if let Some(contract) = self.contracts.get(&contract_id) {
                    if !contract.is_enabled {
                        continue;
                    }
                    
                    for precondition in &contract.preconditions {
                        let result = self.evaluate_condition(
                            &precondition.condition,
                            &parameters,
                            None,
                            &HashMap::new(),
                        );
                        
                        self.stats.preconditions_checked += 1;
                        
                        match result {
                            Ok(AsyncValue::Boolean(true)) => {
                                // Precondition satisfied
                                continue;
                            }
                            Ok(AsyncValue::Boolean(false)) => {
                                // Precondition violated
                                let violation = ContractViolation {
                                    violation_type: ViolationType::PreconditionFailed,
                                    contract_id,
                                    condition_id: ConditionId::Precondition(precondition.id),
                                    message: precondition.error_message.clone(),
                                    position,
                                    context: ContractContext {
                                        parameters: parameters.clone(),
                                        return_value: None,
                                        local_variables: HashMap::new(),
                                        stack_trace: vec![function_name.to_string()],
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                            Ok(_) => {
                                // Invalid condition result type
                                let violation = ContractViolation {
                                    violation_type: ViolationType::PreconditionFailed,
                                    contract_id,
                                    condition_id: ConditionId::Precondition(precondition.id),
                                    message: "Precondition must evaluate to boolean".to_string(),
                                    position,
                                    context: ContractContext {
                                        parameters: parameters.clone(),
                                        return_value: None,
                                        local_variables: HashMap::new(),
                                        stack_trace: vec![function_name.to_string()],
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                            Err(_) => {
                                // Evaluation error
                                let violation = ContractViolation {
                                    violation_type: ViolationType::PreconditionFailed,
                                    contract_id,
                                    condition_id: ConditionId::Precondition(precondition.id),
                                    message: "Failed to evaluate precondition".to_string(),
                                    position,
                                    context: ContractContext {
                                        parameters: parameters.clone(),
                                        return_value: None,
                                        local_variables: HashMap::new(),
                                        stack_trace: vec![function_name.to_string()],
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        self.stats.total_evaluation_time_ms += start_time.elapsed().as_millis() as u64;
        Ok(())
    }
    
    /// Check postconditions after function return
    pub fn check_postconditions(
        &mut self,
        function_name: &str,
        parameters: HashMap<String, AsyncValue>,
        return_value: AsyncValue,
        position: Position,
    ) -> Result<(), ContractViolation> {
        if self.mode == ContractMode::Disabled || self.mode == ContractMode::PreconditionsOnly {
            return Ok(());
        }
        
        let start_time = std::time::Instant::now();
        
        if let Some(contract_ids) = self.function_contracts.get(function_name) {
            for &contract_id in contract_ids {
                if let Some(contract) = self.contracts.get(&contract_id) {
                    if !contract.is_enabled {
                        continue;
                    }
                    
                    for postcondition in &contract.postconditions {
                        let result = self.evaluate_condition(
                            &postcondition.condition,
                            &parameters,
                            Some(&return_value),
                            &HashMap::new(),
                        );
                        
                        self.stats.postconditions_checked += 1;
                        
                        match result {
                            Ok(AsyncValue::Boolean(true)) => {
                                // Postcondition satisfied
                                continue;
                            }
                            Ok(AsyncValue::Boolean(false)) => {
                                // Postcondition violated
                                let violation = ContractViolation {
                                    violation_type: ViolationType::PostconditionFailed,
                                    contract_id,
                                    condition_id: ConditionId::Postcondition(postcondition.id),
                                    message: postcondition.error_message.clone(),
                                    position,
                                    context: ContractContext {
                                        parameters: parameters.clone(),
                                        return_value: Some(return_value.clone()),
                                        local_variables: HashMap::new(),
                                        stack_trace: vec![function_name.to_string()],
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                            Ok(_) | Err(_) => {
                                // Invalid result or evaluation error
                                let violation = ContractViolation {
                                    violation_type: ViolationType::PostconditionFailed,
                                    contract_id,
                                    condition_id: ConditionId::Postcondition(postcondition.id),
                                    message: "Failed to evaluate postcondition".to_string(),
                                    position,
                                    context: ContractContext {
                                        parameters: parameters.clone(),
                                        return_value: Some(return_value.clone()),
                                        local_variables: HashMap::new(),
                                        stack_trace: vec![function_name.to_string()],
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        self.stats.total_evaluation_time_ms += start_time.elapsed().as_millis() as u64;
        Ok(())
    }
    
    /// Check invariants
    pub fn check_invariants(
        &mut self,
        scope: &InvariantScope,
        context: HashMap<String, AsyncValue>,
        position: Position,
    ) -> Result<(), ContractViolation> {
        if self.mode == ContractMode::Disabled {
            return Ok(());
        }
        
        let start_time = std::time::Instant::now();
        
        // Check global invariants
        for &invariant_id in &self.global_invariants.clone() {
            if let Some(contract) = self.contracts.values().find(|c| c.invariants.iter().any(|i| i.id == invariant_id)) {
                if let Some(invariant) = contract.invariants.iter().find(|i| i.id == invariant_id) {
                    if invariant.scope == InvariantScope::Global || invariant.scope == *scope {
                        let result = self.evaluate_condition(
                            &invariant.condition,
                            &HashMap::new(),
                            None,
                            &context,
                        );
                        
                        self.stats.invariants_checked += 1;
                        
                        match result {
                            Ok(AsyncValue::Boolean(true)) => {
                                // Invariant satisfied
                                continue;
                            }
                            Ok(AsyncValue::Boolean(false)) => {
                                // Invariant violated
                                let violation = ContractViolation {
                                    violation_type: ViolationType::InvariantViolated,
                                    contract_id: contract.id,
                                    condition_id: ConditionId::Invariant(invariant.id),
                                    message: invariant.error_message.clone(),
                                    position,
                                    context: ContractContext {
                                        parameters: HashMap::new(),
                                        return_value: None,
                                        local_variables: context.clone(),
                                        stack_trace: Vec::new(),
                                    },
                                };
                                
                                self.stats.violations_found += 1;
                                
                                if self.config.fail_fast {
                                    return Err(violation);
                                }
                            }
                            Ok(_) | Err(_) => {
                                // Invalid result or evaluation error - continue checking
                                self.stats.violations_found += 1;
                            }
                        }
                    }
                }
            }
        }
        
        self.stats.total_evaluation_time_ms += start_time.elapsed().as_millis() as u64;
        Ok(())
    }
    
    /// Set contract checking mode
    pub fn set_mode(&mut self, mode: ContractMode) {
        self.mode = mode;
    }
    
    /// Get contract by ID
    pub fn get_contract(&self, contract_id: ContractId) -> Option<&Contract> {
        self.contracts.get(&contract_id)
    }
    
    /// Get contracts for function
    pub fn get_function_contracts(&self, function_name: &str) -> Vec<&Contract> {
        if let Some(contract_ids) = self.function_contracts.get(function_name) {
            contract_ids.iter()
                .filter_map(|&id| self.contracts.get(&id))
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get system statistics
    pub fn get_stats(&self) -> &ContractSystemStats {
        &self.stats
    }
    
    /// Evaluate a condition expression
    fn evaluate_condition(
        &self,
        condition: &Expression,
        parameters: &HashMap<String, AsyncValue>,
        return_value: Option<&AsyncValue>,
        local_variables: &HashMap<String, AsyncValue>,
    ) -> AsyncResult {
        match condition {
            Expression::IntegerLiteral { value, .. } => Ok(AsyncValue::Integer(*value)),
            Expression::FloatLiteral { value, .. } => Ok(AsyncValue::Float(*value)),
            Expression::BooleanLiteral { value, .. } => Ok(AsyncValue::Boolean(*value)),
            Expression::StringLiteral { value, .. } => Ok(AsyncValue::String(value.clone())),
            Expression::Identifier { name, .. } => {
                if name == "result" {
                    if let Some(value) = return_value {
                        Ok(value.clone())
                    } else {
                        Err(AsyncError::RuntimeError {
                            message: "Result not available in this context".to_string(),
                            position: Position::new(0, 0, 0),
                        })
                    }
                } else if let Some(value) = parameters.get(name) {
                    Ok(value.clone())
                } else if let Some(value) = local_variables.get(name) {
                    Ok(value.clone())
                } else {
                    Err(AsyncError::RuntimeError {
                        message: format!("Variable '{}' not found", name),
                        position: Position::new(0, 0, 0),
                    })
                }
            }
            Expression::BinaryOp { left, right, op, .. } => {
                let left_val = self.evaluate_condition(left, parameters, return_value, local_variables)?;
                let right_val = self.evaluate_condition(right, parameters, return_value, local_variables)?;
                
                match (left_val, right_val, op) {
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Equal) => {
                        Ok(AsyncValue::Boolean(a == b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::NotEqual) => {
                        Ok(AsyncValue::Boolean(a != b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Less) => {
                        Ok(AsyncValue::Boolean(a < b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::LessEqual) => {
                        Ok(AsyncValue::Boolean(a <= b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Greater) => {
                        Ok(AsyncValue::Boolean(a > b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::GreaterEqual) => {
                        Ok(AsyncValue::Boolean(a >= b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Add) => {
                        Ok(AsyncValue::Integer(a + b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Subtract) => {
                        Ok(AsyncValue::Integer(a - b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Multiply) => {
                        Ok(AsyncValue::Integer(a * b))
                    }
                    (AsyncValue::Integer(a), AsyncValue::Integer(b), BinaryOperator::Divide) => {
                        if b != 0 {
                            Ok(AsyncValue::Integer(a / b))
                        } else {
                            Err(AsyncError::RuntimeError {
                                message: "Division by zero".to_string(),
                                position: Position::new(0, 0, 0),
                            })
                        }
                    }
                    (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::And) => {
                        Ok(AsyncValue::Boolean(a && b))
                    }
                    (AsyncValue::Boolean(a), AsyncValue::Boolean(b), BinaryOperator::Or) => {
                        Ok(AsyncValue::Boolean(a || b))
                    }
                    _ => {
                        Err(AsyncError::RuntimeError {
                            message: "Unsupported operation in contract condition".to_string(),
                            position: Position::new(0, 0, 0),
                        })
                    }
                }
            }
            _ => {
                Err(AsyncError::RuntimeError {
                    message: "Unsupported expression in contract condition".to_string(),
                    position: Position::new(0, 0, 0),
                })
            }
        }
    }
}

impl Default for ContractSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Contract violation: {} at {}:{}:{}",
            self.message,
            self.position.line,
            self.position.column,
            self.position.offset
        )
    }
}

impl std::error::Error for ContractViolation {}

/// Helper function to convert expression to string for display
fn expression_to_string(expr: &Expression) -> String {
    match expr {
        Expression::IntegerLiteral { value, .. } => value.to_string(),
        Expression::FloatLiteral { value, .. } => value.to_string(),
        Expression::BooleanLiteral { value, .. } => value.to_string(),
        Expression::StringLiteral { value, .. } => format!("\"{}\"", value),
        Expression::Identifier { name, .. } => name.clone(),
        Expression::BinaryOp { left, right, op, .. } => {
            let op_str = match op {
                BinaryOperator::Add => "+",
                BinaryOperator::Subtract => "-",
                BinaryOperator::Multiply => "*",
                BinaryOperator::Divide => "/",
                BinaryOperator::Equal => "==",
                BinaryOperator::NotEqual => "!=",
                BinaryOperator::Less => "<",
                BinaryOperator::LessEqual => "<=",
                BinaryOperator::Greater => ">",
                BinaryOperator::GreaterEqual => ">=",
                BinaryOperator::And => "and",
                BinaryOperator::Or => "or",
                _ => "?",
            };
            format!("{} {} {}", expression_to_string(left), op_str, expression_to_string(right))
        }
        _ => "?".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_contract_creation() {
        let contract = Contract::new("TestContract".to_string(), Position::new(1, 1, 0));
        
        assert_eq!(contract.name, "TestContract");
        assert!(contract.metadata.is_public); // Capital T = public
        assert!(contract.preconditions.is_empty());
        assert!(contract.postconditions.is_empty());
        assert!(contract.invariants.is_empty());
    }
    
    #[test]
    fn test_contract_precondition() {
        let mut contract = Contract::new("Divide".to_string(), Position::new(1, 1, 0));
        
        // requires b != 0
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::Identifier {
                name: "b".to_string(),
                is_public: false,
                pos: Position::new(1, 10, 0),
            }),
            right: Box::new(Expression::IntegerLiteral {
                value: 0,
                pos: Position::new(1, 15, 0),
            }),
            op: BinaryOperator::NotEqual,
            pos: Position::new(1, 12, 0),
        };
        
        let precondition_id = contract.add_precondition(
            condition,
            "Division by zero not allowed".to_string(),
            Position::new(1, 1, 0),
        );
        
        assert_eq!(contract.preconditions.len(), 1);
        assert_eq!(contract.preconditions[0].id, precondition_id);
        assert_eq!(contract.preconditions[0].error_message, "Division by zero not allowed");
    }
    
    #[test]
    fn test_contract_postcondition() {
        let mut contract = Contract::new("Divide".to_string(), Position::new(1, 1, 0));
        
        // ensures result * b == a
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Identifier {
                    name: "result".to_string(),
                    is_public: false,
                    pos: Position::new(1, 10, 0),
                }),
                right: Box::new(Expression::Identifier {
                    name: "b".to_string(),
                    is_public: false,
                    pos: Position::new(1, 19, 0),
                }),
                op: BinaryOperator::Multiply,
                pos: Position::new(1, 17, 0),
            }),
            right: Box::new(Expression::Identifier {
                name: "a".to_string(),
                is_public: false,
                pos: Position::new(1, 24, 0),
            }),
            op: BinaryOperator::Equal,
            pos: Position::new(1, 21, 0),
        };
        
        let postcondition_id = contract.add_postcondition(
            condition,
            "Result must satisfy: result * b == a".to_string(),
            Position::new(1, 1, 0),
        );
        
        assert_eq!(contract.postconditions.len(), 1);
        assert_eq!(contract.postconditions[0].id, postcondition_id);
    }
    
    #[test]
    fn test_contract_system_registration() {
        let mut system = ContractSystem::new();
        let contract = Contract::new("TestFunction".to_string(), Position::new(1, 1, 0));
        let contract_id = contract.id;
        
        let result = system.register_contract(contract);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), contract_id);
        assert_eq!(system.stats.total_contracts, 1);
        assert!(system.get_contract(contract_id).is_some());
    }
    
    #[test]
    fn test_precondition_checking() {
        let mut system = ContractSystem::new();
        let mut contract = Contract::new("Divide".to_string(), Position::new(1, 1, 0));
        
        // requires b != 0
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::Identifier {
                name: "b".to_string(),
                is_public: false,
                pos: Position::new(1, 10, 0),
            }),
            right: Box::new(Expression::IntegerLiteral {
                value: 0,
                pos: Position::new(1, 15, 0),
            }),
            op: BinaryOperator::NotEqual,
            pos: Position::new(1, 12, 0),
        };
        
        contract.add_precondition(
            condition,
            "Division by zero not allowed".to_string(),
            Position::new(1, 1, 0),
        );
        
        system.register_contract(contract).unwrap();
        
        // Test with valid parameters
        let mut valid_params = HashMap::new();
        valid_params.insert("a".to_string(), AsyncValue::Integer(10));
        valid_params.insert("b".to_string(), AsyncValue::Integer(2));
        
        let result = system.check_preconditions("Divide", valid_params, Position::new(1, 1, 0));
        assert!(result.is_ok());
        
        // Test with invalid parameters
        let mut invalid_params = HashMap::new();
        invalid_params.insert("a".to_string(), AsyncValue::Integer(10));
        invalid_params.insert("b".to_string(), AsyncValue::Integer(0));
        
        let result = system.check_preconditions("Divide", invalid_params, Position::new(1, 1, 0));
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.violation_type, ViolationType::PreconditionFailed);
        assert_eq!(violation.message, "Division by zero not allowed");
    }
    
    #[test]
    fn test_postcondition_checking() {
        let mut system = ContractSystem::new();
        let mut contract = Contract::new("Divide".to_string(), Position::new(1, 1, 0));
        
        // ensures result * b == a
        let condition = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Identifier {
                    name: "result".to_string(),
                    is_public: false,
                    pos: Position::new(1, 10, 0),
                }),
                right: Box::new(Expression::Identifier {
                    name: "b".to_string(),
                    is_public: false,
                    pos: Position::new(1, 19, 0),
                }),
                op: BinaryOperator::Multiply,
                pos: Position::new(1, 17, 0),
            }),
            right: Box::new(Expression::Identifier {
                name: "a".to_string(),
                is_public: false,
                pos: Position::new(1, 24, 0),
            }),
            op: BinaryOperator::Equal,
            pos: Position::new(1, 21, 0),
        };
        
        contract.add_postcondition(
            condition,
            "Result must satisfy: result * b == a".to_string(),
            Position::new(1, 1, 0),
        );
        
        system.register_contract(contract).unwrap();
        
        // Test with correct result
        let mut params = HashMap::new();
        params.insert("a".to_string(), AsyncValue::Integer(10));
        params.insert("b".to_string(), AsyncValue::Integer(2));
        
        let result = system.check_postconditions(
            "Divide",
            params.clone(),
            AsyncValue::Integer(5), // 10 / 2 = 5, and 5 * 2 == 10 ✓
            Position::new(1, 1, 0),
        );
        assert!(result.is_ok());
        
        // Test with incorrect result
        let result = system.check_postconditions(
            "Divide",
            params,
            AsyncValue::Integer(3), // 3 * 2 != 10 ✗
            Position::new(1, 1, 0),
        );
        assert!(result.is_err());
        
        let violation = result.unwrap_err();
        assert_eq!(violation.violation_type, ViolationType::PostconditionFailed);
    }
    
    #[test]
    fn test_contract_signature() {
        let mut contract = Contract::new("SafeDivide".to_string(), Position::new(1, 1, 0));
        
        // Add precondition: requires b != 0
        let pre_condition = Expression::BinaryOp {
            left: Box::new(Expression::Identifier {
                name: "b".to_string(),
                is_public: false,
                pos: Position::new(1, 10, 0),
            }),
            right: Box::new(Expression::IntegerLiteral {
                value: 0,
                pos: Position::new(1, 15, 0),
            }),
            op: BinaryOperator::NotEqual,
            pos: Position::new(1, 12, 0),
        };
        
        contract.add_precondition(
            pre_condition,
            "Division by zero not allowed".to_string(),
            Position::new(1, 1, 0),
        );
        
        // Add postcondition: ensures result * b == a
        let post_condition = Expression::BinaryOp {
            left: Box::new(Expression::BinaryOp {
                left: Box::new(Expression::Identifier {
                    name: "result".to_string(),
                    is_public: false,
                    pos: Position::new(1, 10, 0),
                }),
                right: Box::new(Expression::Identifier {
                    name: "b".to_string(),
                    is_public: false,
                    pos: Position::new(1, 19, 0),
                }),
                op: BinaryOperator::Multiply,
                pos: Position::new(1, 17, 0),
            }),
            right: Box::new(Expression::Identifier {
                name: "a".to_string(),
                is_public: false,
                pos: Position::new(1, 24, 0),
            }),
            op: BinaryOperator::Equal,
            pos: Position::new(1, 21, 0),
        };
        
        contract.add_postcondition(
            post_condition,
            "Result must be correct".to_string(),
            Position::new(1, 1, 0),
        );
        
        let signature = contract.signature();
        assert!(signature.contains("SafeDivide"));
        assert!(signature.contains("requires b != 0"));
        assert!(signature.contains("ensures result * b == a"));
    }
    
    #[test]
    fn test_contract_modes() {
        let mut system = ContractSystem::new();
        
        // Test disabled mode
        system.set_mode(ContractMode::Disabled);
        let result = system.check_preconditions("AnyFunction", HashMap::new(), Position::new(1, 1, 0));
        assert!(result.is_ok()); // Should always pass when disabled
        
        // Test preconditions only mode
        system.set_mode(ContractMode::PreconditionsOnly);
        let result = system.check_postconditions(
            "AnyFunction",
            HashMap::new(),
            AsyncValue::Unit,
            Position::new(1, 1, 0),
        );
        assert!(result.is_ok()); // Should pass when postconditions disabled
    }
    
    #[test]
    fn test_expression_to_string() {
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Identifier {
                name: "x".to_string(),
                is_public: false,
                pos: Position::new(1, 1, 0),
            }),
            right: Box::new(Expression::IntegerLiteral {
                value: 5,
                pos: Position::new(1, 5, 0),
            }),
            op: BinaryOperator::Greater,
            pos: Position::new(1, 3, 0),
        };
        
        let result = expression_to_string(&expr);
        assert_eq!(result, "x > 5");
    }
}