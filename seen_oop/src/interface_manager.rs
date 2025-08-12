//! Interface and implementation management for Seen Language
//!
//! This module handles interface definitions, implementations, and validation
//! to support polymorphism and contract-based programming.

use std::collections::HashMap;
use seen_lexer::Position;
use crate::types::{
    Interface, InterfaceImplementation, Method, MethodSignature, MethodError, 
    Parameter
};

/// Results of interface validation
#[derive(Debug)]
pub struct InterfaceValidationResult {
    /// Whether the implementation is valid
    pub is_valid: bool,
    /// Missing method implementations
    pub missing_methods: Vec<String>,
    /// Conflicting method signatures
    pub signature_conflicts: Vec<String>,
    /// Errors encountered during validation
    pub errors: Vec<MethodError>,
}

impl InterfaceValidationResult {
    /// Create a new validation result
    pub fn new() -> Self {
        Self {
            is_valid: true,
            missing_methods: Vec::new(),
            signature_conflicts: Vec::new(),
            errors: Vec::new(),
        }
    }
    
    /// Mark as invalid and add error
    pub fn add_error(&mut self, error: MethodError) {
        self.is_valid = false;
        self.errors.push(error);
    }
    
    /// Add missing method
    pub fn add_missing_method(&mut self, method_name: String) {
        self.is_valid = false;
        self.missing_methods.push(method_name);
    }
    
    /// Add signature conflict
    pub fn add_signature_conflict(&mut self, method_name: String) {
        self.is_valid = false;
        self.signature_conflicts.push(method_name);
    }
}

impl Default for InterfaceValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for interface management
#[derive(Debug, Clone)]
pub struct InterfaceManagerConfig {
    /// Allow multiple implementations of same interface for one type
    pub allow_multiple_implementations: bool,
    /// Strict signature matching for interface implementations
    pub strict_signature_matching: bool,
    /// Enable interface inheritance
    pub enable_interface_inheritance: bool,
    /// Allow default method implementations in interfaces
    pub allow_default_implementations: bool,
}

impl Default for InterfaceManagerConfig {
    fn default() -> Self {
        Self {
            allow_multiple_implementations: false,
            strict_signature_matching: true,
            enable_interface_inheritance: true,
            allow_default_implementations: true,
        }
    }
}

/// Manages interfaces and their implementations
pub struct InterfaceManager {
    /// All defined interfaces indexed by name
    interfaces: HashMap<String, Interface>,
    /// Interface implementations indexed by type name
    implementations: HashMap<String, Vec<InterfaceImplementation>>,
    /// Interface inheritance relationships (child -> parents)
    inheritance_graph: HashMap<String, Vec<String>>,
    /// Configuration options
    config: InterfaceManagerConfig,
}

impl InterfaceManager {
    /// Create a new interface manager
    pub fn new() -> Self {
        Self {
            interfaces: HashMap::new(),
            implementations: HashMap::new(),
            inheritance_graph: HashMap::new(),
            config: InterfaceManagerConfig::default(),
        }
    }
    
    /// Create interface manager with custom configuration
    pub fn with_config(config: InterfaceManagerConfig) -> Self {
        Self {
            interfaces: HashMap::new(),
            implementations: HashMap::new(),
            inheritance_graph: HashMap::new(),
            config,
        }
    }
    
    /// Register a new interface
    pub fn register_interface(&mut self, interface: Interface) -> Result<(), MethodError> {
        // Validate interface definition
        self.validate_interface_definition(&interface)?;
        
        // Check for naming conflicts
        if self.interfaces.contains_key(&interface.name) {
            return Err(MethodError::ConflictingImplementation {
                type_name: "interface".to_string(),
                method_name: interface.name.clone(),
                position: interface.position,
            });
        }
        
        // Register the interface
        self.interfaces.insert(interface.name.clone(), interface);
        
        Ok(())
    }
    
