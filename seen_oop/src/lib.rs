//! Object-Oriented Programming features for Seen Language
//!
//! This crate provides comprehensive object-oriented programming support for Seen,
//! including method resolution, interface definitions, extension methods, and
//! polymorphic dispatch.
//!
//! ## Features
//!
//! - **Method System**: Receiver syntax methods with automatic resolution
//! - **Interface Contracts**: Interface definitions and implementations
//! - **Extension Methods**: Add methods to existing types from other modules
//! - **Method Overloading**: Multiple methods with same name, different signatures
//! - **Visibility Control**: Public/private/module visibility based on capitalization
//! - **Dynamic Dispatch**: Support for polymorphic method calls
//!
//! ## Example Usage
//!
//! ```rust
//! use seen_oop::{MethodManager, InterfaceManager, types::*};
//! use seen_lexer::Position;
//!
//! let mut method_manager = MethodManager::new();
//! let mut interface_manager = InterfaceManager::new();
//!
//! // Register interfaces and methods
//! // Resolve method calls
//! // Validate implementations
//! ```

pub mod method_manager;
pub mod interface_manager;
pub mod types;

// Re-export main types for convenience
pub use method_manager::{
    MethodManager, 
    MethodManagerConfig, 
    MethodResolutionResult, 
    ResolutionContext
};
pub use interface_manager::{
    InterfaceManager, 
    InterfaceManagerConfig, 
    InterfaceValidationResult
};
pub use types::{
    Method, 
    MethodSignature, 
    ReceiverType, 
    MethodVisibility, 
    Parameter,
    Interface, 
    InterfaceImplementation, 
    Extension,
    MethodError, 
    MethodCall, 
    MethodDispatch, 
    DispatchType
};

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::ast::Type;
    use seen_lexer::Position;
    
    #[test]
    fn test_integration() {
        let mut method_manager = MethodManager::new();
        let mut interface_manager = InterfaceManager::new();
        
        // Create a simple interface
        let interface = Interface {
            name: "Comparable".to_string(),
            required_methods: vec![
                MethodSignature {
                    name: "CompareTo".to_string(),
                    receiver_type: ReceiverType::SelfType { 
                        type_name: "Self".to_string() 
                    },
                    parameters: vec![
                        Parameter {
                            name: "other".to_string(),
                            type_annotation: Type {
                                name: "Self".to_string(),
                                is_nullable: false,
                                generics: Vec::new(),
                            },
                            has_default: false,
                            is_mutable: false,
                        }
                    ],
                    return_type: Type {
                        name: "Int".to_string(),
                        is_nullable: false,
                        generics: Vec::new(),
                    },
                }
            ],
            default_methods: Vec::new(),
            visibility: MethodVisibility::Public,
            position: Position::new(1, 1, 0),
        };
        
        let result = interface_manager.register_interface(interface);
        assert!(result.is_ok());
        
        // Create a method for a concrete type
        let method = Method {
            signature: MethodSignature {
                name: "GetValue".to_string(),
                receiver_type: ReceiverType::SelfType {
                    type_name: "Number".to_string()
                },
                parameters: Vec::new(),
                return_type: Type {
                    name: "Int".to_string(),
                    is_nullable: false,
                    generics: Vec::new(),
                },
            },
            visibility: MethodVisibility::Public,
            module_path: "main".to_string(),
            position: Position::new(2, 1, 0),
        };
        
        let result = method_manager.register_method(method);
        assert!(result.is_ok());
        
        // Test method resolution
        let context = ResolutionContext::new(Position::new(3, 1, 0));
        let resolution = method_manager.resolve_method("Number", "GetValue", &[], &context);
        
        assert!(resolution.is_success());
        assert!(resolution.method.is_some());
    }
    
    #[test]
    fn test_method_visibility() {
        assert_eq!(MethodVisibility::from_name("PublicMethod"), MethodVisibility::Public);
        assert_eq!(MethodVisibility::from_name("privateMethod"), MethodVisibility::Private);
    }
    
    #[test]
    fn test_receiver_types() {
        let self_receiver = ReceiverType::SelfType {
            type_name: "Person".to_string()
        };
        assert_eq!(self_receiver.type_name(), "Person");
        assert!(!self_receiver.requires_mutable());
        assert!(!self_receiver.is_extension());
        
        let mut_ref_receiver = ReceiverType::MutRefType {
            type_name: "Account".to_string()
        };
        assert!(mut_ref_receiver.requires_mutable());
        
        let extension_receiver = ReceiverType::Extension {
            extended_type: "String".to_string(),
            definer_module: "utils".to_string(),
        };
        assert!(extension_receiver.is_extension());
    }
}