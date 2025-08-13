//! IR module system for the Seen programming language

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};
use crate::function::{IRFunction, CallGraph};
use crate::value::{IRValue, IRType};

/// Module visibility levels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Internal, // Visible within the same crate/package
}

/// Module-level constant
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleConstant {
    pub name: String,
    pub value: IRValue,
    pub const_type: IRType,
    pub visibility: Visibility,
}

impl ModuleConstant {
    pub fn new(name: impl Into<String>, value: IRValue, const_type: IRType) -> Self {
        Self {
            name: name.into(),
            value,
            const_type,
            visibility: Visibility::Private,
        }
    }
    
    pub fn public(mut self) -> Self {
        self.visibility = Visibility::Public;
        self
    }
    
    pub fn internal(mut self) -> Self {
        self.visibility = Visibility::Internal;
        self
    }
}

/// Global variable in a module
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalVariable {
    pub name: String,
    pub var_type: IRType,
    pub initial_value: Option<IRValue>,
    pub is_mutable: bool,
    pub visibility: Visibility,
    pub is_thread_local: bool,
}

impl GlobalVariable {
    pub fn new(name: impl Into<String>, var_type: IRType) -> Self {
        Self {
            name: name.into(),
            var_type,
            initial_value: None,
            is_mutable: false,
            visibility: Visibility::Private,
            is_thread_local: false,
        }
    }
    
    pub fn with_initial_value(mut self, value: IRValue) -> Self {
        self.initial_value = Some(value);
        self
    }
    
    pub fn mutable(mut self) -> Self {
        self.is_mutable = true;
        self
    }
    
    pub fn public(mut self) -> Self {
        self.visibility = Visibility::Public;
        self
    }
    
    pub fn thread_local(mut self) -> Self {
        self.is_thread_local = true;
        self
    }
}

/// Type definition in a module
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub name: String,
    pub type_def: IRType,
    pub visibility: Visibility,
    pub is_opaque: bool, // For forward declarations
}

impl TypeDefinition {
    pub fn new(name: impl Into<String>, type_def: IRType) -> Self {
        Self {
            name: name.into(),
            type_def,
            visibility: Visibility::Private,
            is_opaque: false,
        }
    }
    
    pub fn public(mut self) -> Self {
        self.visibility = Visibility::Public;
        self
    }
    
    pub fn opaque(mut self) -> Self {
        self.is_opaque = true;
        self
    }
}

/// Module import/export information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    pub module_path: String,
    pub items: Vec<String>, // Empty means import all public items
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub item_name: String,
    pub alias: Option<String>,
}

/// IR Module representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRModule {
    pub name: String,
    pub path: Option<String>, // File path or module path
    pub functions: HashMap<String, IRFunction>,
    pub constants: HashMap<String, ModuleConstant>,
    pub global_variables: HashMap<String, GlobalVariable>,
    pub types: HashMap<String, TypeDefinition>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub dependencies: Vec<String>, // Module names this depends on
    pub version: Option<String>,
    pub metadata: HashMap<String, String>, // Arbitrary metadata
}

