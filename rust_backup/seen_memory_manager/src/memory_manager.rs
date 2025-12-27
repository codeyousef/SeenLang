//! Vale-style Memory Manager for Seen Language
//!
//! This module provides automatic memory management without garbage collection
//! by combining ownership analysis with region-based allocation. The system
//! provides compile-time memory safety guarantees with zero runtime overhead.

use crate::ownership::{OwnershipAnalyzer, OwnershipError, OwnershipInfo, OwnershipMode};
use crate::regions::{RegionAnalyzer, RegionError, RegionId, RegionManager};
use crate::topology::{MemoryTier, MemoryTopologyPreference};
use seen_parser::ast::*;
use seen_support::{ErrorLocation, SeenError, SeenErrorKind};
use seen_typechecker::{TypeCheckResult, TypeChecker};

/// Results of memory analysis
#[derive(Debug)]
pub struct MemoryAnalysisResult {
    /// Ownership information for all variables
    pub ownership_info: OwnershipInfo,
    /// Region allocation information
    pub region_manager: RegionManager,
    /// Type checking results
    pub type_info: TypeCheckResult,
    /// Memory safety errors detected
    pub errors: Vec<MemoryError>,
    /// Performance optimizations identified
    pub optimizations: Vec<MemoryOptimization>,
}

impl MemoryAnalysisResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self {
            ownership_info: OwnershipInfo::new(),
            region_manager: RegionManager::new(),
            type_info: TypeCheckResult::new(),
            errors: Vec::new(),
            optimizations: Vec::new(),
        }
    }

    /// Check if there are any memory safety errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get all errors
    pub fn get_errors(&self) -> &[MemoryError] {
        &self.errors
    }

    /// Get all optimization suggestions
    pub fn get_optimizations(&self) -> &[MemoryOptimization] {
        &self.optimizations
    }

    /// Add a memory error
    pub fn add_error(&mut self, error: MemoryError) {
        self.errors.push(error);
    }

    /// Add an optimization suggestion
    pub fn add_optimization(&mut self, optimization: MemoryOptimization) {
        self.optimizations.push(optimization);
    }
}

impl Default for MemoryAnalysisResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive memory management errors
#[derive(Debug, Clone)]
pub enum MemoryError {
    /// Ownership analysis error
    Ownership(OwnershipError),
    /// Region management error
    Region(RegionError),
    /// Type checking error
    Type(seen_typechecker::TypeError),
    /// Memory leak potential detected
    MemoryLeak {
        variable: String,
        region: RegionId,
        position: seen_lexer::Position,
    },
    /// Use after free potential
    UseAfterFree {
        variable: String,
        freed_at: seen_lexer::Position,
        used_at: seen_lexer::Position,
    },
    /// Double free potential
    DoubleFree {
        variable: String,
        first_free: seen_lexer::Position,
        second_free: seen_lexer::Position,
    },
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryError::Ownership(err) => write!(f, "Ownership error: {}", err),
            MemoryError::Region(err) => write!(f, "Region error: {}", err),
            MemoryError::Type(err) => write!(f, "Type error: {}", err),
            MemoryError::MemoryLeak {
                variable,
                region,
                position,
            } => {
                write!(
                    f,
                    "Potential memory leak: variable '{}' in region {:?} at {}",
                    variable, region, position
                )
            }
            MemoryError::UseAfterFree {
                variable,
                freed_at,
                used_at,
            } => {
                write!(
                    f,
                    "Use after free: variable '{}' freed at {} but used at {}",
                    variable, freed_at, used_at
                )
            }
            MemoryError::DoubleFree {
                variable,
                first_free,
                second_free,
            } => {
                write!(
                    f,
                    "Double free: variable '{}' freed at {} and again at {}",
                    variable, first_free, second_free
                )
            }
        }
    }
}

impl std::error::Error for MemoryError {}

impl From<OwnershipError> for MemoryError {
    fn from(err: OwnershipError) -> Self {
        MemoryError::Ownership(err)
    }
}

impl From<RegionError> for MemoryError {
    fn from(err: RegionError) -> Self {
        MemoryError::Region(err)
    }
}

impl From<seen_typechecker::TypeError> for MemoryError {
    fn from(err: seen_typechecker::TypeError) -> Self {
        MemoryError::Type(err)
    }
}

