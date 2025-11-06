//! IR module system for the Seen programming language

use crate::function::{CallGraph, IRFunction};
use crate::value::{IRType, IRValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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

/// Type alias definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeAlias {
    pub name: String,
    pub target: IRType,
    pub is_public: bool,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleMetadata {
    pub key: String,
    pub value: String,
}

/// IR Module representation
#[derive(Debug, Clone)]
pub struct IRModule {
    pub name: String,
    pub path: Option<String>, // File path or module path
    pub functions: Vec<IRFunction>,
    function_lookup: HashMap<String, u32>,
    pub constants: Vec<ModuleConstant>,
    constant_lookup: HashMap<String, u32>,
    pub global_variables: Vec<GlobalVariable>,
    global_lookup: HashMap<String, u32>,
    pub types: Vec<TypeDefinition>,
    type_lookup: HashMap<String, u32>,
    pub type_aliases: Vec<TypeAlias>,
    type_alias_lookup: HashMap<String, u32>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub dependencies: Vec<String>, // Module names this depends on
    pub version: Option<String>,
    pub metadata: Vec<ModuleMetadata>,
    metadata_lookup: HashMap<String, u32>,
}

#[derive(Serialize, Deserialize)]
struct IRModuleSerde {
    name: String,
    path: Option<String>,
    functions: Vec<IRFunction>,
    constants: Vec<ModuleConstant>,
    global_variables: Vec<GlobalVariable>,
    types: Vec<TypeDefinition>,
    type_aliases: Vec<TypeAlias>,
    imports: Vec<Import>,
    exports: Vec<Export>,
    dependencies: Vec<String>,
    version: Option<String>,
    metadata: Vec<ModuleMetadata>,
}

impl Serialize for IRModule {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IRModuleSerde {
            name: self.name.clone(),
            path: self.path.clone(),
            functions: self.functions.clone(),
            constants: self.constants.clone(),
            global_variables: self.global_variables.clone(),
            types: self.types.clone(),
            type_aliases: self.type_aliases.clone(),
            imports: self.imports.clone(),
            exports: self.exports.clone(),
            dependencies: self.dependencies.clone(),
            version: self.version.clone(),
            metadata: self.metadata.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for IRModule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = IRModuleSerde::deserialize(deserializer)?;
        let mut module = IRModule {
            name: data.name,
            path: data.path,
            functions: data.functions,
            function_lookup: HashMap::new(),
            constants: data.constants,
            constant_lookup: HashMap::new(),
            global_variables: data.global_variables,
            global_lookup: HashMap::new(),
            types: data.types,
            type_lookup: HashMap::new(),
            type_aliases: data.type_aliases,
            type_alias_lookup: HashMap::new(),
            imports: data.imports,
            exports: data.exports,
            dependencies: data.dependencies,
            version: data.version,
            metadata: data.metadata,
            metadata_lookup: HashMap::new(),
        };
        module.rebuild_indices();
        Ok(module)
    }
}

impl IRModule {
    pub fn new(name: impl Into<String>) -> Self {
        let module = Self {
            name: name.into(),
            path: None,
            functions: Vec::new(),
            function_lookup: HashMap::new(),
            constants: Vec::new(),
            constant_lookup: HashMap::new(),
            global_variables: Vec::new(),
            global_lookup: HashMap::new(),
            types: Vec::new(),
            type_lookup: HashMap::new(),
            type_aliases: Vec::new(),
            type_alias_lookup: HashMap::new(),
            imports: Vec::new(),
            exports: Vec::new(),
            dependencies: Vec::new(),
            version: None,
            metadata: Vec::new(),
            metadata_lookup: HashMap::new(),
        };
        module
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn add_function(&mut self, mut function: IRFunction) {
        function.rebuild_local_lookup();
        if let Some(index) = self.function_lookup.get(&function.name).cloned() {
            self.functions[index as usize] = function;
        } else {
            let index = self.functions.len() as u32;
            self.function_lookup.insert(function.name.clone(), index);
            self.functions.push(function);
        }
    }

    pub fn add_constant(&mut self, constant: ModuleConstant) {
        if let Some(index) = self.constant_lookup.get(&constant.name).cloned() {
            self.constants[index as usize] = constant;
        } else {
            let index = self.constants.len() as u32;
            self.constant_lookup.insert(constant.name.clone(), index);
            self.constants.push(constant);
        }
    }

    pub fn add_global(&mut self, global: GlobalVariable) {
        if let Some(index) = self.global_lookup.get(&global.name).cloned() {
            self.global_variables[index as usize] = global;
        } else {
            let index = self.global_variables.len() as u32;
            self.global_lookup.insert(global.name.clone(), index);
            self.global_variables.push(global);
        }
    }

    pub fn add_type(&mut self, type_def: TypeDefinition) {
        if let Some(index) = self.type_lookup.get(&type_def.name).cloned() {
            self.types[index as usize] = type_def;
        } else {
            let index = self.types.len() as u32;
            self.type_lookup.insert(type_def.name.clone(), index);
            self.types.push(type_def);
        }
    }

    pub fn add_type_alias(&mut self, type_alias: TypeAlias) {
        if let Some(index) = self.type_alias_lookup.get(&type_alias.name).cloned() {
            self.type_aliases[index as usize] = type_alias;
        } else {
            let index = self.type_aliases.len() as u32;
            self.type_alias_lookup
                .insert(type_alias.name.clone(), index);
            self.type_aliases.push(type_alias);
        }
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
        let key = key.into();
        let value = value.into();
        if let Some(index) = self.metadata_lookup.get(&key).cloned() {
            self.metadata[index as usize].value = value;
        } else {
            let index = self.metadata.len() as u32;
            self.metadata_lookup.insert(key.clone(), index);
            self.metadata.push(ModuleMetadata { key, value });
        }
    }

    pub fn metadata_iter(&self) -> impl Iterator<Item = &ModuleMetadata> {
        self.metadata.iter()
    }

    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.metadata_lookup
            .get(key)
            .and_then(|idx| self.metadata.get(*idx as usize))
            .map(|meta| meta.value.as_str())
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&IRFunction> {
        self.function_lookup
            .get(name)
            .and_then(|index| self.functions.get(*index as usize))
    }

    /// Get a mutable reference to a function
    pub fn get_function_mut(&mut self, name: &str) -> Option<&mut IRFunction> {
        if let Some(index) = self.function_lookup.get(name).cloned() {
            self.functions.get_mut(index as usize)
        } else {
            None
        }
    }

    /// Get all public functions
    pub fn public_functions(&self) -> impl Iterator<Item = &IRFunction> {
        self.functions.iter().filter(|f| f.is_public)
    }

    /// Returns true if the module contains a function with the given name.
    pub fn has_function(&self, name: &str) -> bool {
        self.function_lookup.contains_key(name)
    }

    /// Iterate over all functions immutably.
    pub fn functions_iter(&self) -> impl Iterator<Item = &IRFunction> {
        self.functions.iter()
    }

    /// Iterate over all functions mutably.
    pub fn functions_iter_mut(&mut self) -> impl Iterator<Item = &mut IRFunction> {
        self.functions.iter_mut()
    }

    /// Get constant by name.
    pub fn get_constant(&self, name: &str) -> Option<&ModuleConstant> {
        self.constant_lookup
            .get(name)
            .and_then(|index| self.constants.get(*index as usize))
    }

    /// Returns true if a constant exists.
    pub fn has_constant(&self, name: &str) -> bool {
        self.constant_lookup.contains_key(name)
    }

    /// Get mutable constant by name.
    pub fn get_constant_mut(&mut self, name: &str) -> Option<&mut ModuleConstant> {
        if let Some(index) = self.constant_lookup.get(name).cloned() {
            self.constants.get_mut(index as usize)
        } else {
            None
        }
    }

    /// Get global variable by name.
    pub fn get_global(&self, name: &str) -> Option<&GlobalVariable> {
        self.global_lookup
            .get(name)
            .and_then(|index| self.global_variables.get(*index as usize))
    }

    /// Returns true if a global exists.
    pub fn has_global(&self, name: &str) -> bool {
        self.global_lookup.contains_key(name)
    }

    /// Get mutable global variable by name.
    pub fn get_global_mut(&mut self, name: &str) -> Option<&mut GlobalVariable> {
        if let Some(index) = self.global_lookup.get(name).cloned() {
            self.global_variables.get_mut(index as usize)
        } else {
            None
        }
    }

    /// Get all exported items
    pub fn exported_items(&self) -> Vec<&str> {
        self.exports.iter().map(|e| e.item_name.as_str()).collect()
    }

    /// Check if an item is exported
    pub fn is_exported(&self, item_name: &str) -> bool {
        self.exports.iter().any(|e| e.item_name == item_name)
    }

    /// Rebuild lookup tables from the current collections.
    pub fn rebuild_indices(&mut self) {
        self.function_lookup.clear();
        for (idx, function) in self.functions.iter_mut().enumerate() {
            function.rebuild_local_lookup();
            self.function_lookup
                .insert(function.name.clone(), idx as u32);
        }

        self.constant_lookup.clear();
        for (idx, constant) in self.constants.iter().enumerate() {
            self.constant_lookup
                .insert(constant.name.clone(), idx as u32);
        }

        self.global_lookup.clear();
        for (idx, global) in self.global_variables.iter().enumerate() {
            self.global_lookup.insert(global.name.clone(), idx as u32);
        }

        self.type_lookup.clear();
        for (idx, ty) in self.types.iter().enumerate() {
            self.type_lookup.insert(ty.name.clone(), idx as u32);
        }

        self.type_alias_lookup.clear();
        for (idx, alias) in self.type_aliases.iter().enumerate() {
            self.type_alias_lookup
                .insert(alias.name.clone(), idx as u32);
        }

        self.metadata_lookup.clear();
        for (idx, meta) in self.metadata.iter().enumerate() {
            self.metadata_lookup.insert(meta.key.clone(), idx as u32);
        }
    }

    /// Validate the module
    pub fn validate(&self) -> Result<(), String> {
        // Check that all exported items exist
        for export in &self.exports {
            let exists = self.function_lookup.contains_key(&export.item_name)
                || self.constant_lookup.contains_key(&export.item_name)
                || self.global_lookup.contains_key(&export.item_name)
                || self.type_lookup.contains_key(&export.item_name);

            if !exists {
                return Err(format!(
                    "Exported item '{}' does not exist in module",
                    export.item_name
                ));
            }
        }

        // Validate all functions
        for function in &self.functions {
            if let Err(e) = function.validate() {
                return Err(format!(
                    "Function '{}' validation failed: {}",
                    function.name, e
                ));
            }
        }

        // Check for duplicate names across different namespaces
        let mut all_names = std::collections::HashSet::new();

        for function in &self.functions {
            if !all_names.insert(function.name.clone()) {
                return Err(format!("Duplicate name '{}' in module", function.name));
            }
        }

        for constant in &self.constants {
            if !all_names.insert(constant.name.clone()) {
                return Err(format!("Duplicate name '{}' in module", constant.name));
            }
        }

        for global in &self.global_variables {
            if !all_names.insert(global.name.clone()) {
                return Err(format!("Duplicate name '{}' in module", global.name));
            }
        }

        for ty in &self.types {
            if !all_names.insert(ty.name.clone()) {
                return Err(format!("Duplicate name '{}' in module", ty.name));
            }
        }

        Ok(())
    }

    /// Get the call graph for this module
    pub fn call_graph(&self) -> CallGraph {
        let mut graph = CallGraph::new();

        // Add all functions to the graph
        for function in &self.functions {
            graph.add_function(function.clone());
        }

        // Analyze functions to build call sites and edges
        for caller_function in &self.functions {
            let caller_name = &caller_function.name;
            // Scan function CFG for call instructions
            for block in caller_function.cfg.blocks_iter() {
                let block_label = block.label.0.clone();
                for instruction in &block.instructions {
                    if let crate::instruction::Instruction::Call {
                        target,
                        args,
                        result,
                    } = instruction
                    {
                        if let crate::IRValue::GlobalVariable(callee_name) = target {
                            // Add call site if target function exists in this module
                            if self.function_lookup.contains_key(callee_name) {
                                let call_site = crate::function::CallSite {
                                    caller: caller_name.clone(),
                                    callee: callee_name.clone(),
                                    block_label: block_label.clone(),
                                    arguments: args.clone(),
                                    return_value: result.clone(),
                                    is_tail_call: false, // Would need more analysis to determine
                                };
                                graph.add_call_site(call_site);
                            }
                        }
                    }
                }
            }
        }

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
        for function in &self.functions {
            for block in function.cfg.blocks_iter() {
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
            let mut entries: Vec<&TypeDefinition> = self.types.iter().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for type_def in entries {
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
            let mut entries: Vec<&ModuleConstant> = self.constants.iter().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for constant in entries {
                write!(
                    f,
                    "const {}: {} = {}",
                    constant.name, constant.const_type, constant.value
                )?;
                if constant.visibility == Visibility::Public {
                    write!(f, " [public]")?;
                }
                writeln!(f)?;
            }
        }

        // Global variables
        if !self.global_variables.is_empty() {
            writeln!(f, "\n; Global variables")?;
            let mut entries: Vec<&GlobalVariable> = self.global_variables.iter().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for global in entries {
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
            let mut entries: Vec<&IRFunction> = self.functions.iter().collect();
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for function in entries {
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

        if !self.metadata.is_empty() {
            writeln!(f, "\n; Metadata")?;
            let mut entries: Vec<&ModuleMetadata> = self.metadata.iter().collect();
            entries.sort_by(|a, b| a.key.cmp(&b.key));
            for meta in entries {
                writeln!(f, "{} = {}", meta.key, meta.value)?;
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

        let constant = ModuleConstant::new("PI", IRValue::Float(3.14159), IRType::Float).public();

        module.add_constant(constant);

        assert!(module.has_constant("PI"));
        assert_eq!(
            module.get_constant("PI").unwrap().visibility,
            Visibility::Public
        );
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
    fn test_module_metadata_arena() {
        let mut module = IRModule::new("test");
        module.set_metadata("zeta", "2");
        module.set_metadata("alpha", "1");
        module.set_metadata("zeta", "3");

        let mut entries: Vec<_> = module
            .metadata_iter()
            .map(|m| (m.key.clone(), m.value.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            entries,
            vec![
                ("alpha".to_string(), "1".to_string()),
                ("zeta".to_string(), "3".to_string())
            ]
        );
        assert_eq!(module.metadata_value("zeta"), Some("3"));
        assert_eq!(module.metadata_value("missing"), None);

        let display = format!("{}", module);
        let meta_section = "\n; Metadata\nalpha = 1\nzeta = 3\n";
        assert!(display.contains(meta_section));
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

        let global_ref = module.get_global("counter").expect("global exists");
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
