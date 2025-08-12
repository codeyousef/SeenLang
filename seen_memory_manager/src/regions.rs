//! Vale-style region-based memory management
//!
//! This module implements regions (memory arenas) that provide deterministic
//! memory management without garbage collection. Regions allow grouping related
//! objects and deallocating them together efficiently.

use std::collections::{HashMap, HashSet, VecDeque};
use seen_parser::ast::*;
use seen_lexer::Position;
use crate::ownership::{OwnershipInfo, OwnershipError};

/// Unique identifier for a memory region
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(u32);

impl RegionId {
    /// Create a new region ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    /// Get the numeric ID
    pub fn id(&self) -> u32 {
        self.0
    }
}

/// Represents a memory region that contains related allocations
#[derive(Debug, Clone)]
pub struct Region {
    /// Unique identifier for this region
    pub id: RegionId,
    /// Human-readable name for debugging
    pub name: String,
    /// Variables allocated in this region
    pub allocations: HashSet<String>,
    /// Child regions that are nested within this region
    pub child_regions: Vec<RegionId>,
    /// Parent region (if this is a nested region)
    pub parent_region: Option<RegionId>,
    /// Position where this region was created
    pub created_at: Position,
    /// Whether this region is still active
    pub is_active: bool,
}

impl Region {
    /// Create a new region
    pub fn new(id: RegionId, name: String, parent: Option<RegionId>, pos: Position) -> Self {
        Self {
            id,
            name,
            allocations: HashSet::new(),
            child_regions: Vec::new(),
            parent_region: parent,
            created_at: pos,
            is_active: true,
        }
    }
    
    /// Add an allocation to this region
    pub fn add_allocation(&mut self, variable: String) {
        self.allocations.insert(variable);
    }
    
    /// Remove an allocation from this region
    pub fn remove_allocation(&mut self, variable: &str) {
        self.allocations.remove(variable);
    }
    
    /// Add a child region
    pub fn add_child(&mut self, child_id: RegionId) {
        self.child_regions.push(child_id);
    }
    
    /// Mark region as inactive (deallocated)
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
}

/// Manages all memory regions in a program
#[derive(Debug)]
pub struct RegionManager {
    /// All regions indexed by their ID
    regions: HashMap<RegionId, Region>,
    /// Next available region ID
    next_region_id: u32,
    /// Stack of currently active regions (for nested scopes)
    region_stack: VecDeque<RegionId>,
    /// The global/root region
    global_region: RegionId,
    /// Errors encountered during region analysis
    errors: Vec<RegionError>,
}

impl RegionManager {
    /// Create a new region manager
    pub fn new() -> Self {
        let global_id = RegionId::new(0);
        let mut regions = HashMap::new();
        
        // Create the global region
        regions.insert(global_id, Region::new(
            global_id,
            "global".to_string(),
            None,
            Position::new(0, 0),
        ));
        
        let mut region_stack = VecDeque::new();
        region_stack.push_back(global_id);
        
        Self {
            regions,
            next_region_id: 1,
            region_stack,
            global_region: global_id,
            errors: Vec::new(),
        }
    }
    
    /// Create a new region as a child of the current region
    pub fn create_region(&mut self, name: String, pos: Position) -> RegionId {
        let region_id = RegionId::new(self.next_region_id);
        self.next_region_id += 1;
        
        let parent_id = self.current_region();
        let region = Region::new(region_id, name, Some(parent_id), pos);
        
        self.regions.insert(region_id, region);
        
        // Add as child to parent region
        if let Some(parent) = self.regions.get_mut(&parent_id) {
            parent.add_child(region_id);
        }
        
        region_id
    }
    
    /// Enter a region (push onto stack)
    pub fn enter_region(&mut self, region_id: RegionId) {
        self.region_stack.push_back(region_id);
    }
    
    /// Exit the current region (pop from stack)
    pub fn exit_region(&mut self) -> Option<RegionId> {
        if self.region_stack.len() > 1 {
            let exited = self.region_stack.pop_back();
            
            // Deactivate the region and all its children
            if let Some(region_id) = exited {
                self.deactivate_region_tree(region_id);
            }
            
            exited
        } else {
            None // Can't exit global region
        }
    }
    
