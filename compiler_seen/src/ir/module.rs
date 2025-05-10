// compiler_seen/src/ir/module.rs
// Defines a Module, the top-level IR container for a compilation unit.

use super::function::Function;
use super::types::IrType;

/// Represents a compilation unit (e.g., a single Seen file or a linked set of files).
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String, // Name of the module (e.g., file path or package name)
    pub functions: Vec<Function>, // Functions defined in this module
    pub global_variables: Vec<GlobalVariable>,
    // pub struct_declarations: Vec<StructDeclaration>, // Definitions of struct types used in this module
    pub target_triple: Option<String>, // e.g., "x86_64-unknown-linux-gnu"
    pub source_filename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GlobalVariable {
    pub name: String,
    pub ty: IrType, // Type of the global variable (must be sized)
    // pub initializer: Option<ConstantValue>, // Optional initializer (must be a constant)
    // pub linkage: LinkageType, // e.g., Public, Private, External
    // pub is_constant: bool, // If the global is a constant
    // pub thread_local: bool,
}

// enum LinkageType { Public, PrivateInternal, External, ... }

impl Module {
    pub fn new(name: String) -> Self {
        Module {
            name,
            functions: Vec::new(),
            global_variables: Vec::new(),
            // struct_declarations: Vec::new(),
            target_triple: None,
            source_filename: None,
        }
    }

    pub fn add_function(&mut self, func: Function) {
        self.functions.push(func);
    }

    pub fn add_global_variable(&mut self, global: GlobalVariable) {
        self.global_variables.push(global);
    }

    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }
}

// TODO:
// - Storing struct type definitions explicitly if they are not just anonymous types in IrType.
// - Storing metadata like target architecture, data layout.
// - Handling of external function declarations (signatures without bodies).
// - Global variable initializers (constant expressions).
// - Linkage types for functions and globals.
