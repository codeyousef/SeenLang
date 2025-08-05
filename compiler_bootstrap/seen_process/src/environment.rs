//! Environment variable management

use std::collections::HashMap;
use std::env;
use seen_common::{SeenResult, SeenError};

/// Environment variable representation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

impl EnvVar {
    /// Create a new environment variable
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Environment variable manager
pub struct Environment {
    vars: HashMap<String, String>,
    inherited: bool,
}

impl Environment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            inherited: false,
        }
    }
    
    /// Create an environment inheriting from the current process
    pub fn inherit() -> Self {
        let mut env = Self::new();
        env.inherited = true;
        
        // Copy current environment variables
        for (key, value) in env::vars() {
            env.vars.insert(key, value);
        }
        
        env
    }
    
    /// Get an environment variable
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(String::as_str)
    }
    
    /// Set an environment variable
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(name.into(), value.into());
    }
    
    /// Remove an environment variable
    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.vars.remove(name)
    }
    
    /// Clear all environment variables
    pub fn clear(&mut self) {
        self.vars.clear();
        self.inherited = false;
    }
    
    /// Get all environment variables
    pub fn vars(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
    
    /// Convert to a vector of key=value strings
    pub fn to_vec(&self) -> Vec<String> {
        self.vars
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect()
    }
    
    /// Apply this environment to the current process
    pub fn apply(&self) -> SeenResult<()> {
        // Clear existing environment if not inherited
        if !self.inherited {
            for (key, _) in env::vars() {
                env::remove_var(&key);
            }
        }
        
        // Set all variables
        for (key, value) in &self.vars {
            env::set_var(key, value);
        }
        
        Ok(())
    }
    
    /// Create a new environment from the current process environment
    pub fn from_current() -> Self {
        Self::inherit()
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the value of an environment variable
pub fn get_env(name: &str) -> Option<String> {
    env::var(name).ok()
}

/// Set an environment variable in the current process
pub fn set_env(name: &str, value: &str) -> SeenResult<()> {
    env::set_var(name, value);
    Ok(())
}

/// Remove an environment variable from the current process
pub fn remove_env(name: &str) -> SeenResult<()> {
    env::remove_var(name);
    Ok(())
}

/// Get all environment variables
pub fn all_env() -> HashMap<String, String> {
    env::vars().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_environment_creation() {
        let mut env = Environment::new();
        assert_eq!(env.get("PATH"), None);
        
        env.set("TEST_VAR", "test_value");
        assert_eq!(env.get("TEST_VAR"), Some("test_value"));
    }
    
    #[test]
    fn test_environment_inheritance() {
        // Set a test variable in the current process
        env::set_var("SEEN_TEST_INHERIT", "inherited_value");
        
        let env = Environment::inherit();
        assert_eq!(env.get("SEEN_TEST_INHERIT"), Some("inherited_value"));
        
        // Clean up
        env::remove_var("SEEN_TEST_INHERIT");
    }
    
    #[test]
    fn test_environment_modification() {
        let mut env = Environment::new();
        
        env.set("VAR1", "value1");
        env.set("VAR2", "value2");
        
        assert_eq!(env.get("VAR1"), Some("value1"));
        assert_eq!(env.get("VAR2"), Some("value2"));
        
        let removed = env.remove("VAR1");
        assert_eq!(removed, Some("value1".to_string()));
        assert_eq!(env.get("VAR1"), None);
        
        env.clear();
        assert_eq!(env.get("VAR2"), None);
    }
    
    #[test]
    fn test_environment_to_vec() {
        let mut env = Environment::new();
        env.set("A", "1");
        env.set("B", "2");
        
        let vec = env.to_vec();
        assert!(vec.contains(&"A=1".to_string()));
        assert!(vec.contains(&"B=2".to_string()));
    }
    
    #[test]
    fn test_env_helpers() {
        // Test setting and getting
        set_env("SEEN_TEST_HELPER", "helper_value").expect("Failed to set env var");
        assert_eq!(get_env("SEEN_TEST_HELPER"), Some("helper_value".to_string()));
        
        // Test removal
        remove_env("SEEN_TEST_HELPER").expect("Failed to remove env var");
        assert_eq!(get_env("SEEN_TEST_HELPER"), None);
    }
}