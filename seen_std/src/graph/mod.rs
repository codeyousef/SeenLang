//! Graph algorithms for dependency analysis and compilation order
//!
//! High-performance graph algorithms optimized for compiler workloads:
//! - Dependency resolution
//! - Topological sorting for compilation order  
//! - Strongly connected components for cycle detection
//! - Module dependency tracking

use crate::collections::{Vec, HashMap, HashSet};
use crate::string::String;

/// Graph representation optimized for dependency analysis
#[derive(Debug, Clone)]
pub struct Graph<T> 
where
    T: Clone + Eq + std::hash::Hash,
{
    /// Adjacency list representation
    adj_list: HashMap<T, Vec<T>>,
    /// All nodes in the graph
    nodes: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash> Graph<T> {
    /// Create a new empty graph
    pub fn new() -> Self {
        Graph {
            adj_list: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: T) {
        self.nodes.insert(node.clone());
        if !self.adj_list.contains_key(&node) {
            self.adj_list.insert(node, Vec::new());
        }
    }

    /// Add an edge from `from` to `to` (directed edge)
    pub fn add_edge(&mut self, from: T, to: T) {
        self.add_node(from.clone());
        self.add_node(to.clone());
        
        if let Some(adj) = self.adj_list.get_mut(&from) {
            adj.push(to);
        }
    }

    /// Get all neighbors of a node
    pub fn neighbors(&self, node: &T) -> Option<&Vec<T>> {
        self.adj_list.get(node)
    }

    /// Get all nodes in the graph
    pub fn nodes(&self) -> &HashSet<T> {
        &self.nodes
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get number of edges
    pub fn edge_count(&self) -> usize {
        let mut count = 0;
        for adj in self.adj_list.values() {
            count += adj.len();
        }
        count
    }

    /// Check if there's an edge from `from` to `to`
    pub fn has_edge(&self, from: &T, to: &T) -> bool {
        if let Some(adj) = self.adj_list.get(from) {
            adj.contains(to)
        } else {
            false
        }
    }

    /// Topological sort using Kahn's algorithm
    /// Returns None if the graph has cycles
    pub fn topological_sort(&self) -> Option<Vec<T>> {
        let mut in_degree = HashMap::new();
        let mut result = Vec::new();
        let mut queue = Vec::new();

        // Initialize in-degrees
        for node in &self.nodes {
            in_degree.insert(node.clone(), 0usize);
        }

        // Calculate in-degrees
        for (_, neighbors) in &self.adj_list {
            for neighbor in neighbors {
                if let Some(degree) = in_degree.get_mut(neighbor) {
                    *degree += 1;
                }
            }
        }

        // Add nodes with zero in-degree to queue
        for (node, degree) in &in_degree {
            if *degree == 0usize {
                queue.push(node.clone());
            }
        }

        // Process nodes
        while !queue.is_empty() {
            let current = queue.remove(0);
            result.push(current.clone());

            // Reduce in-degree of neighbors
            if let Some(neighbors) = self.adj_list.get(&current) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0usize {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        // Check if all nodes were processed (no cycles)
        if result.len() == self.nodes.len() {
            Some(result)
        } else {
            None // Graph has cycles
        }
    }

    /// Find strongly connected components using Kosaraju's algorithm
    pub fn strongly_connected_components(&self) -> Vec<Vec<T>> {
        let mut visited = HashSet::new();
        let mut finish_stack = Vec::new();
        let mut components = Vec::new();

        // First DFS to fill finish stack
        for node in &self.nodes {
            if !visited.contains(node) {
                self.dfs_finish_time(node, &mut visited, &mut finish_stack);
            }
        }

        // Create transposed graph
        let transposed = self.transpose();

        // Second DFS on transposed graph in reverse finish order
        visited.clear();
        while !finish_stack.is_empty() {
            let node = finish_stack.remove(finish_stack.len() - 1);
            if !visited.contains(&node) {
                let mut component = Vec::new();
                transposed.dfs_component(&node, &mut visited, &mut component);
                if !component.is_empty() {
                    components.push(component);
                }
            }
        }

        components
    }

    /// Check if the graph has cycles
    pub fn has_cycles(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Find dependencies for a given node (all nodes this node depends on)
    pub fn dependencies(&self, node: &T) -> Vec<T> {
        let mut deps = Vec::new();
        let mut visited = HashSet::new();
        self.dfs_dependencies(node, &mut visited, &mut deps);
        deps
    }

    /// Find dependents for a given node (all nodes that depend on this node)
    pub fn dependents(&self, node: &T) -> Vec<T> {
        let mut deps = Vec::new();
        for (from_node, neighbors) in &self.adj_list {
            if neighbors.contains(node) {
                deps.push(from_node.clone());
            }
        }
        deps
    }

    // Helper methods

    /// DFS for finish time calculation
    fn dfs_finish_time(&self, node: &T, visited: &mut HashSet<T>, finish_stack: &mut Vec<T>) {
        visited.insert(node.clone());

        if let Some(neighbors) = self.adj_list.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    self.dfs_finish_time(neighbor, visited, finish_stack);
                }
            }
        }

        finish_stack.push(node.clone());
    }

    /// DFS for component finding
    fn dfs_component(&self, node: &T, visited: &mut HashSet<T>, component: &mut Vec<T>) {
        visited.insert(node.clone());
        component.push(node.clone());

        if let Some(neighbors) = self.adj_list.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    self.dfs_component(neighbor, visited, component);
                }
            }
        }
    }

    /// Create transposed (reversed) graph
    fn transpose(&self) -> Graph<T> {
        let mut transposed = Graph::new();

        // Add all nodes
        for node in &self.nodes {
            transposed.add_node(node.clone());
        }

        // Reverse all edges
        for (from, neighbors) in &self.adj_list {
            for to in neighbors {
                transposed.add_edge(to.clone(), from.clone());
            }
        }

        transposed
    }

    /// DFS for finding dependencies
    fn dfs_dependencies(&self, node: &T, visited: &mut HashSet<T>, deps: &mut Vec<T>) {
        if visited.contains(node) {
            return;
        }
        visited.insert(node.clone());

        if let Some(neighbors) = self.adj_list.get(node) {
            for neighbor in neighbors {
                if !visited.contains(neighbor) {
                    deps.push(neighbor.clone());
                    self.dfs_dependencies(neighbor, visited, deps);
                }
            }
        }
    }
}

