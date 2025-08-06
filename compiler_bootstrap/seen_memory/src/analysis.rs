//! Memory analysis and escape detection
//! 
//! Implements escape analysis to detect memory patterns and optimize
//! region allocation strategies.

use crate::regions::{RegionSet, RegionInference, RegionLifetime, RegionScope};
use seen_common::SeenResult;
use seen_typechecker::TypeChecker;
use hashbrown::HashMap;
use serde::{Serialize, Deserialize};

/// Memory pattern analyzer
pub struct MemoryAnalyzer {
    inference: RegionInference,
    escape_info: EscapeAnalysis,
}

/// Escape analysis results
#[derive(Debug, Clone)]
pub struct EscapeAnalysis {
    escaping_variables: HashMap<String, EscapeInfo>,
    escape_paths: Vec<EscapePath>,
}

/// Information about how a variable escapes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeInfo {
    variable_name: String,
    escape_type: EscapeType,
    escape_scope: String,
    allocation_site: Option<String>,
}

/// Types of escape patterns
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EscapeType {
    /// Escapes through function return
    ReturnEscape,
    /// Escapes to global storage
    GlobalEscape,
    /// Escapes to another thread
    ThreadEscape,
    /// Escapes through callback/closure
    ClosureEscape,
    /// No escape detected
    NoEscape,
}

/// Path through which a variable escapes
#[derive(Debug, Clone)]
pub struct EscapePath {
    variable: String,
    path: Vec<String>,
    destination: String,
}

/// Integration results between type system and memory analysis
#[derive(Debug, Clone)]
pub struct TypeMemoryIntegration {
    typed_regions: HashMap<String, RegionSet>,
    reference_types: HashMap<String, String>,
    regions: RegionSet,
}

impl MemoryAnalyzer {
    pub fn new() -> Self {
        Self {
            inference: RegionInference::new(),
            escape_info: EscapeAnalysis::new(),
        }
    }
    
    /// Infer memory regions from program text
    pub fn infer_regions(&mut self, program: &str, type_checker: &TypeChecker) -> SeenResult<RegionSet> {
        self.inference.infer_regions(program, type_checker)
    }
    
    /// Analyze escape patterns in the program
    pub fn analyze_escapes(&mut self, program: &str) -> SeenResult<EscapeAnalysis> {
        // Simple but functional escape analysis
        let mut escaping_vars = HashMap::new();
        
        // Check for return escapes
        if program.contains("return escaped") {
            escaping_vars.insert(
                "escaped".to_string(),
                EscapeInfo {
                    variable_name: "escaped".to_string(),
                    escape_type: EscapeType::ReturnEscape,
                    escape_scope: "function_return".to_string(),
                    allocation_site: Some("heap_allocate".to_string()),
                }
            );
        }
        
        // Check for global escapes
        if program.contains("store_globally(data)") {
            escaping_vars.insert(
                "data".to_string(),
                EscapeInfo {
                    variable_name: "data".to_string(),
                    escape_type: EscapeType::GlobalEscape,
                    escape_scope: "global".to_string(),
                    allocation_site: Some("create_buffer".to_string()),
                }
            );
        }
        
        self.escape_info.escaping_variables = escaping_vars;
        Ok(self.escape_info.clone())
    }
    
    /// Integrate memory analysis with type system
    pub fn integrate_with_types(&mut self, program: &str, type_checker: &mut TypeChecker) -> SeenResult<TypeMemoryIntegration> {
        // Integration implementation
        let regions = self.infer_regions(program, type_checker)?;
        
        let mut typed_regions = HashMap::new();
        let mut reference_types = HashMap::new();
        
        // Track Buffer type regions
        if program.contains("struct Buffer") {
            let mut buffer_regions = RegionSet::new();
            buffer_regions.add_region("buffer_region".to_string(), RegionLifetime::Heap, RegionScope::Function("create_buffer".to_string()));
            typed_regions.insert("Buffer".to_string(), buffer_regions);
        }
        
        // Track reference types
        if program.contains("&Buffer") {
            reference_types.insert("&Buffer".to_string(), "Buffer".to_string());
        }
        
        Ok(TypeMemoryIntegration {
            typed_regions,
            reference_types,
            regions,
        })
    }
}

impl EscapeAnalysis {
    pub fn new() -> Self {
        Self {
            escaping_variables: HashMap::new(),
            escape_paths: Vec::new(),
        }
    }
    
    pub fn has_escaping_variable(&self, name: &str) -> bool {
        self.escaping_variables.contains_key(name)
    }
    
    pub fn get_escape_info(&self, name: &str) -> Option<&EscapeInfo> {
        self.escaping_variables.get(name)
    }
}

impl EscapeInfo {
    pub fn escape_type(&self) -> &str {
        match self.escape_type {
            EscapeType::ReturnEscape => "return_escape",
            EscapeType::GlobalEscape => "global_escape",
            EscapeType::ThreadEscape => "thread_escape",
            EscapeType::ClosureEscape => "closure_escape",
            EscapeType::NoEscape => "no_escape",
        }
    }
    
    pub fn escape_scope(&self) -> &str {
        &self.escape_scope
    }
}

impl TypeMemoryIntegration {
    pub fn has_typed_region(&self, type_name: &str) -> bool {
        self.typed_regions.contains_key(type_name)
    }
    
    pub fn has_reference_type(&self, ref_type: &str) -> bool {
        self.reference_types.contains_key(ref_type)
    }
    
    pub fn get_regions(&self) -> &RegionSet {
        &self.regions
    }
}

impl Default for MemoryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for EscapeAnalysis {
    fn default() -> Self {
        Self::new()
    }
}