    /// Register an interface implementation for a type
    pub fn register_implementation(
        &mut self, 
        implementation: InterfaceImplementation
    ) -> Result<(), MethodError> {
        // Check if interface exists
        if !self.interfaces.contains_key(&implementation.interface_name) {
            return Err(MethodError::InterfaceNotFound {
                interface_name: implementation.interface_name.clone(),
                position: implementation.position,
            });
        }
        
        // Validate the implementation
        let validation_result = self.validate_implementation(&implementation);
        if !validation_result.is_valid {
            return Err(MethodError::IncompleteImplementation {
                interface_name: implementation.interface_name.clone(),
                type_name: implementation.implementing_type.clone(),
                missing_methods: validation_result.missing_methods,
                position: implementation.position,
            });
        }
        
        // Check for conflicts if multiple implementations not allowed
        if !self.config.allow_multiple_implementations {
            if let Some(existing_impls) = self.implementations.get(&implementation.implementing_type) {
                if existing_impls.iter().any(|impl_| impl_.interface_name == implementation.interface_name) {
                    return Err(MethodError::ConflictingImplementation {
                        type_name: implementation.implementing_type.clone(),
                        method_name: implementation.interface_name.clone(),
                        position: implementation.position,
                    });
                }
            }
        }
        
        // Register the implementation
        self.implementations.entry(implementation.implementing_type.clone())
            .or_insert_with(Vec::new)
            .push(implementation);
        
        Ok(())
    }
    
    /// Add interface inheritance relationship
    pub fn add_interface_inheritance(
        &mut self, 
        child_interface: String, 
        parent_interface: String
    ) -> Result<(), MethodError> {
        if !self.config.enable_interface_inheritance {
            return Err(MethodError::InvalidSignature {
                reason: "Interface inheritance is disabled".to_string(),
                position: Position::new(0, 0, 0),
            });
        }
        
        // Check both interfaces exist
        if !self.interfaces.contains_key(&child_interface) {
            return Err(MethodError::InterfaceNotFound {
                interface_name: child_interface,
                position: Position::new(0, 0, 0),
            });
        }
        
        if !self.interfaces.contains_key(&parent_interface) {
            return Err(MethodError::InterfaceNotFound {
                interface_name: parent_interface.clone(),
                position: Position::new(0, 0, 0),
            });
        }
        
        // Check for circular inheritance
        if self.would_create_cycle(&child_interface, &parent_interface) {
            return Err(MethodError::InvalidSignature {
                reason: format!("Circular inheritance detected: {} -> {}", child_interface, parent_interface),
                position: Position::new(0, 0, 0),
            });
        }
        
        // Add inheritance relationship
        self.inheritance_graph.entry(child_interface)
            .or_insert_with(Vec::new)
            .push(parent_interface);
        
        Ok(())
    }
    
    /// Get all interfaces implemented by a type
    pub fn get_interfaces_for_type(&self, type_name: &str) -> Vec<&Interface> {
        let mut result = Vec::new();
        
        if let Some(implementations) = self.implementations.get(type_name) {
            for impl_ in implementations {
                if let Some(interface) = self.interfaces.get(&impl_.interface_name) {
                    result.push(interface);
                    
                    // Add inherited interfaces if inheritance is enabled
                    if self.config.enable_interface_inheritance {
                        result.extend(self.get_inherited_interfaces(&impl_.interface_name));
                    }
                }
            }
        }
        
        result
    }
    
    /// Check if a type implements a specific interface
    pub fn implements_interface(&self, type_name: &str, interface_name: &str) -> bool {
        if let Some(implementations) = self.implementations.get(type_name) {
            // Direct implementation
            if implementations.iter().any(|impl_| impl_.interface_name == interface_name) {
                return true;
            }
            
            // Check inherited interfaces
            if self.config.enable_interface_inheritance {
                for impl_ in implementations {
                    if self.interface_inherits_from(&impl_.interface_name, interface_name) {
                        return true;
                    }
                }
            }
        }
        
        false
    }
    
    /// Get all methods required by interfaces for a type
    pub fn get_interface_methods(&self, type_name: &str) -> Vec<&MethodSignature> {
        let mut result = Vec::new();
        
        for interface in self.get_interfaces_for_type(type_name) {
            result.extend(interface.required_methods.iter());
        }
        
        result
    }
    
