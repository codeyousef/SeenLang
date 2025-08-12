//! Method resolution and dispatch system for Seen Language
//!
//! This module implements the method system with receiver syntax support,
//! method visibility checking, and efficient method dispatch resolution.

use std::collections::HashMap;
use seen_parser::ast::*;
use crate::types::{Method, MethodSignature, ReceiverType, MethodVisibility, MethodError};

/// Results of method resolution
#[derive(Debug, Clone)]
pub struct MethodResolutionResult {
    /// The resolved method if found
    pub method: Option<Method>,
    /// Any errors encountered during resolution
    pub errors: Vec<MethodError>,
    /// Alternative methods that were considered
    pub candidates: Vec<Method>,
}

impl MethodResolutionResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self {
            method: None,
            errors: Vec::new(),
            candidates: Vec::new(),
        }
    }
    
    /// Check if method resolution was successful
    pub fn is_success(&self) -> bool {
        self.method.is_some() && self.errors.is_empty()
    }
    
    /// Add an error to the result
    pub fn add_error(&mut self, error: MethodError) {
        self.errors.push(error);
    }
    
    /// Add a candidate method
    pub fn add_candidate(&mut self, method: Method) {
        self.candidates.push(method);
    }
    
    /// Set the resolved method
    pub fn set_method(&mut self, method: Method) {
        self.method = Some(method);
    }
}

impl Default for MethodResolutionResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for method resolution behavior
#[derive(Debug, Clone)]
pub struct MethodManagerConfig {
    /// Allow extension methods from other modules
    pub allow_extension_methods: bool,
    /// Enable method overloading resolution
    pub enable_method_overloading: bool,
    /// Strict visibility checking
    pub strict_visibility: bool,
    /// Cache method resolutions for performance
    pub enable_method_cache: bool,
}

impl Default for MethodManagerConfig {
    fn default() -> Self {
        Self {
            allow_extension_methods: true,
            enable_method_overloading: true,
            strict_visibility: true,
            enable_method_cache: true,
        }
    }
}

/// Main method manager that handles method resolution and dispatch
pub struct MethodManager {
    /// All known methods indexed by type name
    methods: HashMap<String, Vec<Method>>,
    /// Extension methods indexed by extended type
    extension_methods: HashMap<String, Vec<Method>>,
    /// Method resolution cache for performance
    method_cache: HashMap<String, MethodResolutionResult>,
    /// Configuration options
    config: MethodManagerConfig,
}

