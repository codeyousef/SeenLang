//! Type definitions for the object-oriented programming system

use seen_parser::ast::Type;
use seen_lexer::Position;
use serde::{Serialize, Deserialize};

/// Represents a method with its signature and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Method {
    /// Method signature including name, parameters, and return type
    pub signature: MethodSignature,
    /// Visibility level of the method
    pub visibility: MethodVisibility,
    /// Module path where the method is defined
    pub module_path: String,
    /// Position in source code where method is defined
    pub position: Position,
}

/// Method signature with receiver, parameters, and return type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodSignature {
    /// Name of the method
    pub name: String,
    /// Type of receiver (self, &self, &mut self, extension)
    pub receiver_type: ReceiverType,
    /// Method parameters
    pub parameters: Vec<Parameter>,
    /// Return type of the method
    pub return_type: Type,
}

/// Different types of method receivers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReceiverType {
    /// Self receiver: fun (self: Type) Method()
    SelfType { type_name: String },
    /// Immutable reference receiver: fun (self: &Type) Method()
    RefType { type_name: String },
    /// Mutable reference receiver: fun (self: &mut Type) Method()
    MutRefType { type_name: String },
    /// Extension method: fun (type: Type) Method() from another module
    Extension { 
        extended_type: String, 
        definer_module: String 
    },
}

impl ReceiverType {
    /// Get the type name that this receiver operates on
    pub fn type_name(&self) -> String {
        match self {
            ReceiverType::SelfType { type_name } => type_name.clone(),
            ReceiverType::RefType { type_name } => type_name.clone(),
            ReceiverType::MutRefType { type_name } => type_name.clone(),
            ReceiverType::Extension { extended_type, .. } => extended_type.clone(),
        }
    }
    
    /// Check if this receiver requires a mutable reference
    pub fn requires_mutable(&self) -> bool {
        matches!(self, ReceiverType::MutRefType { .. })
    }
    
    /// Check if this is an extension method
    pub fn is_extension(&self) -> bool {
        matches!(self, ReceiverType::Extension { .. })
    }
}

/// Method parameter information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub type_annotation: Type,
    /// Whether parameter has a default value
    pub has_default: bool,
    /// Whether parameter is mutable (inout)
    pub is_mutable: bool,
}

/// Visibility levels for methods
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MethodVisibility {
    /// Public method (capitalized name)
    Public,
    /// Private method (lowercase name, same type only)
    Private,
    /// Module-level method (visible within same module)
    Module,
}

impl MethodVisibility {
    /// Determine visibility from method name using Seen's capitalization rules
    pub fn from_name(name: &str) -> Self {
        if name.chars().next().map_or(false, |c| c.is_uppercase()) {
            MethodVisibility::Public
        } else {
            MethodVisibility::Private
        }
    }
}

/// Interface definition for type contracts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    /// Interface name
    pub name: String,
    /// Required method signatures
    pub required_methods: Vec<MethodSignature>,
    /// Optional default method implementations
    pub default_methods: Vec<Method>,
    /// Visibility of the interface
    pub visibility: MethodVisibility,
    /// Position where interface is defined
    pub position: Position,
}

/// Implementation of an interface for a specific type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceImplementation {
    /// Type that implements the interface
    pub implementing_type: String,
    /// Interface being implemented
    pub interface_name: String,
    /// Method implementations
    pub method_implementations: Vec<Method>,
    /// Position where implementation is defined
    pub position: Position,
}

/// Extension method definition that adds methods to existing types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    /// Type being extended
    pub extended_type: String,
    /// Methods being added
    pub methods: Vec<Method>,
    /// Module defining the extension
    pub definer_module: String,
    /// Position where extension is defined
    pub position: Position,
}

/// Errors that can occur in the method system
#[derive(Debug, Clone, thiserror::Error)]
pub enum MethodError {
    /// Method not found for the given type and name
    #[error("Method '{method_name}' not found on type '{type_name}' at {position}")]
    MethodNotFound {
        type_name: String,
        method_name: String,
        position: Position,
    },
    
    /// Method exists but is not visible in current context
    #[error("Method '{method_name}' on type '{type_name}' is not visible at {position}")]
    MethodNotVisible {
        type_name: String,
        method_name: String,
        position: Position,
    },
    
    /// Method already exists (when overloading is disabled)
    #[error("Method '{method_name}' already exists on type '{type_name}' at {position}")]
    MethodAlreadyExists {
        type_name: String,
        method_name: String,
        position: Position,
    },
    