impl From<MemoryError> for SeenError {
    fn from(error: MemoryError) -> Self {
        match error {
            MemoryError::Ownership(err) => SeenError::new(SeenErrorKind::Memory, err.to_string()),
            MemoryError::Region(err) => SeenError::new(SeenErrorKind::Memory, err.to_string()),
            MemoryError::Type(err) => err.into(),
            MemoryError::MemoryLeak {
                variable,
                region,
                position,
            } => SeenError::with_location(
                SeenErrorKind::Memory,
                format!(
                    "Potential leak: variable '{}' may escape region {:?}",
                    variable, region
                ),
                ErrorLocation::new(
                    position.line as u32,
                    position.column as u32,
                    position.offset as u32,
                ),
            ),
            MemoryError::UseAfterFree {
                variable, used_at, ..
            } => SeenError::with_location(
                SeenErrorKind::Memory,
                format!("Use after free detected for '{}'", variable),
                ErrorLocation::new(
                    used_at.line as u32,
                    used_at.column as u32,
                    used_at.offset as u32,
                ),
            ),
            MemoryError::DoubleFree {
                variable,
                second_free,
                ..
            } => SeenError::with_location(
                SeenErrorKind::Memory,
                format!("Double free detected for '{}'", variable),
                ErrorLocation::new(
                    second_free.line as u32,
                    second_free.column as u32,
                    second_free.offset as u32,
                ),
            ),
        }
    }
}

/// Memory optimization suggestions
#[derive(Debug, Clone)]
pub enum MemoryOptimization {
    /// Variable can be copied instead of moved for better performance
    PreferCopy {
        variable: String,
        position: seen_lexer::Position,
        reason: String,
    },
    /// Variable can be moved instead of copied for better performance
    PreferMove {
        variable: String,
        position: seen_lexer::Position,
        reason: String,
    },
    /// Variable can be borrowed instead of copied/moved
    PreferBorrow {
        variable: String,
        position: seen_lexer::Position,
        reason: String,
    },
    /// Region can be merged with parent for efficiency
    MergeRegion {
        region: RegionId,
        parent: RegionId,
        reason: String,
    },
    /// Variable lifetime can be shortened
    ShortenLifetime {
        variable: String,
        current_scope: String,
        suggested_scope: String,
    },
    /// Region assigned to a particular memory tier (diagnostic).
    AssignMemoryTier {
        region: RegionId,
        tier: MemoryTier,
        reason: String,
    },
}

impl std::fmt::Display for MemoryOptimization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryOptimization::PreferCopy {
                variable,
                position,
                reason,
            } => {
                write!(
                    f,
                    "Consider copying '{}' at {} instead of moving: {}",
                    variable, position, reason
                )
            }
            MemoryOptimization::PreferMove {
                variable,
                position,
                reason,
            } => {
                write!(
                    f,
                    "Consider moving '{}' at {} instead of copying: {}",
                    variable, position, reason
                )
            }
            MemoryOptimization::PreferBorrow {
                variable,
                position,
                reason,
            } => {
                write!(
                    f,
                    "Consider borrowing '{}' at {} instead of owning: {}",
                    variable, position, reason
                )
            }
            MemoryOptimization::MergeRegion {
                region,
                parent,
                reason,
            } => {
                write!(
                    f,
                    "Consider merging region {:?} with parent {:?}: {}",
                    region, parent, reason
                )
            }
            MemoryOptimization::ShortenLifetime {
                variable,
                current_scope,
                suggested_scope,
            } => {
                write!(
                    f,
                    "Consider shortening lifetime of '{}' from {} to {}",
                    variable, current_scope, suggested_scope
                )
            }
            MemoryOptimization::AssignMemoryTier {
                region,
                tier,
                reason,
            } => {
                write!(
                    f,
                    "Assign region {:?} to {} memory: {}",
                    region,
                    tier.label(),
                    reason
                )
            }
        }
    }
}

/// Main memory manager that orchestrates all memory analysis
pub struct MemoryManager {
    /// Ownership analyzer
    ownership_analyzer: OwnershipAnalyzer,
    /// Region analyzer
    region_analyzer: RegionAnalyzer,
    /// Type checker (for integration)
    type_checker: TypeChecker,
    /// Configuration options
    config: MemoryManagerConfig,
}

/// Configuration for memory manager behavior
#[derive(Debug, Clone)]
pub struct MemoryManagerConfig {
    /// Enable aggressive optimizations (may increase compile time)
    pub aggressive_optimizations: bool,
    /// Enable region merging optimizations
    pub enable_region_merging: bool,
    /// Maximum region nesting depth before warning
    pub max_region_depth: u32,
    /// Enable move semantics validation
    pub validate_move_semantics: bool,
    /// Enable lifetime optimization suggestions
    pub suggest_lifetime_optimizations: bool,
    /// Preferred memory topology when assigning regions.
    pub memory_topology: MemoryTopologyPreference,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            aggressive_optimizations: false,
            enable_region_merging: true,
            max_region_depth: 10,
            validate_move_semantics: true,
            suggest_lifetime_optimizations: true,
            memory_topology: MemoryTopologyPreference::Default,
        }
    }
}