impl IRModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: None,
            functions: HashMap::new(),
            constants: HashMap::new(),
            global_variables: HashMap::new(),
            types: HashMap::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            dependencies: Vec::new(),
            version: None,
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
    
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
    
    pub fn add_function(&mut self, function: IRFunction) {
        self.functions.insert(function.name.clone(), function);
    }
    
    pub fn add_constant(&mut self, constant: ModuleConstant) {
        self.constants.insert(constant.name.clone(), constant);
    }
    
    pub fn add_global(&mut self, global: GlobalVariable) {
        self.global_variables.insert(global.name.clone(), global);
    }
    
    pub fn add_type(&mut self, type_def: TypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }
    
    pub fn add_import(&mut self, import: Import) {
        if !self.dependencies.contains(&import.module_path) {
            self.dependencies.push(import.module_path.clone());
        }
        self.imports.push(import);
    }
    
    pub fn add_export(&mut self, export: Export) {
        self.exports.push(export);
    }
    
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
    
    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&IRFunction> {
        self.functions.get(name)
    }
    
    /// Get a mutable reference to a function
    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IRFunction> {
        self.functions.get_mut(name)
    }
    
    /// Get all public functions
    pub fn public_functions(&self) -> impl Iterator<Item = &IRFunction> {
        self.functions.values().filter(|f| f.is_public)
    }
    
    /// Get all exported items
    pub fn exported_items(&self) -> Vec<&str> {
        self.exports.iter().map(|e| e.item_name.as_str()).collect()
    }
    
    /// Check if an item is exported
    pub fn is_exported(&self, item_name: &str) -> bool {
        self.exports.iter().any(|e| e.item_name == item_name)
    }
    
    /// Validate the module
    pub fn validate(&self) -> Result<(), String> {
        // Check that all exported items exist
        for export in &self.exports {
            let exists = self.functions.contains_key(&export.item_name) ||
                        self.constants.contains_key(&export.item_name) ||
                        self.global_variables.contains_key(&export.item_name) ||
                        self.types.contains_key(&export.item_name);
            
            if !exists {
                return Err(format!("Exported item '{}' does not exist in module", export.item_name));
            }
        }
        
        // Validate all functions
        for function in self.functions.values() {
            if let Err(e) = function.validate() {
                return Err(format!("Function '{}' validation failed: {}", function.name, e));
            }
        }
        
        // Check for duplicate names across different namespaces
        let mut all_names = std::collections::HashSet::new();
        
        for name in self.functions.keys() {
            if !all_names.insert(name.clone()) {
                return Err(format!("Duplicate name '{}' in module", name));
            }
        }
        
        for name in self.constants.keys() {
            if !all_names.insert(name.clone()) {
                return Err(format!("Duplicate name '{}' in module", name));
            }
        }
        
        for name in self.global_variables.keys() {
            if !all_names.insert(name.clone()) {
                return Err(format!("Duplicate name '{}' in module", name));
            }
        }
        
        for name in self.types.keys() {
            if !all_names.insert(name.clone()) {
                return Err(format!("Duplicate name '{}' in module", name));
            }
        }
        
        Ok(())
    }
    
    /// Get the call graph for this module
    pub fn call_graph(&self) -> CallGraph {
        let mut graph = CallGraph::new();
        
        // Add all functions to the graph
        for function in self.functions.values() {
            graph.add_function(function.clone());
        }
        
        // TODO: Analyze functions to build call sites
        // This would require analyzing the CFG of each function
        
        graph
    }
    
    /// Get module statistics
    pub fn statistics(&self) -> ModuleStatistics {
        let mut stats = ModuleStatistics::default();
        
        stats.function_count = self.functions.len();
        stats.constant_count = self.constants.len();
        stats.global_count = self.global_variables.len();
        stats.type_count = self.types.len();
        stats.import_count = self.imports.len();
        stats.export_count = self.exports.len();
        
        // Calculate code size (approximate)
        for function in self.functions.values() {
            for block in function.cfg.blocks.values() {
                stats.instruction_count += block.instructions.len();
                if block.terminator.is_some() {
                    stats.instruction_count += 1;
                }
            }
        }
        
        stats
    }
}

/// Module compilation statistics
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModuleStatistics {
    pub function_count: usize,
    pub constant_count: usize,
    pub global_count: usize,
    pub type_count: usize,
    pub import_count: usize,
    pub export_count: usize,
    pub instruction_count: usize,
}

impl fmt::Display for IRModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; Module: {}", self.name)?;
        
        if let Some(path) = &self.path {
            writeln!(f, "; Path: {}", path)?;
        }
        
        if let Some(version) = &self.version {
            writeln!(f, "; Version: {}", version)?;
        }
        
        if !self.dependencies.is_empty() {
            writeln!(f, "; Dependencies: {}", self.dependencies.join(", "))?;
        }
        
        // Imports
        if !self.imports.is_empty() {
            writeln!(f, "\n; Imports")?;
            for import in &self.imports {
                write!(f, "import {}", import.module_path)?;
                if !import.items.is_empty() {
                    write!(f, " ({})", import.items.join(", "))?;
                }
                if let Some(alias) = &import.alias {
                    write!(f, " as {}", alias)?;
                }
                writeln!(f)?;
            }
        }
        
        // Type definitions
        if !self.types.is_empty() {
            writeln!(f, "\n; Type definitions")?;
            for type_def in self.types.values() {
                write!(f, "type {} = {}", type_def.name, type_def.type_def)?;
                if type_def.visibility == Visibility::Public {
                    write!(f, " [public]")?;
                }
                if type_def.is_opaque {
                    write!(f, " [opaque]")?;
                }
                writeln!(f)?;
            }
        }
        
        // Constants
        if !self.constants.is_empty() {
            writeln!(f, "\n; Constants")?;
            for constant in self.constants.values() {
                write!(f, "const {}: {} = {}", 
                       constant.name, constant.const_type, constant.value)?;
                if constant.visibility == Visibility::Public {
                    write!(f, " [public]")?;
                }
                writeln!(f)?;
            }
        }
        
        // Global variables
        if !self.global_variables.is_empty() {
            writeln!(f, "\n; Global variables")?;
            for global in self.global_variables.values() {
                write!(f, "global ")?;
                if global.is_mutable {
                    write!(f, "mut ")?;
                }
                write!(f, "{}: {}", global.name, global.var_type)?;
                if let Some(init) = &global.initial_value {
                    write!(f, " = {}", init)?;
                }
                if global.visibility == Visibility::Public {
                    write!(f, " [public]")?;
                }
                if global.is_thread_local {
                    write!(f, " [thread_local]")?;
                }
                writeln!(f)?;
            }
        }
        
        // Functions
        if !self.functions.is_empty() {
            writeln!(f, "\n; Functions")?;
            for function in self.functions.values() {
                writeln!(f, "{}", function)?;
            }
        }
        
        // Exports
        if !self.exports.is_empty() {
            writeln!(f, "\n; Exports")?;
            for export in &self.exports {
                write!(f, "export {}", export.item_name)?;
                if let Some(alias) = &export.alias {
                    write!(f, " as {}", alias)?;
                }
                writeln!(f)?;
            }
        }
        
        Ok(())
    }
}