    /// Invalid receiver type in method definition
    #[error("Invalid receiver in method definition: {reason} at {position}")]
    InvalidReceiver {
        reason: String,
        position: Position,
    },
    
    /// Invalid method signature
    #[error("Invalid method signature: {reason} at {position}")]
    InvalidSignature {
        reason: String,
        position: Position,
    },
    
    /// Trying to call mutable method on immutable receiver
    #[error("Cannot call method '{method_name}' on immutable receiver at {position}")]
    ImmutableReceiver {
        method_name: String,
        position: Position,
    },
    
    /// Ambiguous method call with multiple candidates
    #[error("Ambiguous method call with {candidates} candidates at {position}")]
    AmbiguousMethodCall {
        candidates: usize,
        position: Position,
    },
    
    /// Reserved method name used
    #[error("Method name '{name}' is reserved and cannot be used at {position}")]
    ReservedMethodName {
        name: String,
        position: Position,
    },
    
    /// Interface not found
    #[error("Interface '{interface_name}' not found at {position}")]
    InterfaceNotFound {
        interface_name: String,
        position: Position,
    },
    
    /// Interface implementation missing required methods
    #[error("Implementation of '{interface_name}' for '{type_name}' missing methods: {missing_methods:?} at {position}")]
    IncompleteImplementation {
        interface_name: String,
        type_name: String,
        missing_methods: Vec<String>,
        position: Position,
    },
    
    /// Conflicting method implementations
    #[error("Conflicting implementations of method '{method_name}' for type '{type_name}' at {position}")]
    ConflictingImplementation {
        type_name: String,
        method_name: String,
        position: Position,
    },
}

/// Method call information for type checking
#[derive(Debug, Clone)]
pub struct MethodCall {
    /// Receiver expression
    pub receiver: Box<seen_parser::ast::Expression>,
    /// Method name being called
    pub method_name: String,
    /// Arguments passed to the method
    pub arguments: Vec<seen_parser::ast::Expression>,
    /// Position of the method call
    pub position: Position,
}

/// Method dispatch information
#[derive(Debug, Clone)]
pub struct MethodDispatch {
    /// Resolved method to call
    pub method: Method,
    /// Type of dispatch (static, dynamic, extension)
    pub dispatch_type: DispatchType,
    /// Whether this is a virtual call
    pub is_virtual: bool,
}

/// Types of method dispatch
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchType {
    /// Static dispatch (compile-time resolved)
    Static,
    /// Dynamic dispatch (runtime resolved)
    Dynamic,
    /// Extension method dispatch
    Extension,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_receiver_type_name() {
        let self_receiver = ReceiverType::SelfType {
            type_name: "Person".to_string()
        };
        assert_eq!(self_receiver.type_name(), "Person");
        
        let ref_receiver = ReceiverType::RefType {
            type_name: "Account".to_string()
        };
        assert_eq!(ref_receiver.type_name(), "Account");
        
        let extension_receiver = ReceiverType::Extension {
            extended_type: "String".to_string(),
            definer_module: "extensions".to_string(),
        };
        assert_eq!(extension_receiver.type_name(), "String");
    }
    
    #[test]
    fn test_receiver_mutability() {
        let self_receiver = ReceiverType::SelfType {
            type_name: "Person".to_string()
        };
        assert!(!self_receiver.requires_mutable());
        
        let mut_ref_receiver = ReceiverType::MutRefType {
            type_name: "Account".to_string()
        };
        assert!(mut_ref_receiver.requires_mutable());
    }
    
    #[test]
    fn test_method_visibility_from_name() {
        assert_eq!(MethodVisibility::from_name("GetName"), MethodVisibility::Public);
        assert_eq!(MethodVisibility::from_name("getName"), MethodVisibility::Private);
        assert_eq!(MethodVisibility::from_name("UpdateAge"), MethodVisibility::Public);
        assert_eq!(MethodVisibility::from_name("updateAge"), MethodVisibility::Private);
    }
    
    #[test]
    fn test_extension_receiver_detection() {
        let extension = ReceiverType::Extension {
            extended_type: "String".to_string(),
            definer_module: "utils".to_string(),
        };
        assert!(extension.is_extension());
        
        let self_receiver = ReceiverType::SelfType {
            type_name: "Person".to_string()
        };
        assert!(!self_receiver.is_extension());
    }
}