impl MethodManager {
    /// Create a new method manager with default configuration
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            extension_methods: HashMap::new(),
            method_cache: HashMap::new(),
            config: MethodManagerConfig::default(),
        }
    }
    
    /// Create a method manager with custom configuration
    pub fn with_config(config: MethodManagerConfig) -> Self {
        Self {
            methods: HashMap::new(),
            extension_methods: HashMap::new(),
            method_cache: HashMap::new(),
            config,
        }
    }
    
    /// Register a method with the manager
    pub fn register_method(&mut self, method: Method) -> Result<(), MethodError> {
        // Validate the method signature
        self.validate_method_signature(&method)?;
        
        // Check for conflicts if overloading is disabled
        if !self.config.enable_method_overloading {
            if let Some(existing_methods) = self.methods.get(&method.signature.receiver_type.type_name()) {
                if existing_methods.iter().any(|m| m.signature.name == method.signature.name) {
                    return Err(MethodError::MethodAlreadyExists {
                        type_name: method.signature.receiver_type.type_name(),
                        method_name: method.signature.name.clone(),
                        position: method.position,
                    });
                }
            }
        }
        
        // Register the method
        match &method.signature.receiver_type {
            ReceiverType::SelfType { type_name } |
            ReceiverType::RefType { type_name } |
            ReceiverType::MutRefType { type_name } => {
                self.methods.entry(type_name.clone())
                    .or_insert_with(Vec::new)
                    .push(method);
            }
            ReceiverType::Extension { extended_type, .. } => {
                self.extension_methods.entry(extended_type.clone())
                    .or_insert_with(Vec::new)
                    .push(method);
            }
        }
        
        // Clear cache since we added a new method
        if self.config.enable_method_cache {
            self.method_cache.clear();
        }
        
        Ok(())
    }
    
    /// Resolve a method call to a specific method
    pub fn resolve_method(
        &mut self, 
        receiver_type: &str, 
        method_name: &str, 
        args: &[Type], 
        context: &ResolutionContext
    ) -> MethodResolutionResult {
        // Check cache first
        let cache_key = format!("{}::{}", receiver_type, method_name);
        if self.config.enable_method_cache {
            if let Some(cached_result) = self.method_cache.get(&cache_key) {
                return cached_result.clone();
            }
        }
        
        let mut result = MethodResolutionResult::new();
        
        // Step 1: Find all candidate methods
        let mut candidates = Vec::new();
        
        // Look for instance methods
        if let Some(instance_methods) = self.methods.get(receiver_type) {
            for method in instance_methods {
                if method.signature.name == method_name {
                    candidates.push(method.clone());
                }
            }
        }
        
        // Look for extension methods if enabled
        if self.config.allow_extension_methods {
            if let Some(ext_methods) = self.extension_methods.get(receiver_type) {
                for method in ext_methods {
                    if method.signature.name == method_name {
                        candidates.push(method.clone());
                    }
                }
            }
        }
        
        // Step 2: Filter by argument compatibility
        let compatible_methods: Vec<_> = candidates.into_iter()
            .filter(|method| self.is_signature_compatible(&method.signature, args))
            .collect();
        
        if compatible_methods.is_empty() {
            result.add_error(MethodError::MethodNotFound {
                type_name: receiver_type.to_string(),
                method_name: method_name.to_string(),
                position: context.call_position,
            });
            return result;
        }
        
        // Step 3: Check visibility for all compatible methods
        let visible_methods: Vec<_> = compatible_methods.into_iter()
            .filter(|method| self.is_method_visible(method, context))
            .collect();
        
        if visible_methods.is_empty() {
            result.add_error(MethodError::MethodNotVisible {
                type_name: receiver_type.to_string(),
                method_name: method_name.to_string(),
                position: context.call_position,
            });
            return result;
        }
        
        // Step 4: Resolve overloading if multiple methods remain
        let resolved_method = if visible_methods.len() == 1 {
            visible_methods.into_iter().next().unwrap()
        } else {
            match self.resolve_overloading(&visible_methods, args, context) {
                Ok(method) => method,
                Err(error) => {
                    result.add_error(error);
                    return result;
                }
            }
        };
        
        // Step 5: Final validation
        if let Err(error) = self.validate_method_call(&resolved_method, args, context) {
            result.add_error(error);
            return result;
        }
        
        result.set_method(resolved_method);
        
        // Cache the result
        if self.config.enable_method_cache {
            self.method_cache.insert(cache_key, result.clone());
        }
        
        result
    }
    
    /// Validate a method signature for correctness
    fn validate_method_signature(&self, method: &Method) -> Result<(), MethodError> {
        // Check receiver type is valid
        match &method.signature.receiver_type {
            ReceiverType::SelfType { type_name } => {
                if type_name.is_empty() {
                    return Err(MethodError::InvalidReceiver {
                        reason: "Receiver type name cannot be empty".to_string(),
                        position: method.position,
                    });
                }
            }
            ReceiverType::RefType { type_name } => {
                if type_name.is_empty() {
                    return Err(MethodError::InvalidReceiver {
                        reason: "Reference receiver type name cannot be empty".to_string(),
                        position: method.position,
                    });
                }
            }
            ReceiverType::MutRefType { type_name } => {
                if type_name.is_empty() {
                    return Err(MethodError::InvalidReceiver {
                        reason: "Mutable reference receiver type name cannot be empty".to_string(),
                        position: method.position,
                    });
                }
            }
            ReceiverType::Extension { extended_type, definer_module } => {
                if extended_type.is_empty() {
                    return Err(MethodError::InvalidReceiver {
                        reason: "Extended type name cannot be empty".to_string(),
                        position: method.position,
                    });
                }
                if definer_module.is_empty() {
                    return Err(MethodError::InvalidReceiver {
                        reason: "Extension definer module cannot be empty".to_string(),
                        position: method.position,
                    });
                }
            }
        }
        
        // Check method name is not empty
        if method.signature.name.is_empty() {
            return Err(MethodError::InvalidSignature {
                reason: "Method name cannot be empty".to_string(),
                position: method.position,
            });
        }
        
        // Check for reserved method names
        match method.signature.name.as_str() {
            "new" | "init" | "constructor" => {
                return Err(MethodError::ReservedMethodName {
                    name: method.signature.name.clone(),
                    position: method.position,
                });
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Check if a method signature is compatible with given arguments
    fn is_signature_compatible(&self, signature: &MethodSignature, args: &[Type]) -> bool {
        if signature.parameters.len() != args.len() {
            return false;
        }
        
        for (param, arg) in signature.parameters.iter().zip(args.iter()) {
            if !self.is_type_compatible(&param.type_annotation, arg) {
                return false;
            }
        }
        
        true
    }
    
    /// Check if two types are compatible for method calls
    fn is_type_compatible(&self, expected: &Type, actual: &Type) -> bool {
        // For now, require exact match - this could be extended for subtyping
        expected == actual
    }
    
    /// Check if a method is visible in the given context
    fn is_method_visible(&self, method: &Method, context: &ResolutionContext) -> bool {
        if !self.config.strict_visibility {
            return true;
        }
        
        match method.visibility {
            MethodVisibility::Public => true,
            MethodVisibility::Private => {
                // Private methods only visible within same type
                context.current_type.as_ref()
                    .map(|current| current == &method.signature.receiver_type.type_name())
                    .unwrap_or(false)
            }
            MethodVisibility::Module => {
                // Module methods visible within same module
                context.current_module.as_ref()
                    .map(|current| current == &method.module_path)
                    .unwrap_or(false)
            }
        }
    }
    
    /// Resolve method overloading by finding the best match
    fn resolve_overloading(&self, methods: &[Method], _args: &[Type], context: &ResolutionContext) -> Result<Method, MethodError> {
        // For now, just return the first method if overloading is enabled
        // This could be enhanced with more sophisticated resolution rules
        if self.config.enable_method_overloading && !methods.is_empty() {
            Ok(methods[0].clone())
        } else {
            Err(MethodError::AmbiguousMethodCall {
                candidates: methods.len(),
                position: context.call_position,
            })
        }
    }
    
    /// Validate a method call for correctness
    fn validate_method_call(
        &self, 
        method: &Method, 
        _args: &[Type], 
        context: &ResolutionContext
    ) -> Result<(), MethodError> {
        // Check receiver mutability requirements
        match &method.signature.receiver_type {
            ReceiverType::MutRefType { .. } => {
                if !context.receiver_is_mutable {
                    return Err(MethodError::ImmutableReceiver {
                        method_name: method.signature.name.clone(),
                        position: context.call_position,
                    });
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Get all methods for a given type
    pub fn get_methods_for_type(&self, type_name: &str) -> Vec<&Method> {
        let mut result = Vec::new();
        
        // Add instance methods
        if let Some(methods) = self.methods.get(type_name) {
            result.extend(methods.iter());
        }
        
        // Add extension methods if enabled
        if self.config.allow_extension_methods {
            if let Some(ext_methods) = self.extension_methods.get(type_name) {
                result.extend(ext_methods.iter());
            }
        }
        
        result
    }
    
    /// Clear the method resolution cache
    pub fn clear_cache(&mut self) {
        self.method_cache.clear();
    }
    
    /// Get current configuration
    pub fn config(&self) -> &MethodManagerConfig {
        &self.config
    }
    
    /// Update configuration
    pub fn set_config(&mut self, config: MethodManagerConfig) {
        self.config = config;
        // Clear cache when config changes
        self.clear_cache();
    }
}

impl Default for MethodManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Context information for method resolution
#[derive(Debug, Clone)]
pub struct ResolutionContext {
    /// Position of the method call
    pub call_position: seen_lexer::Position,
    /// Current type context (for visibility checking)
    pub current_type: Option<String>,
    /// Current module context (for visibility checking)
    pub current_module: Option<String>,
    /// Whether the receiver is mutable
    pub receiver_is_mutable: bool,
}

impl ResolutionContext {
    /// Create a new resolution context
    pub fn new(call_position: seen_lexer::Position) -> Self {
        Self {
            call_position,
            current_type: None,
            current_module: None,
            receiver_is_mutable: false,
        }
    }
    
    /// Set the current type context
    pub fn with_type(mut self, type_name: String) -> Self {
        self.current_type = Some(type_name);
        self
    }
    
    /// Set the current module context
    pub fn with_module(mut self, module_name: String) -> Self {
        self.current_module = Some(module_name);
        self
    }
    
    /// Set receiver mutability
    pub fn with_mutable_receiver(mut self, is_mutable: bool) -> Self {
        self.receiver_is_mutable = is_mutable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::ast::Type;
    use seen_lexer::Position;
    use crate::types::{Parameter, MethodVisibility, ReceiverType};
    
    #[test]
    fn test_method_manager_creation() {
        let manager = MethodManager::new();
        assert!(manager.config.allow_extension_methods);
        assert!(manager.config.enable_method_overloading);
    }
    
    #[test]
    fn test_method_registration() {
        let mut manager = MethodManager::new();
        
        let method = Method {
            signature: MethodSignature {
                name: "GetName".to_string(),
                receiver_type: ReceiverType::SelfType {
                    type_name: "Person".to_string()
                },
                parameters: Vec::new(),
                return_type: Type {
                    name: "String".to_string(),
                    is_nullable: false,
                    generics: Vec::new(),
                },
            },
            visibility: MethodVisibility::Public,
            module_path: "main".to_string(),
            position: Position::new(1, 1, 0),
        };
        
        let result = manager.register_method(method);
        assert!(result.is_ok());
        
        let methods = manager.get_methods_for_type("Person");
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].signature.name, "GetName");
    }
    
    #[test]
    fn test_method_resolution() {
        let mut manager = MethodManager::new();
        
        // Register a method
        let method = Method {
            signature: MethodSignature {
                name: "GetAge".to_string(),
                receiver_type: ReceiverType::SelfType {
                    type_name: "Person".to_string()
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
            position: Position::new(1, 1, 0),
        };
        
        manager.register_method(method).unwrap();
        
        // Resolve the method
        let context = ResolutionContext::new(Position::new(2, 1, 0));
        let result = manager.resolve_method("Person", "GetAge", &[], &context);
        
        assert!(result.is_success());
        assert!(result.method.is_some());
        assert_eq!(result.method.unwrap().signature.name, "GetAge");
    }
    
    #[test]
    fn test_method_not_found() {
        let mut manager = MethodManager::new();
        
        let context = ResolutionContext::new(Position::new(1, 1, 0));
        let result = manager.resolve_method("Person", "NonExistentMethod", &[], &context);
        
        assert!(!result.is_success());
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], MethodError::MethodNotFound { .. }));
    }
    
    #[test]
    fn test_extension_method() {
        let mut manager = MethodManager::new();
        
        // Register an extension method
        let method = Method {
            signature: MethodSignature {
                name: "IsEmpty".to_string(),
                receiver_type: ReceiverType::Extension {
                    extended_type: "String".to_string(),
                    definer_module: "extensions".to_string(),
                },
                parameters: Vec::new(),
                return_type: Type {
                    name: "Bool".to_string(),
                    is_nullable: false,
                    generics: Vec::new(),
                },
            },
            visibility: MethodVisibility::Public,
            module_path: "extensions".to_string(),
            position: Position::new(1, 1, 0),
        };
        
        manager.register_method(method).unwrap();
        
        // Resolve the extension method
        let context = ResolutionContext::new(Position::new(2, 1, 0));
        let result = manager.resolve_method("String", "IsEmpty", &[], &context);
        
        assert!(result.is_success());
        assert!(result.method.is_some());
    }
}