    /// Get the currently active region
    pub fn current_region(&self) -> RegionId {
        *self.region_stack.back().unwrap_or(&self.global_region)
    }
    
    /// Allocate a variable in the current region
    pub fn allocate_in_current_region(&mut self, variable: String) {
        let current = self.current_region();
        if let Some(region) = self.regions.get_mut(&current) {
            region.add_allocation(variable);
        }
    }
    
    /// Allocate a variable in a specific region
    pub fn allocate_in_region(&mut self, variable: String, region_id: RegionId) -> Result<(), RegionError> {
        if let Some(region) = self.regions.get_mut(&region_id) {
            if !region.is_active {
                return Err(RegionError::RegionInactive {
                    region_id,
                    variable,
                    position: Position::new(0, 0), // Position tracked from allocation site
                });
            }
            region.add_allocation(variable);
            Ok(())
        } else {
            Err(RegionError::RegionNotFound {
                region_id,
                position: Position::new(0, 0), // Position tracked from allocation site
            })
        }
    }
    
    /// Deallocate a specific variable
    pub fn deallocate_variable(&mut self, variable: &str) {
        for region in self.regions.values_mut() {
            region.remove_allocation(variable);
        }
    }
    
    /// Recursively deactivate a region and all its children
    fn deactivate_region_tree(&mut self, region_id: RegionId) {
        if let Some(region) = self.regions.get(&region_id).cloned() {
            // First deactivate all children
            for child_id in &region.child_regions {
                self.deactivate_region_tree(*child_id);
            }
            
            // Then deactivate this region
            if let Some(region) = self.regions.get_mut(&region_id) {
                region.deactivate();
            }
        }
    }
    
    /// Get region information by ID
    pub fn get_region(&self, region_id: RegionId) -> Option<&Region> {
        self.regions.get(&region_id)
    }
    
    /// Check if a variable is allocated in any active region
    pub fn is_allocated(&self, variable: &str) -> bool {
        self.regions.values()
            .any(|region| region.is_active && region.allocations.contains(variable))
    }
    
    /// Find which region contains a variable
    pub fn find_variable_region(&self, variable: &str) -> Option<RegionId> {
        self.regions.iter()
            .find(|(_, region)| region.is_active && region.allocations.contains(variable))
            .map(|(id, _)| *id)
    }
    
    /// Get all active regions
    pub fn active_regions(&self) -> Vec<RegionId> {
        self.regions.iter()
            .filter(|(_, region)| region.is_active)
            .map(|(id, _)| *id)
            .collect()
    }
    
    /// Validate region hierarchy and detect issues
    pub fn validate_regions(&mut self) -> Vec<RegionError> {
        let mut errors = Vec::new();
        
        // Check for orphaned regions
        for (region_id, region) in &self.regions {
            if let Some(parent_id) = region.parent_region {
                if !self.regions.contains_key(&parent_id) {
                    errors.push(RegionError::OrphanedRegion {
                        region_id: *region_id,
                        missing_parent: parent_id,
                        position: region.created_at,
                    });
                }
            }
        }
        
        // Check for circular references
        for region_id in self.regions.keys() {
            if self.has_circular_reference(*region_id) {
                errors.push(RegionError::CircularReference {
                    region_id: *region_id,
                    position: self.regions[region_id].created_at,
                });
            }
        }
        
        errors
    }
    
    /// Check if a region has circular parent references
    fn has_circular_reference(&self, region_id: RegionId) -> bool {
        let mut visited = HashSet::new();
        let mut current = Some(region_id);
        
        while let Some(id) = current {
            if visited.contains(&id) {
                return true;
            }
            visited.insert(id);
            
            current = self.regions.get(&id)
                .and_then(|region| region.parent_region);
        }
        
        false
    }
    
    /// Get all errors encountered
    pub fn get_errors(&self) -> &[RegionError] {
        &self.errors
    }
    
