use crate::{
    handles::{GenerationalHandle, HybridGenerationalArena},
    ownership::OwnershipInfo,
};
use hashbrown::HashMap;
use seen_lexer::Position;
use seen_parser::ast::*;
use std::collections::{HashSet, VecDeque};

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

/// Handle that ties a generational slot to a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionHandle {
    region: RegionId,
    slot: GenerationalHandle,
}

impl RegionHandle {
    /// Create a handle for `region` and `slot`.
    pub fn new(region: RegionId, slot: GenerationalHandle) -> Self {
        Self { region, slot }
    }

    /// Region that owns the allocation.
    pub fn region(&self) -> RegionId {
        self.region
    }

    /// Raw generational slot.
    pub fn slot(&self) -> GenerationalHandle {
        self.slot
    }
}

/// Metadata stored for each allocation within a region.
#[derive(Debug, Clone)]
pub struct AllocationMetadata {
    /// Variable or symbol identifier tied to this allocation.
    pub variable: String,
    /// Source position where the allocation originated.
    pub created_at: Position,
}

/// Represents a memory region that contains related allocations
#[derive(Debug, Clone)]
pub struct Region {
    /// Unique identifier for this region
    pub id: RegionId,
    /// Human-readable name for debugging
    pub name: String,
    /// Strategy hint provided by the source program or compiler.
    pub strategy_hint: RegionStrategy,
    /// Strategy chosen after analysis (may differ from hint when hint is Auto).
    pub selected_strategy: RegionStrategy,
    /// Variables allocated in this region
    pub allocations: hashbrown::HashMap<String, RegionHandle>,
    /// Slot table that tracks allocation lifetimes
    pub allocation_table: HybridGenerationalArena<AllocationMetadata>,
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
    pub fn new(
        id: RegionId,
        name: String,
        strategy_hint: RegionStrategy,
        parent: Option<RegionId>,
        pos: Position,
    ) -> Self {
        Self {
            id,
            name,
            strategy_hint,
            selected_strategy: RegionStrategy::Auto,
            allocations: HashMap::new(),
            allocation_table: HybridGenerationalArena::new(),
            child_regions: Vec::new(),
            parent_region: parent,
            created_at: pos,
            is_active: true,
        }
    }

    /// Add an allocation to this region
    pub fn add_allocation(&mut self, variable: String, created_at: Position) -> RegionHandle {
        let meta = AllocationMetadata {
            variable: variable.clone(),
            created_at,
        };
        let slot = self.allocation_table.insert(meta);
        let handle = RegionHandle::new(self.id, slot);
        self.allocations.insert(variable, handle);
        handle
    }

    /// Remove an allocation from this region
    pub fn remove_allocation(&mut self, variable: &str) -> Option<AllocationMetadata> {
        if let Some(handle) = self.allocations.remove(variable) {
            self.allocation_table.remove(handle.slot())
        } else {
            None
        }
    }

    /// Add a child region
    pub fn add_child(&mut self, child_id: RegionId) {
        self.child_regions.push(child_id);
    }

    /// Mark region as inactive (deallocated)
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.allocations.clear();
        self.allocation_table.clear();
    }

    /// Determine the optimal strategy based on hints and allocation shape.
    fn determine_strategy(&self) -> RegionStrategy {
        match self.strategy_hint {
            RegionStrategy::Auto => {
                let allocation_count = self.allocations.len();
                let has_children = !self.child_regions.is_empty();
                if !has_children && allocation_count <= 8 {
                    RegionStrategy::Stack
                } else if has_children && allocation_count == 0 {
                    // Control-only regions default to stack semantics.
                    RegionStrategy::Stack
                } else {
                    RegionStrategy::Bump
                }
            }
            explicit => explicit,
        }
    }

    /// Finalize strategy selection before deactivation.
    pub fn finalize_strategy(&mut self) {
        self.selected_strategy = self.determine_strategy();
    }

    /// Retrieve the strategy chosen after analysis.
    pub fn selected_strategy(&self) -> RegionStrategy {
        self.selected_strategy
    }

    /// Retrieve metadata for an active allocation.
    pub fn allocation_metadata(&self, handle: GenerationalHandle) -> Option<&AllocationMetadata> {
        self.allocation_table.resolve(handle)
    }

    /// Fast path metadata lookup (elides checks in release mode).
    pub fn allocation_metadata_fast(
        &self,
        handle: GenerationalHandle,
    ) -> Option<&AllocationMetadata> {
        self.allocation_table.resolve_fast(handle)
    }
}