    /// Validate an interface definition
    fn validate_interface_definition(&self, interface: &Interface) -> Result<(), MethodError> {
        // Check interface name is not empty
        if interface.name.is_empty() {
            return Err(MethodError::InvalidSignature {
                reason: "Interface name cannot be empty".to_string(),
                position: interface.position,
            });
        }
        
        // Check method names are unique
        let mut method_names = std::collections::HashSet::new();
        for method in &interface.required_methods {
            if !method_names.insert(&method.name) {
                return Err(MethodError::ConflictingImplementation {
                    type_name: interface.name.clone(),
                    method_name: method.name.clone(),
                    position: interface.position,
                });
            }
        }
        
        // Check default methods don't conflict with required methods
        if self.config.allow_default_implementations {
            for default_method in &interface.default_methods {
                if interface.required_methods.iter().any(|req| req.name == default_method.signature.name) {
                    return Err(MethodError::ConflictingImplementation {
                        type_name: interface.name.clone(),
                        method_name: default_method.signature.name.clone(),
                        position: interface.position,
                    });
                }
            }
        }
        
        Ok(())
    }
    
    /// Validate an interface implementation
    fn validate_implementation(&self, implementation: &InterfaceImplementation) -> InterfaceValidationResult {
        let mut result = InterfaceValidationResult::new();
        
        let interface = match self.interfaces.get(&implementation.interface_name) {
            Some(interface) => interface,
            None => {
                result.add_error(MethodError::InterfaceNotFound {
                    interface_name: implementation.interface_name.clone(),
                    position: implementation.position,
                });
                return result;
            }
        };
        
        // Check all required methods are implemented
        for required_method in &interface.required_methods {
            let is_implemented = implementation.method_implementations
                .iter()
                .any(|impl_method| {
                    impl_method.signature.name == required_method.name &&
                    self.signatures_match(&impl_method.signature, required_method)
                });
            
            if !is_implemented {
                result.add_missing_method(required_method.name.clone());
            }
        }
        
        // Check for signature conflicts
        for impl_method in &implementation.method_implementations {
            if let Some(required_method) = interface.required_methods
                .iter()
                .find(|req| req.name == impl_method.signature.name) {
                if !self.signatures_match(&impl_method.signature, required_method) {
                    result.add_signature_conflict(impl_method.signature.name.clone());
                }
            }
        }
        
        result
    }
    
    /// Check if two method signatures match
    fn signatures_match(&self, impl_sig: &MethodSignature, required_sig: &MethodSignature) -> bool {
        if !self.config.strict_signature_matching {
            // Relaxed matching - just check name and parameter count
            return impl_sig.name == required_sig.name && 
                   impl_sig.parameters.len() == required_sig.parameters.len();
        }
        
        // Strict matching
        if impl_sig.name != required_sig.name {
            return false;
        }
        
        if impl_sig.parameters.len() != required_sig.parameters.len() {
            return false;
        }
        
        for (impl_param, req_param) in impl_sig.parameters.iter().zip(required_sig.parameters.iter()) {
            if !self.parameters_match(impl_param, req_param) {
                return false;
            }
        }
        
        // Check return types match
        impl_sig.return_type == required_sig.return_type
    }
    
    /// Check if two parameters match
    fn parameters_match(&self, impl_param: &Parameter, req_param: &Parameter) -> bool {
        impl_param.type_annotation == req_param.type_annotation &&
        impl_param.is_mutable == req_param.is_mutable
    }
    
    /// Check if adding inheritance would create a cycle
    fn would_create_cycle(&self, child: &str, parent: &str) -> bool {
        let mut visited = std::collections::HashSet::new();
        self.has_cycle_recursive(parent, child, &mut visited)
    }
    