    /// Clear all errors
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }
}

impl Default for RegionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during region management
#[derive(Debug, Clone)]
pub enum RegionError {
    /// Region not found
    RegionNotFound {
        region_id: RegionId,
        position: Position,
    },
    /// Trying to allocate in an inactive region
    RegionInactive {
        region_id: RegionId,
        variable: String,
        position: Position,
    },
    /// Region has lost its parent (orphaned)
    OrphanedRegion {
        region_id: RegionId,
        missing_parent: RegionId,
        position: Position,
    },
    /// Circular reference in region hierarchy
    CircularReference {
        region_id: RegionId,
        position: Position,
    },
    /// Variable allocated in multiple regions
    MultipleAllocation {
        variable: String,
        regions: Vec<RegionId>,
        position: Position,
    },
}

impl std::fmt::Display for RegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegionError::RegionNotFound { region_id, position } => {
                write!(f, "Region {:?} not found at {}", region_id, position)
            }
            RegionError::RegionInactive { region_id, variable, position } => {
                write!(f, "Cannot allocate variable '{}' in inactive region {:?} at {}", 
                       variable, region_id, position)
            }
            RegionError::OrphanedRegion { region_id, missing_parent, position } => {
                write!(f, "Region {:?} is orphaned (parent {:?} not found) at {}", 
                       region_id, missing_parent, position)
            }
            RegionError::CircularReference { region_id, position } => {
                write!(f, "Circular reference detected in region {:?} at {}", 
                       region_id, position)
            }
            RegionError::MultipleAllocation { variable, regions, position } => {
                write!(f, "Variable '{}' allocated in multiple regions {:?} at {}", 
                       variable, regions, position)
            }
        }
    }
}

impl std::error::Error for RegionError {}

/// Analyzes expressions to determine optimal region allocation
pub struct RegionAnalyzer {
    /// Region manager
    region_manager: RegionManager,
    /// Ownership information from previous analysis
    ownership_info: Option<OwnershipInfo>,
}

impl RegionAnalyzer {
    /// Create a new region analyzer
    pub fn new() -> Self {
        Self {
            region_manager: RegionManager::new(),
            ownership_info: None,
        }
    }
    
    /// Create analyzer with existing ownership information
    pub fn with_ownership(ownership_info: OwnershipInfo) -> Self {
        Self {
            region_manager: RegionManager::new(),
            ownership_info: Some(ownership_info),
        }
    }
    
    /// Analyze regions for a complete program
    pub fn analyze_program(&mut self, program: &Program) -> Result<RegionManager, RegionError> {
        for expression in &program.expressions {
            self.analyze_expression(expression)?;
        }
        
        // Validate the final region structure
        let errors = self.region_manager.validate_regions();
        if !errors.is_empty() {
            return Err(errors.into_iter().next().unwrap());
        }
        
        Ok(std::mem::take(&mut self.region_manager))
    }
    