/// Manages all memory regions in a program
#[derive(Debug)]
pub struct RegionManager {
    /// All regions stored contiguously by their numeric ID
    regions: Vec<Region>,
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
        let mut regions = Vec::new();
        regions.push(Region::new(
            global_id,
            "global".to_string(),
            RegionStrategy::Auto,
            None,
            Position::new(0, 0, 0),
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
    pub fn create_region(
        &mut self,
        name: String,
        strategy_hint: RegionStrategy,
        pos: Position,
    ) -> RegionId {
        let region_id = RegionId::new(self.next_region_id);
        self.next_region_id += 1;

        let parent_id = self.current_region();
        let region = Region::new(region_id, name, strategy_hint, Some(parent_id), pos);

        let index = region_id.id() as usize;
        debug_assert_eq!(index, self.regions.len(), "region ids should be sequential");
        self.regions.push(region);

        let parent_index = parent_id.id() as usize;
        if let Some(parent) = self.regions.get_mut(parent_index) {
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
    pub fn allocate_in_current_region(
        &mut self,
        variable: String,
        position: Position,
    ) -> Result<RegionHandle, RegionError> {
        let current = self.current_region();
        self.allocate_in_region(variable, current, position)
    }

    /// Allocate a variable in a specific region
    pub fn allocate_in_region(
        &mut self,
        variable: String,
        region_id: RegionId,
        position: Position,
    ) -> Result<RegionHandle, RegionError> {
        if let Some(existing_region) = self.find_variable_region(&variable) {
            let err = RegionError::MultipleAllocation {
                variable,
                regions: vec![existing_region, region_id],
                position,
            };
            self.errors.push(err.clone());
            return Err(err);
        }

        let index = region_id.id() as usize;
        if let Some(region) = self.regions.get_mut(index) {
            if !region.is_active {
                let err = RegionError::RegionInactive {
                    region_id,
                    variable,
                    position,
                };
                self.errors.push(err.clone());
                return Err(err);
            }
            let handle = region.add_allocation(variable, position);
            Ok(handle)
        } else {
            let err = RegionError::RegionNotFound {
                region_id,
                position,
            };
            self.errors.push(err.clone());
            Err(err)
        }
    }

    /// Deallocate a specific variable
    pub fn deallocate_variable(&mut self, variable: &str) {
        for region in &mut self.regions {
            region.remove_allocation(variable);
        }
    }

    /// Recursively deactivate a region and all its children
    fn deactivate_region_tree(&mut self, region_id: RegionId) {
        let index = region_id.id() as usize;
        if index >= self.regions.len() {
            return;
        }

        let child_ids = self.regions[index].child_regions.clone();
        for child_id in child_ids {
            self.deactivate_region_tree(child_id);
        }

        let region = &mut self.regions[index];
        region.finalize_strategy();
        region.deactivate();
    }

    /// Get region information by ID
    pub fn get_region(&self, region_id: RegionId) -> Option<&Region> {
        self.regions.get(region_id.id() as usize)
    }

    /// Fetch the selected strategy for a region, if known.
    pub fn region_strategy(&self, region_id: RegionId) -> Option<RegionStrategy> {
        self.regions
            .get(region_id.id() as usize)
            .map(|region| region.selected_strategy())
    }

    /// Iterate over all regions (including inactive ones).
    pub fn regions(&self) -> impl Iterator<Item = &Region> {
        self.regions.iter()
    }

    /// Check if a variable is allocated in any active region
    pub fn is_allocated(&self, variable: &str) -> bool {
        self.regions
            .iter()
            .any(|region| region.is_active && region.allocations.contains_key(variable))
    }

    /// Find which region contains a variable
    pub fn find_variable_region(&self, variable: &str) -> Option<RegionId> {
        self.regions
            .iter()
            .enumerate()
            .find(|(_, region)| region.is_active && region.allocations.contains_key(variable))
            .map(|(id, _)| RegionId::new(id as u32))
    }

    /// Retrieve the handle associated with a variable if it is still live.
    pub fn allocation_handle(&self, variable: &str) -> Option<RegionHandle> {
        self.regions.iter().find_map(|region| {
            if region.is_active {
                region.allocations.get(variable).copied()
            } else {
                None
            }
        })
    }

    /// Resolve allocation metadata from a handle with full validation.
    pub fn resolve_handle(&self, handle: RegionHandle) -> Option<&AllocationMetadata> {
        let region = self.regions.get(handle.region().id() as usize)?;
        if !region.is_active {
            return None;
        }
        region.allocation_metadata(handle.slot())
    }

    /// Hot-path resolution used by release builds to avoid extra branching.
    pub fn resolve_handle_fast(&self, handle: RegionHandle) -> Option<&AllocationMetadata> {
        let region = self.regions.get(handle.region().id() as usize)?;
        if !region.is_active {
            return None;
        }
        region.allocation_metadata_fast(handle.slot())
    }

    /// Get all active regions
    pub fn active_regions(&self) -> Vec<RegionId> {
        self.regions
            .iter()
            .enumerate()
            .filter(|(_, region)| region.is_active)
            .map(|(id, _)| RegionId::new(id as u32))
            .collect()
    }

    /// Validate region hierarchy and detect issues
    pub fn validate_regions(&mut self) -> Vec<RegionError> {
        let mut errors = Vec::new();

        // Check for orphaned regions
        for (index, region) in self.regions.iter().enumerate() {
            let region_id = RegionId::new(index as u32);
            if let Some(parent_id) = region.parent_region {
                let parent_index = parent_id.id() as usize;
                if parent_index >= self.regions.len() {
                    errors.push(RegionError::OrphanedRegion {
                        region_id,
                        missing_parent: parent_id,
                        position: region.created_at,
                    });
                }
            }
        }

        // Check for circular references
        for (index, _) in self.regions.iter().enumerate() {
            let region_id = RegionId::new(index as u32);
            if self.has_circular_reference(region_id) {
                errors.push(RegionError::CircularReference {
                    region_id,
                    position: self
                        .regions
                        .get(index)
                        .map(|region| region.created_at)
                        .unwrap_or_else(|| Position::new(0, 0, 0)),
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

            current = self
                .regions
                .get(id.id() as usize)
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
            RegionError::RegionNotFound {
                region_id,
                position,
            } => {
                write!(f, "Region {:?} not found at {}", region_id, position)
            }
            RegionError::RegionInactive {
                region_id,
                variable,
                position,
            } => {
                write!(
                    f,
                    "Cannot allocate variable '{}' in inactive region {:?} at {}",
                    variable, region_id, position
                )
            }
            RegionError::OrphanedRegion {
                region_id,
                missing_parent,
                position,
            } => {
                write!(
                    f,
                    "Region {:?} is orphaned (parent {:?} not found) at {}",
                    region_id, missing_parent, position
                )
            }
            RegionError::CircularReference {
                region_id,
                position,
            } => {
                write!(
                    f,
                    "Circular reference detected in region {:?} at {}",
                    region_id, position
                )
            }
            RegionError::MultipleAllocation {
                variable,
                regions,
                position,
            } => {
                write!(
                    f,
                    "Variable '{}' allocated in multiple regions {:?} at {}",
                    variable, regions, position
                )
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
            Expression::Let {
                name, value, pos, ..
            } => {
                // First analyze the value expression
                self.analyze_expression(value)?;

                // Then allocate the variable in the current region
                self.region_manager
                    .allocate_in_current_region(name.clone(), *pos)?;

                Ok(())
            }

            // Blocks create new regions
            Expression::Block { expressions, pos } => {
                let region_id = self.region_manager.create_region(
                    format!("block_{}", pos.line),
                    RegionStrategy::Auto,
                    *pos,
                );

                self.region_manager.enter_region(region_id);

                for expr in expressions {
                    self.analyze_expression(expr)?;
                }

                self.region_manager.exit_region();
                Ok(())
            }

            // Function definitions create their own regions
            Expression::Function {
                name, body, pos, ..
            } => {
                let region_id = self.region_manager.create_region(
                    format!("function_{}", name),
                    RegionStrategy::Auto,
                    *pos,
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
                    RegionStrategy::Auto,
                    *pos,
                );

                self.region_manager.enter_region(region_id);
                self.analyze_expression(body)?;
                self.region_manager.exit_region();

                Ok(())
            }

            Expression::Region {
                name,
                strategy,
                body,
                pos,
            } => {
                let region_name = name
                    .clone()
                    .unwrap_or_else(|| format!("region_{}", pos.line));
                let region_id = self
                    .region_manager
                    .create_region(region_name, *strategy, *pos);
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

            Expression::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
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
        let pos = Position::new(1, 1, 0);

        let region_id = manager.create_region("test_region".to_string(), RegionStrategy::Auto, pos);

        assert!(manager.get_region(region_id).is_some());
        assert_eq!(manager.get_region(region_id).unwrap().name, "test_region");
    }

    #[test]
    fn test_region_stack() {
        let mut manager = RegionManager::new();
        let global = manager.current_region();

        let pos = Position::new(1, 1, 0);
        let region1 = manager.create_region("region1".to_string(), RegionStrategy::Auto, pos);
        manager.enter_region(region1);

        assert_eq!(manager.current_region(), region1);

        let exited = manager.exit_region();
        assert_eq!(exited, Some(region1));
        assert_eq!(manager.current_region(), global);
    }

    #[test]
    fn test_variable_allocation() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1, 0);

        let handle = manager
            .allocate_in_current_region("x".to_string(), pos)
            .expect("allocation should succeed");

        assert!(manager.is_allocated("x"));
        assert!(manager.find_variable_region("x").is_some());
        assert_eq!(
            manager
                .resolve_handle(handle)
                .map(|meta| meta.variable.as_str()),
            Some("x")
        );

        manager.deallocate_variable("x");
        assert!(!manager.is_allocated("x"));
        assert!(manager.resolve_handle(handle).is_none());
    }

    #[test]
    fn test_region_analyzer() {
        let mut analyzer = RegionAnalyzer::new();

        // Create a simple program: let x = 42
        let program = Program {
            expressions: vec![Expression::Let {
                name: "x".to_string(),
                type_annotation: None,
                value: Box::new(Expression::IntegerLiteral {
                    value: 42,
                    pos: Position::new(1, 9, 8),
                }),
                is_mutable: false,
                delegation: None,
                pos: Position::new(1, 1, 0),
            }],
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
            expressions: vec![Expression::Block {
                expressions: vec![Expression::Let {
                    name: "y".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::IntegerLiteral {
                        value: 10,
                        pos: Position::new(2, 5, 4),
                    }),
                    is_mutable: false,
                    delegation: None,
                    pos: Position::new(2, 1, 0),
                }],
                pos: Position::new(1, 1, 0),
            }],
        };

        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());

        let region_manager = result.unwrap();

        // Variable should not be allocated anymore (block exited)
        assert!(!region_manager.is_allocated("y"));
    }

    #[test]
    fn handle_generations_change_after_reuse() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1, 0);

        let first = manager
            .allocate_in_current_region("item".to_string(), pos)
            .unwrap();
        manager.deallocate_variable("item");
        let second = manager
            .allocate_in_current_region("item".to_string(), pos)
            .unwrap();

        assert_ne!(first.slot().generation(), second.slot().generation());
    }

    #[test]
    fn resolve_handle_fast_matches_checked() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1, 0);

        let handle = manager
            .allocate_in_current_region("fast".to_string(), pos)
            .unwrap();

        let checked = manager.resolve_handle(handle).unwrap().created_at;
        let fast = manager.resolve_handle_fast(handle).unwrap().created_at;

        assert_eq!(checked, fast);
    }

    #[test]
    fn auto_strategy_prefers_stack_for_small_regions() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1, 0);

        let region_id = manager.create_region("small".to_string(), RegionStrategy::Auto, pos);
        manager.enter_region(region_id);
        manager
            .allocate_in_current_region("a".to_string(), pos)
            .unwrap();
        manager
            .allocate_in_current_region("b".to_string(), pos)
            .unwrap();

        manager.exit_region();

        assert_eq!(
            manager.region_strategy(region_id),
            Some(RegionStrategy::Stack)
        );
    }