    /// Recursive cycle detection
    fn has_cycle_recursive(&self, current: &str, target: &str, visited: &mut std::collections::HashSet<String>) -> bool {
        if current == target {
            return true;
        }
        
        if visited.contains(current) {
            return false;
        }
        
        visited.insert(current.to_string());
        
        if let Some(parents) = self.inheritance_graph.get(current) {
            for parent in parents {
                if self.has_cycle_recursive(parent, target, visited) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Get all interfaces inherited by an interface
    fn get_inherited_interfaces(&self, interface_name: &str) -> Vec<&Interface> {
        let mut result = Vec::new();
        
        if let Some(parents) = self.inheritance_graph.get(interface_name) {
            for parent_name in parents {
                if let Some(parent_interface) = self.interfaces.get(parent_name) {
                    result.push(parent_interface);
                    // Recursively get inherited interfaces
                    result.extend(self.get_inherited_interfaces(parent_name));
                }
            }
        }
        
        result
    }
    
    /// Check if an interface inherits from another
    fn interface_inherits_from(&self, child: &str, ancestor: &str) -> bool {
        if let Some(parents) = self.inheritance_graph.get(child) {
            for parent in parents {
                if parent == ancestor || self.interface_inherits_from(parent, ancestor) {
                    return true;
                }
            }
        }
        false
    }
    
    /// Get current configuration
    pub fn config(&self) -> &InterfaceManagerConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn set_config(&mut self, config: InterfaceManagerConfig) {
        self.config = config;
    }
}

impl Default for InterfaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ReceiverType, MethodVisibility};
    use seen_parser::ast::Type;
    use seen_lexer::Position;
    
    #[test]
    fn test_interface_registration() {
        let mut manager = InterfaceManager::new();
        
        let interface = Interface {
            name: "Drawable".to_string(),
            required_methods: vec![
                MethodSignature {
                    name: "Draw".to_string(),
                    receiver_type: ReceiverType::SelfType { type_name: "Self".to_string() },
                    parameters: Vec::new(),
                    return_type: Type {
                        name: "Unit".to_string(),
                        is_nullable: false,
                        generics: Vec::new(),
                    },
                }
            ],
            default_methods: Vec::new(),
            visibility: MethodVisibility::Public,
            position: Position::new(1, 1, 0),
        };
        
        let result = manager.register_interface(interface);
        assert!(result.is_ok());
        assert!(manager.interfaces.contains_key("Drawable"));
    }
    
    #[test]
    fn test_interface_implementation() {
        let mut manager = InterfaceManager::new();
        
        // Register interface first
        let interface = Interface {
            name: "Printable".to_string(),
            required_methods: vec![
                MethodSignature {
                    name: "Print".to_string(),
                    receiver_type: ReceiverType::SelfType { type_name: "Self".to_string() },
                    parameters: Vec::new(),
                    return_type: Type {
                        name: "Unit".to_string(),
                        is_nullable: false,
                        generics: Vec::new(),
                    },
                }
            ],
            default_methods: Vec::new(),
            visibility: MethodVisibility::Public,
            position: Position::new(1, 1, 0),
        };
        
        manager.register_interface(interface).unwrap();
        
        // Register implementation
        let implementation = InterfaceImplementation {
            implementing_type: "Document".to_string(),
            interface_name: "Printable".to_string(),
            method_implementations: vec![
                Method {
                    signature: MethodSignature {
                        name: "Print".to_string(),
                        receiver_type: ReceiverType::SelfType { type_name: "Document".to_string() },
                        parameters: Vec::new(),
                        return_type: Type {
                            name: "Unit".to_string(),
                            is_nullable: false,
                            generics: Vec::new(),
                        },
                    },
                    visibility: MethodVisibility::Public,
                    module_path: "main".to_string(),
                    position: Position::new(2, 1, 0),
                }
            ],
            position: Position::new(2, 1, 0),
        };
        
        let result = manager.register_implementation(implementation);
        assert!(result.is_ok());
        assert!(manager.implements_interface("Document", "Printable"));
    }
    
    #[test]
    fn test_incomplete_implementation() {
        let mut manager = InterfaceManager::new();
        
        // Register interface with required method
        let interface = Interface {
            name: "Serializable".to_string(),
            required_methods: vec![
                MethodSignature {
                    name: "Serialize".to_string(),
                    receiver_type: ReceiverType::SelfType { type_name: "Self".to_string() },
                    parameters: Vec::new(),
                    return_type: Type {
                        name: "String".to_string(),
                        is_nullable: false,
                        generics: Vec::new(),
                    },
                }
            ],
            default_methods: Vec::new(),
            visibility: MethodVisibility::Public,
            position: Position::new(1, 1, 0),
        };
        
        manager.register_interface(interface).unwrap();
        
        // Try to register incomplete implementation
        let implementation = InterfaceImplementation {
            implementing_type: "User".to_string(),
            interface_name: "Serializable".to_string(),
            method_implementations: Vec::new(), // No methods implemented!
            position: Position::new(2, 1, 0),
        };
        
        let result = manager.register_implementation(implementation);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MethodError::IncompleteImplementation { .. }));
    }
}