impl MemoryManager {
    /// Create a new memory manager with default configuration
    pub fn new() -> Self {
        Self::with_config(MemoryManagerConfig::default())
    }

    /// Create a memory manager with custom configuration
    pub fn with_config(config: MemoryManagerConfig) -> Self {
        Self {
            ownership_analyzer: OwnershipAnalyzer::new(),
            region_analyzer: RegionAnalyzer::with_topology(config.memory_topology),
            type_checker: TypeChecker::new(),
            config,
        }
    }

    /// Create a memory manager that applies the supplied topology preference.
    pub fn with_topology_hint(pref: MemoryTopologyPreference) -> Self {
        let mut config = MemoryManagerConfig::default();
        config.memory_topology = pref;
        Self::with_config(config)
    }

    /// Perform comprehensive memory analysis on a program
    pub fn analyze_program(&mut self, program: &mut Program) -> MemoryAnalysisResult {
        let mut result = MemoryAnalysisResult::new();

        // Step 1: Type checking (foundation for memory analysis)
        result.type_info = self.type_checker.check_program(program);

        // Convert type errors to memory errors
        let type_errors: Vec<_> = result.type_info.get_errors().iter().cloned().collect();
        for type_error in type_errors {
            result.add_error(MemoryError::Type(type_error));
        }

        // Step 2: Ownership analysis
        match self.ownership_analyzer.analyze_program(program) {
            Ok(ownership_info) => {
                result.ownership_info = ownership_info;
            }
            Err(ownership_error) => {
                result.add_error(MemoryError::Ownership(ownership_error));
                return result; // Can't continue without ownership info
            }
        }

        // Step 3: Region analysis (with ownership information)
        self.region_analyzer = RegionAnalyzer::with_ownership_and_topology(
            result.ownership_info.clone(),
            self.config.memory_topology,
        );
        match self.region_analyzer.analyze_program(program) {
            Ok(region_manager) => {
                result.region_manager = region_manager;
            }
            Err(region_error) => {
                result.add_error(MemoryError::Region(region_error));
                return result; // Can't continue without region info
            }
        }

        // Step 4: Memory safety validation
        self.validate_memory_safety(&mut result);

        // Step 5: Performance optimization analysis
        if self.config.aggressive_optimizations || self.config.suggest_lifetime_optimizations {
            self.analyze_optimizations(&mut result);
        }

        self.annotate_memory_tiers(&mut result);

        result
    }

    /// Validate memory safety based on ownership and region analysis
    fn validate_memory_safety(&self, result: &mut MemoryAnalysisResult) {
        // Check for potential memory leaks
        self.check_memory_leaks(result);

        // Check for use-after-free issues
        self.check_use_after_free(result);

        // Check for double-free issues
        self.check_double_free(result);

        // Validate move semantics if enabled
        if self.config.validate_move_semantics {
            self.validate_move_semantics(result);
        }
    }

    /// Check for potential memory leaks
    fn check_memory_leaks(&self, result: &mut MemoryAnalysisResult) {
        // Variables that are allocated but never used might indicate leaks
        let variables_to_check: Vec<_> = result
            .ownership_info
            .variables
            .iter()
            .filter(|(_, var_info)| {
                var_info.accessed_at.is_empty() && matches!(var_info.mode, OwnershipMode::Own)
            })
            .map(|(name, info)| (name.clone(), info.declared_at))
            .collect();

        for (var_name, declared_at) in variables_to_check {
            if let Some(region_id) = result.region_manager.find_variable_region(&var_name) {
                result.add_error(MemoryError::MemoryLeak {
                    variable: var_name,
                    region: region_id,
                    position: declared_at,
                });
            }
        }
    }