    #[test]
    fn explicit_strategy_hint_is_preserved() {
        let mut manager = RegionManager::new();
        let pos = Position::new(1, 1, 0);

        let region_id = manager.create_region("cxl".to_string(), RegionStrategy::CxlNear, pos);
        manager.enter_region(region_id);
        manager.exit_region();

        assert_eq!(
            manager.region_strategy(region_id),
            Some(RegionStrategy::CxlNear)
        );
    }

    #[test]
    fn analyzer_applies_region_hint() {
        let mut analyzer = RegionAnalyzer::new();
        let pos = Position::new(10, 5, 4);
        let program = Program {
            expressions: vec![Expression::Region {
                name: Some("upload".to_string()),
                strategy: RegionStrategy::Bump,
                body: Box::new(Expression::Block {
                    expressions: vec![Expression::Let {
                        name: "buffer".to_string(),
                        type_annotation: None,
                        value: Box::new(Expression::IntegerLiteral { value: 1, pos }),
                        is_mutable: false,
                        delegation: None,
                        pos,
                    }],
                    pos,
                }),
                pos,
            }],
        };

        let region_manager = analyzer.analyze_program(&program).unwrap();
        let region = region_manager
            .regions()
            .find(|region| region.name == "upload")
            .expect("region upload should exist");
        assert_eq!(region.selected_strategy(), RegionStrategy::Bump);
    }
}