/// Module linking information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkingInfo {
    pub module_name: String,
    pub exported_symbols: HashMap<String, String>, // name -> mangled name
    pub imported_symbols: HashMap<String, String>, // name -> expected mangled name
    pub dependencies: Vec<String>,
}

impl LinkingInfo {
    pub fn new(module_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            exported_symbols: HashMap::new(),
            imported_symbols: HashMap::new(),
            dependencies: Vec::new(),
        }
    }
    
    pub fn add_export(&mut self, name: impl Into<String>, mangled: impl Into<String>) {
        self.exported_symbols.insert(name.into(), mangled.into());
    }
    
    pub fn add_import(&mut self, name: impl Into<String>, mangled: impl Into<String>) {
        self.imported_symbols.insert(name.into(), mangled.into());
    }
    
    pub fn add_dependency(&mut self, dep: impl Into<String>) {
        let dep = dep.into();
        if !self.dependencies.contains(&dep) {
            self.dependencies.push(dep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::IRValue;

    #[test]
    fn test_module_creation() {
        let module = IRModule::new("test_module")
            .with_path("src/test.seen")
            .with_version("1.0.0");
        
        assert_eq!(module.name, "test_module");
        assert_eq!(module.path, Some("src/test.seen".to_string()));
        assert_eq!(module.version, Some("1.0.0".to_string()));
    }
    
    #[test]
    fn test_module_constants() {
        let mut module = IRModule::new("test");
        
        let constant = ModuleConstant::new(
            "PI",
            IRValue::Float(3.14159),
            IRType::Float
        ).public();
        
        module.add_constant(constant);
        
        assert!(module.constants.contains_key("PI"));
        assert_eq!(module.constants["PI"].visibility, Visibility::Public);
    }
    
    #[test]
    fn test_module_imports_exports() {
        let mut module = IRModule::new("test");
        
        let import = Import {
            module_path: "std.math".to_string(),
            items: vec!["sin".to_string(), "cos".to_string()],
            alias: None,
        };
        
        let export = Export {
            item_name: "my_function".to_string(),
            alias: Some("exported_func".to_string()),
        };
        
        module.add_import(import);
        module.add_export(export);
        
        assert_eq!(module.imports.len(), 1);
        assert_eq!(module.exports.len(), 1);
        assert!(module.dependencies.contains(&"std.math".to_string()));
    }
    
    #[test]
    fn test_global_variables() {
        let mut module = IRModule::new("test");
        
        let global = GlobalVariable::new("counter", IRType::Integer)
            .with_initial_value(IRValue::Integer(0))
            .mutable()
            .public()
            .thread_local();
        
        module.add_global(global);
        
        let global_ref = &module.global_variables["counter"];
        assert!(global_ref.is_mutable);
        assert_eq!(global_ref.visibility, Visibility::Public);
        assert!(global_ref.is_thread_local);
        assert_eq!(global_ref.initial_value, Some(IRValue::Integer(0)));
    }
    
    #[test]
    fn test_linking_info() {
        let mut linking = LinkingInfo::new("my_module");
        
        linking.add_export("my_func", "_seen_my_module_my_func");
        linking.add_import("external_func", "_external_func");
        linking.add_dependency("std");
        
        assert_eq!(linking.exported_symbols.len(), 1);
        assert_eq!(linking.imported_symbols.len(), 1);
        assert_eq!(linking.dependencies.len(), 1);
    }
}