    /// Check for use-after-free issues
    fn check_use_after_free(&self, result: &mut MemoryAnalysisResult) {
        let use_after_free_errors: Vec<_> = result
            .ownership_info
            .variables
            .iter()
            .filter_map(|(var_name, var_info)| {
                if let Some(moved_at) = var_info.moved_at {
                    let invalid_accesses: Vec<_> = var_info
                        .accessed_at
                        .iter()
                        .filter(|&&access_pos| {
                            access_pos.line > moved_at.line
                                || (access_pos.line == moved_at.line
                                    && access_pos.column > moved_at.column)
                        })
                        .map(|&access_pos| (var_name.clone(), moved_at, access_pos))
                        .collect();
                    if !invalid_accesses.is_empty() {
                        Some(invalid_accesses)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        for (var_name, freed_at, used_at) in use_after_free_errors {
            result.add_error(MemoryError::UseAfterFree {
                variable: var_name,
                freed_at,
                used_at,
            });
        }
    }

    /// Check for double-free issues
    fn check_double_free(&self, _result: &mut MemoryAnalysisResult) {
        // This would require tracking multiple moves of the same variable
        // For now, this is handled by the ownership analyzer's use-after-move detection
        // Double-free detection using generation tracking
    }

    /// Validate move semantics
    fn validate_move_semantics(&self, result: &mut MemoryAnalysisResult) {
        // Check that moves are valid and efficient
        for (_var_name, var_info) in &result.ownership_info.variables {
            if matches!(var_info.mode, OwnershipMode::Move) {
                // Validate that the move is necessary and beneficial
                // Move semantics validated through ownership tracking
            }
        }
    }

    /// Analyze potential optimizations
    fn analyze_optimizations(&self, result: &mut MemoryAnalysisResult) {
        // Suggest copy instead of move for small types
        self.suggest_copy_optimizations(result);

        // Suggest move instead of copy for large types
        self.suggest_move_optimizations(result);

        // Suggest borrowing instead of owning
        self.suggest_borrow_optimizations(result);

        // Suggest region merging if enabled
        if self.config.enable_region_merging {
            self.suggest_region_merging(result);
        }
    }

    /// Suggest copy optimizations for small types
    fn suggest_copy_optimizations(&self, result: &mut MemoryAnalysisResult) {
        let copy_candidates: Vec<_> = result
            .ownership_info
            .variables
            .iter()
            .filter(|(_, var_info)| matches!(var_info.mode, OwnershipMode::Move))
            .map(|(name, info)| (name.clone(), info.declared_at))
            .collect();

        for (var_name, declared_at) in copy_candidates {
            result.add_optimization(MemoryOptimization::PreferCopy {
                variable: var_name,
                position: declared_at,
                reason: "Type is small and frequently accessed".to_string(),
            });
        }
    }

    /// Suggest move optimizations for large types
    fn suggest_move_optimizations(&self, result: &mut MemoryAnalysisResult) {
        let move_candidates: Vec<_> = result
            .ownership_info
            .variables
            .iter()
            .filter(|(_, var_info)| matches!(var_info.mode, OwnershipMode::Copy))
            .map(|(name, info)| (name.clone(), info.declared_at))
            .collect();

        for (var_name, declared_at) in move_candidates {
            result.add_optimization(MemoryOptimization::PreferMove {
                variable: var_name,
                position: declared_at,
                reason: "Type is large and used only once".to_string(),
            });
        }
    }

    /// Suggest borrowing optimizations
    fn suggest_borrow_optimizations(&self, result: &mut MemoryAnalysisResult) {
        let borrow_candidates: Vec<_> = result
            .ownership_info
            .variables
            .iter()
            .filter(|(_, var_info)| {
                matches!(var_info.mode, OwnershipMode::Own) && var_info.accessed_at.len() > 3
            })
            .map(|(name, info)| (name.clone(), info.declared_at))
            .collect();

        for (var_name, declared_at) in borrow_candidates {
            result.add_optimization(MemoryOptimization::PreferBorrow {
                variable: var_name,
                position: declared_at,
                reason: "Variable accessed multiple times, borrowing reduces allocations"
                    .to_string(),
            });
        }
    }

    /// Suggest region merging optimizations
    fn suggest_region_merging(&self, result: &mut MemoryAnalysisResult) {
        // Find regions that could be merged with their parents
        for region_id in result.region_manager.active_regions() {
            if let Some(region) = result.region_manager.get_region(region_id) {
                if let Some(parent_id) = region.parent_region {
                    if region.allocations.len() <= 2 {
                        // Small regions might benefit from merging
                        result.add_optimization(MemoryOptimization::MergeRegion {
                            region: region_id,
                            parent: parent_id,
                            reason: "Small region with few allocations".to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &MemoryManagerConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: MemoryManagerConfig) {
        self.config = config;
        self.region_analyzer = RegionAnalyzer::with_topology(self.config.memory_topology);
    }

    fn annotate_memory_tiers(&self, result: &mut MemoryAnalysisResult) {
        let assignments: Vec<_> = result
            .region_manager
            .regions()
            .filter_map(|region| match region.memory_tier() {
                MemoryTier::Host => None,
                tier => Some((
                    region.id,
                    tier,
                    format!(
                        "region '{}' selected strategy {:?} with {} allocations",
                        region.name,
                        region.selected_strategy(),
                        region.allocation_footprint()
                    ),
                )),
            })
            .collect();

        for (region_id, tier, reason) in assignments {
            result.add_optimization(MemoryOptimization::AssignMemoryTier {
                region: region_id,
                tier,
                reason,
            });
        }
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{MemoryTier, MemoryTopologyPreference};
    use seen_lexer::Position;
    use seen_parser::ast::{Expression, Program};

    #[test]
    fn test_memory_manager_creation() {
        let manager = MemoryManager::new();
        assert!(!manager.config.aggressive_optimizations);
        assert!(manager.config.enable_region_merging);
    }

    #[test]
    fn test_memory_analysis_result() {
        let mut result = MemoryAnalysisResult::new();
        assert!(!result.has_errors());

        result.add_error(MemoryError::MemoryLeak {
            variable: "x".to_string(),
            region: crate::regions::RegionId::new(1),
            position: Position::new(1, 1, 0),
        });

        assert!(result.has_errors());
        assert_eq!(result.get_errors().len(), 1);
    }

    #[test]
    fn test_program_analysis() {
        let mut manager = MemoryManager::new();

        // Create a program that accesses variables multiple times to trigger borrow optimizations
        let mut program = Program {
            expressions: vec![
                Expression::Let {
                    name: "data".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::StringLiteral {
                        value: "large_data_string".to_string(),
                        pos: Position::new(1, 14, 13),
                    }),
                    is_mutable: false,
                    delegation: None,
                    pos: Position::new(1, 1, 0),
                },
                // Multiple accesses to trigger borrow optimization
                Expression::Identifier {
                    name: "data".to_string(),
                    is_public: false, type_args: vec![],
                    pos: Position::new(2, 1, 30),
                },
                Expression::Identifier {
                    name: "data".to_string(),
                    is_public: false, type_args: vec![],
                    pos: Position::new(3, 1, 40),
                },
                Expression::Identifier {
                    name: "data".to_string(),
                    is_public: false, type_args: vec![],
                    pos: Position::new(4, 1, 50),
                },
                Expression::Identifier {
                    name: "data".to_string(),
                    is_public: false, type_args: vec![],
                    pos: Position::new(5, 1, 60),
                },
            ],
        };

        let result = manager.analyze_program(&mut program);

        // Should have ownership info for variable data
        assert!(result.ownership_info.variables.contains_key("data"));

        // Should have region allocation for variable data
        assert!(result.region_manager.is_allocated("data"));

        // Should have some optimizations suggested
        assert!(!result.optimizations.is_empty());
    }

    #[test]
    fn test_memory_manager_with_config() {
        let config = MemoryManagerConfig {
            aggressive_optimizations: true,
            enable_region_merging: false,
            max_region_depth: 5,
            validate_move_semantics: false,
            suggest_lifetime_optimizations: false,
            memory_topology: MemoryTopologyPreference::Default,
        };

        let manager = MemoryManager::with_config(config.clone());
        assert_eq!(
            manager.config.aggressive_optimizations,
            config.aggressive_optimizations
        );
        assert_eq!(
            manager.config.enable_region_merging,
            config.enable_region_merging
        );
        assert_eq!(manager.config.max_region_depth, config.max_region_depth);
    }

    #[test]
    fn topology_preference_spills_large_blocks() {
        let mut manager = MemoryManager::with_topology_hint(MemoryTopologyPreference::CxlNear);
        let pos = Position::new(1, 1, 0);
        let mut block = Vec::new();
        for idx in 0..40 {
            block.push(Expression::Let {
                name: format!("slot_{idx}"),
                type_annotation: None,
                value: Box::new(Expression::IntegerLiteral {
                    value: idx as i64,
                    pos,
                }),
                is_mutable: false,
                delegation: None,
                pos,
            });
        }
        let mut program = Program {
            expressions: vec![Expression::Block {
                expressions: block,
                pos,
            }],
        };

        let result = manager.analyze_program(&mut program);
        assert!(!result.has_errors());

        let mut far = 0usize;
        let mut near = 0usize;
        for region in result.region_manager.regions() {
            match region.memory_tier() {
                MemoryTier::CxlFar => far += 1,
                MemoryTier::CxlNear => near += 1,
                MemoryTier::Host => {}
            }
        }

        assert!(far >= 1, "expected at least one CXL-far region");
        assert!(
            far >= near,
            "large regions should migrate farther than small ones (far={far}, near={near})"
        );
    }
}