impl<T> Default for Graph<T>
where
    T: Clone + Eq + std::hash::Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Module dependency graph for compiler use
pub struct ModuleGraph {
    graph: Graph<String>,
}

impl ModuleGraph {
    /// Create new module dependency graph
    pub fn new() -> Self {
        ModuleGraph {
            graph: Graph::new(),
        }
    }

    /// Add module
    pub fn add_module(&mut self, module: &str) {
        self.graph.add_node(String::from(module));
    }

    /// Add dependency: `module` depends on `dependency`
    pub fn add_dependency(&mut self, module: &str, dependency: &str) {
        // If module depends on dependency, then dependency must be built first
        // So we add an edge FROM dependency TO module
        self.graph.add_edge(String::from(dependency), String::from(module));
    }

    /// Get compilation order (topologically sorted)
    /// Returns None if there are circular dependencies
    pub fn compilation_order(&self) -> Option<Vec<String>> {
        self.graph.topological_sort()
    }

    /// Check for circular dependencies
    pub fn has_circular_dependencies(&self) -> bool {
        self.graph.has_cycles()
    }

    /// Find strongly connected components (circular dependency groups)
    pub fn circular_dependency_groups(&self) -> Vec<Vec<String>> {
        let components = self.graph.strongly_connected_components();
        // Filter out single-node components (unless they have self-loops)
        let mut circular_groups = Vec::new();
        for component in components {
            if component.len() > 1 {
                circular_groups.push(component);
            } else if component.len() == 1 {
                // Check for self-loop
                if let Some(node) = component.first() {
                    if self.graph.has_edge(node, node) {
                        circular_groups.push(component);
                    }
                }
            }
        }
        circular_groups
    }

    /// Get all dependencies of a module (modules this module depends on)
    pub fn module_dependencies(&self, module: &str) -> Vec<String> {
        let module_name = String::from(module);
        // Since edges go FROM dependency TO dependent, we need to find nodes that have edges TO this module
        self.graph.dependents(&module_name)
    }

    /// Get all modules that depend on this module
    pub fn module_dependents(&self, module: &str) -> Vec<String> {
        let module_name = String::from(module);
        // Since edges go FROM dependency TO dependent, we need to find nodes this module has edges TO
        self.graph.dependencies(&module_name)
    }

    /// Get all modules
    pub fn modules(&self) -> Vec<String> {
        let mut modules = Vec::new();
        for module in self.graph.nodes() {
            modules.push(module.clone());
        }
        modules
    }

    /// Get statistics about the dependency graph
    pub fn statistics(&self) -> DependencyStatistics {
        DependencyStatistics {
            total_modules: self.graph.node_count(),
            total_dependencies: self.graph.edge_count(),
            has_cycles: self.graph.has_cycles(),
            strongly_connected_components: self.graph.strongly_connected_components().len(),
        }
    }
}

impl Default for ModuleGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about dependency graph
#[derive(Debug, Clone)]
pub struct DependencyStatistics {
    pub total_modules: usize,
    pub total_dependencies: usize,
    pub has_cycles: bool,
    pub strongly_connected_components: usize,
}

#[cfg(test)]
mod tests;