    /// Analyze an expression for region allocation
    pub fn analyze_expression(&mut self, expr: &Expression) -> Result<(), RegionError> {
        match expr {
            // Variable bindings create allocations in current region
            Expression::Let { name, value, pos, .. } => {
                // First analyze the value expression
                self.analyze_expression(value)?;
                
                // Then allocate the variable in the current region
                self.region_manager.allocate_in_current_region(name.clone());
                
                Ok(())
            }
            
            // Blocks create new regions
            Expression::Block { expressions, pos } => {
                let region_id = self.region_manager.create_region(
                    format!("block_{}", pos.line),
                    *pos
                );
                
                self.region_manager.enter_region(region_id);
                
                for expr in expressions {
                    self.analyze_expression(expr)?;
                }
                
                self.region_manager.exit_region();
                Ok(())
            }
            
            // Function definitions create their own regions
            Expression::Function { name, body, pos, .. } => {
                let region_id = self.region_manager.create_region(
                    format!("function_{}", name),
                    *pos
                );
                
                self.region_manager.enter_region(region_id);
                self.analyze_expression(body)?;
                self.region_manager.exit_region();
                
                Ok(())
            }
            
            // Lambda expressions create temporary regions
            Expression::Lambda { body, pos, .. } => {
                let region_id = self.region_manager.create_region(
                    format!("lambda_{}", pos.line),
                    *pos
                );
                
                self.region_manager.enter_region(region_id);
                self.analyze_expression(body)?;
                self.region_manager.exit_region();
                
                Ok(())
            }
            
            // Other expressions recurse into sub-expressions
            Expression::BinaryOp { left, right, .. } => {
                self.analyze_expression(left)?;
                self.analyze_expression(right)?;
                Ok(())
            }
            
            Expression::UnaryOp { operand, .. } => {
                self.analyze_expression(operand)?;
                Ok(())
            }
            
            Expression::Call { callee, args, .. } => {
                self.analyze_expression(callee)?;
                for arg in args {
                    self.analyze_expression(arg)?;
                }
                Ok(())
            }
            
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.analyze_expression(condition)?;
                self.analyze_expression(then_branch)?;
                if let Some(else_expr) = else_branch {
                    self.analyze_expression(else_expr)?;
                }
                Ok(())
            }
            
            Expression::ArrayLiteral { elements, .. } => {
                for element in elements {
                    self.analyze_expression(element)?;
                }
                Ok(())
            }
            
            Expression::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.analyze_expression(value)?;
                }
                Ok(())
            }
            
            // Literals and simple expressions don't need region analysis
            _ => Ok(()),
        }
    }
    
    /// Get the region manager
    pub fn region_manager(&self) -> &RegionManager {
        &self.region_manager
    }
    
    /// Get mutable access to region manager
    pub fn region_manager_mut(&mut self) -> &mut RegionManager {
        &mut self.region_manager
    }
}

impl Default for RegionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_region_creation() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1);
        
        let region_id = manager.create_region("test_region".to_string(), pos);
        
        assert!(manager.get_region(region_id).is_some());
        assert_eq!(manager.get_region(region_id).unwrap().name, "test_region");
    }
    
    #[test]
    fn test_region_stack() {
        let mut manager = RegionManager::new();
        let global = manager.current_region();
        
        let pos = Position::new(1, 1);
        let region1 = manager.create_region("region1".to_string(), pos);
        manager.enter_region(region1);
        
        assert_eq!(manager.current_region(), region1);
        
        let exited = manager.exit_region();
        assert_eq!(exited, Some(region1));
        assert_eq!(manager.current_region(), global);
    }
    
    #[test]
    fn test_variable_allocation() {
        let mut manager = RegionManager::new();
        
        manager.allocate_in_current_region("x".to_string());
        
        assert!(manager.is_allocated("x"));
        assert!(manager.find_variable_region("x").is_some());
    }
    
    #[test]
    fn test_region_analyzer() {
        let mut analyzer = RegionAnalyzer::new();
        
        // Create a simple program: let x = 42
        let program = Program {
            expressions: vec![
                Expression::Let {
                    name: "x".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::IntegerLiteral {
                        value: 42,
                        pos: Position::new(1, 9),
                    }),
                    is_mutable: false,
                    pos: Position::new(1, 1),
                }
            ],
        };
        
        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        
        let region_manager = result.unwrap();
        assert!(region_manager.is_allocated("x"));
    }
    
    #[test]
    fn test_block_regions() {
        let mut analyzer = RegionAnalyzer::new();
        
        // Create a program with a block
        let program = Program {
            expressions: vec![
                Expression::Block {
                    expressions: vec![
                        Expression::Let {
                            name: "y".to_string(),
                            type_annotation: None,
                            value: Box::new(Expression::IntegerLiteral {
                                value: 10,
                                pos: Position::new(2, 5),
                            }),
                            is_mutable: false,
                            pos: Position::new(2, 1),
                        }
                    ],
                    pos: Position::new(1, 1),
                }
            ],
        };
        
        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        
        let region_manager = result.unwrap();
        
        // Variable should not be allocated anymore (block exited)
        assert!(!region_manager.is_allocated("y"));
    }
}