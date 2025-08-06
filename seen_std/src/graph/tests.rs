//! Comprehensive tests for graph algorithms
//!
//! Tests cover all critical graph functionality needed for dependency analysis

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let graph: Graph<i32> = Graph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_nodes() {
        let mut graph = Graph::new();
        graph.add_node(1);
        graph.add_node(2);
        graph.add_node(3);

        assert_eq!(graph.node_count(), 3);
        assert!(graph.nodes().contains(&1));
        assert!(graph.nodes().contains(&2));
        assert!(graph.nodes().contains(&3));
    }

    #[test]
    fn test_add_edges() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(1, 3);

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 3);
        assert!(graph.has_edge(&1, &2));
        assert!(graph.has_edge(&2, &3));
        assert!(graph.has_edge(&1, &3));
        assert!(!graph.has_edge(&3, &1));
    }

    #[test]
    fn test_neighbors() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(1, 3);
        graph.add_edge(1, 4);

        let neighbors = graph.neighbors(&1).unwrap();
        assert_eq!(neighbors.len(), 3);
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
        assert!(neighbors.contains(&4));

        assert!(graph.neighbors(&5).is_none());
    }

    #[test]
    fn test_topological_sort_simple() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(1, 3);

        let result = graph.topological_sort().unwrap();
        assert_eq!(result.len(), 3);
        
        // 1 should come before 2 and 3
        let pos1 = result.iter().position(|&x| x == 1).unwrap();
        let pos2 = result.iter().position(|&x| x == 2).unwrap();
        let pos3 = result.iter().position(|&x| x == 3).unwrap();
        
        assert!(pos1 < pos2);
        assert!(pos1 < pos3);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_topological_sort_complex() {
        let mut graph = Graph::new();
        // Dependencies: 5->11, 7->11, 7->8, 3->8, 3->10, 11->2, 11->9, 11->10, 8->9
        graph.add_edge(5, 11);
        graph.add_edge(7, 11);
        graph.add_edge(7, 8);
        graph.add_edge(3, 8);
        graph.add_edge(3, 10);
        graph.add_edge(11, 2);
        graph.add_edge(11, 9);
        graph.add_edge(11, 10);
        graph.add_edge(8, 9);

        let result = graph.topological_sort().unwrap();
        assert_eq!(result.len(), 8);  // 5, 7, 3, 11, 8, 2, 9, 10

        // Verify ordering constraints
        let mut positions = HashMap::new();
        for (i, &val) in result.iter().enumerate() {
            positions.insert(val, i);
        }

        // 5 -> 11
        assert!(positions.get(&5).unwrap() < positions.get(&11).unwrap());
        // 7 -> 11 and 7 -> 8
        assert!(positions.get(&7).unwrap() < positions.get(&11).unwrap());
        assert!(positions.get(&7).unwrap() < positions.get(&8).unwrap());
        // 3 -> 8 and 3 -> 10
        assert!(positions.get(&3).unwrap() < positions.get(&8).unwrap());
        assert!(positions.get(&3).unwrap() < positions.get(&10).unwrap());
        // 11 -> 2, 11 -> 9, 11 -> 10
        assert!(positions.get(&11).unwrap() < positions.get(&2).unwrap());
        assert!(positions.get(&11).unwrap() < positions.get(&9).unwrap());
        assert!(positions.get(&11).unwrap() < positions.get(&10).unwrap());
        // 8 -> 9
        assert!(positions.get(&8).unwrap() < positions.get(&9).unwrap());
    }

    #[test]
    fn test_topological_sort_with_cycle() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1); // Creates a cycle

        let result = graph.topological_sort();
        assert!(result.is_none()); // Should fail due to cycle
    }

    #[test]
    fn test_has_cycles() {
        let mut acyclic_graph = Graph::new();
        acyclic_graph.add_edge(1, 2);
        acyclic_graph.add_edge(2, 3);
        assert!(!acyclic_graph.has_cycles());

        let mut cyclic_graph = Graph::new();
        cyclic_graph.add_edge(1, 2);
        cyclic_graph.add_edge(2, 3);
        cyclic_graph.add_edge(3, 1);
        assert!(cyclic_graph.has_cycles());
    }

    #[test]
    fn test_self_loop_cycle() {
        let mut graph = Graph::new();
        graph.add_edge(1, 1); // Self-loop
        assert!(graph.has_cycles());
    }

    #[test]
    fn test_strongly_connected_components_simple() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);

        let components = graph.strongly_connected_components();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].len(), 3);
    }

    #[test]
    fn test_strongly_connected_components_multiple() {
        let mut graph = Graph::new();
        // First SCC: 1->2->3->1
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);
        // Second SCC: 4->5->4
        graph.add_edge(4, 5);
        graph.add_edge(5, 4);
        // Connection between SCCs
        graph.add_edge(3, 4);

        let components = graph.strongly_connected_components();
        assert_eq!(components.len(), 2);
        
        // Find the components by size
        let mut sizes: Vec<usize> = components.iter().map(|c| c.len()).collect();
        sizes.sort();
        let mut expected = Vec::new();
        expected.push(2);
        expected.push(3);
        assert_eq!(sizes, expected);
    }

    #[test]
    fn test_dependencies() {
        let mut graph = Graph::new();
        graph.add_edge(1, 2);
        graph.add_edge(1, 3);
        graph.add_edge(2, 4);
        graph.add_edge(3, 4);

        let deps = graph.dependencies(&1);
        assert_eq!(deps.len(), 3); // Should include 2, 3, 4
        assert!(deps.contains(&2));
        assert!(deps.contains(&3));
        assert!(deps.contains(&4));

        let deps2 = graph.dependencies(&2);
        assert_eq!(deps2.len(), 1); // Should include only 4
        assert!(deps2.contains(&4));
    }

    #[test]
    fn test_dependents() {
        let mut graph = Graph::new();
        graph.add_edge(1, 4);
        graph.add_edge(2, 4);
        graph.add_edge(3, 4);
        graph.add_edge(4, 5);

        let deps = graph.dependents(&4);
        assert_eq!(deps.len(), 3); // Should include 1, 2, 3
        assert!(deps.contains(&1));
        assert!(deps.contains(&2));
        assert!(deps.contains(&3));

        let deps5 = graph.dependents(&5);
        assert_eq!(deps5.len(), 1); // Should include only 4
        assert!(deps5.contains(&4));
    }

    #[test]
    fn test_module_graph_creation() {
        let graph = ModuleGraph::new();
        assert_eq!(graph.modules().len(), 0);
    }

    #[test]
    fn test_module_graph_add_modules() {
        let mut graph = ModuleGraph::new();
        graph.add_module("std");
        graph.add_module("collections");
        graph.add_module("main");

        let modules = graph.modules();
        assert_eq!(modules.len(), 3);
        assert!(modules.contains(&String::from("std")));
        assert!(modules.contains(&String::from("collections")));
        assert!(modules.contains(&String::from("main")));
    }

    #[test]
    fn test_module_graph_dependencies() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("main", "collections");
        graph.add_dependency("collections", "std");
        graph.add_dependency("main", "std");

        let main_deps = graph.module_dependencies("main");
        assert_eq!(main_deps.len(), 2); // collections and std
        assert!(main_deps.contains(&String::from("collections")));
        assert!(main_deps.contains(&String::from("std")));

        let collections_deps = graph.module_dependencies("collections");
        assert_eq!(collections_deps.len(), 1); // only std
        assert!(collections_deps.contains(&String::from("std")));
    }

    #[test]
    fn test_module_compilation_order() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("main", "collections");
        graph.add_dependency("collections", "std");
        graph.add_dependency("main", "io");
        graph.add_dependency("io", "std");

        let order = graph.compilation_order().unwrap();
        assert_eq!(order.len(), 4);

        // std should be compiled first
        let std_pos = order.iter().position(|x| x == "std").unwrap();
        let collections_pos = order.iter().position(|x| x == "collections").unwrap();
        let io_pos = order.iter().position(|x| x == "io").unwrap();
        let main_pos = order.iter().position(|x| x == "main").unwrap();

        assert!(std_pos < collections_pos);
        assert!(std_pos < io_pos);
        assert!(collections_pos < main_pos);
        assert!(io_pos < main_pos);
    }

    #[test]
    fn test_module_circular_dependencies() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("a", "b");
        graph.add_dependency("b", "c");
        graph.add_dependency("c", "a"); // Creates cycle

        assert!(graph.has_circular_dependencies());
        assert!(graph.compilation_order().is_none());

        let circular_groups = graph.circular_dependency_groups();
        assert_eq!(circular_groups.len(), 1);
        assert_eq!(circular_groups[0].len(), 3);
    }

    #[test]
    fn test_module_self_dependency() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("module", "module"); // Self-dependency

        assert!(graph.has_circular_dependencies());
        let circular_groups = graph.circular_dependency_groups();
        assert_eq!(circular_groups.len(), 1);
        assert_eq!(circular_groups[0].len(), 1);
        assert_eq!(circular_groups[0][0], String::from("module"));
    }

    #[test]
    fn test_module_dependents() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("app", "std");
        graph.add_dependency("lib", "std");
        graph.add_dependency("tests", "std");

        let std_dependents = graph.module_dependents("std");
        assert_eq!(std_dependents.len(), 3);
        assert!(std_dependents.contains(&String::from("app")));
        assert!(std_dependents.contains(&String::from("lib")));
        assert!(std_dependents.contains(&String::from("tests")));
    }

    #[test]
    fn test_dependency_statistics() {
        let mut graph = ModuleGraph::new();
        graph.add_dependency("main", "std");
        graph.add_dependency("lib", "std");
        graph.add_dependency("main", "lib");

        let stats = graph.statistics();
        assert_eq!(stats.total_modules, 3);
        assert_eq!(stats.total_dependencies, 3);
        assert!(!stats.has_cycles);
        assert_eq!(stats.strongly_connected_components, 3); // Each node is its own SCC
    }

    #[test]
    fn test_large_dependency_graph() {
        let mut graph = ModuleGraph::new();
        
        // Create a larger graph: each module depends on previous ones
        for i in 1..=100 {
            let module_name = format!("module_{}", i);
            if i > 1 {
                let prev_module = format!("module_{}", i - 1);
                graph.add_dependency(&module_name, &prev_module);
            }
            if i > 10 {
                // Also depend on module 10 levels back
                let back_module = format!("module_{}", i - 10);
                graph.add_dependency(&module_name, &back_module);
            }
        }

        let order = graph.compilation_order().unwrap();
        assert_eq!(order.len(), 100);
        
        // module_1 should be first
        assert_eq!(order[0], String::from("module_1"));
        // module_100 should be last  
        assert_eq!(order[99], String::from("module_100"));

        let stats = graph.statistics();
        assert_eq!(stats.total_modules, 100);
        assert!(!stats.has_cycles);
    }

    #[test]
    fn test_compiler_realistic_scenario() {
        let mut graph = ModuleGraph::new();
        
        // Realistic compiler module dependencies
        graph.add_dependency("main", "cli");
        graph.add_dependency("main", "compiler");
        graph.add_dependency("cli", "args");
        graph.add_dependency("cli", "filesystem");
        graph.add_dependency("compiler", "lexer");
        graph.add_dependency("compiler", "parser");
        graph.add_dependency("compiler", "typechecker");
        graph.add_dependency("compiler", "codegen");
        graph.add_dependency("lexer", "string_utils");
        graph.add_dependency("parser", "lexer");
        graph.add_dependency("parser", "ast");
        graph.add_dependency("typechecker", "parser");
        graph.add_dependency("typechecker", "ast");
        graph.add_dependency("codegen", "typechecker");
        graph.add_dependency("codegen", "ast");
        graph.add_dependency("ast", "string_utils");
        
        let order = graph.compilation_order().unwrap();
        
        // Verify some key ordering constraints
        let mut positions = HashMap::new();
        for (i, name) in order.iter().enumerate() {
            positions.insert(name.clone(), i);
        }
            
        // string_utils should come before lexer and ast
        assert!(positions.get(&String::from("string_utils")).unwrap() < positions.get(&String::from("lexer")).unwrap());
        assert!(positions.get(&String::from("string_utils")).unwrap() < positions.get(&String::from("ast")).unwrap());
        
        // lexer should come before parser
        assert!(positions.get(&String::from("lexer")).unwrap() < positions.get(&String::from("parser")).unwrap());
        
        // parser should come before typechecker
        assert!(positions.get(&String::from("parser")).unwrap() < positions.get(&String::from("typechecker")).unwrap());
        
        // typechecker should come before codegen
        assert!(positions.get(&String::from("typechecker")).unwrap() < positions.get(&String::from("codegen")).unwrap());
        
        // compiler should come before main
        assert!(positions.get(&String::from("compiler")).unwrap() < positions.get(&String::from("main")).unwrap());

        let stats = graph.statistics();
        assert!(!stats.has_cycles);
        assert!(stats.total_dependencies > 10);
    }

    #[test]
    fn test_performance_large_graph() {
        let mut graph = Graph::new();
        
        // Create a large graph with many dependencies
        for i in 0..1000 {
            graph.add_node(i);
            if i > 0 {
                graph.add_edge(i, i - 1); // Linear dependency chain
            }
            if i > 10 {
                graph.add_edge(i, i - 10); // Additional dependencies
            }
        }

        let start = std::time::Instant::now();
        let result = graph.topological_sort().unwrap();
        let duration = start.elapsed();

        assert_eq!(result.len(), 1000);
        assert!(duration.as_millis() < 100); // Should be fast (<100ms)
    }

    #[test]
    fn test_empty_graph_operations() {
        let graph: Graph<String> = Graph::new();
        
        assert_eq!(graph.topological_sort(), Some(Vec::new()));
        assert!(!graph.has_cycles());
        assert_eq!(graph.strongly_connected_components(), Vec::<Vec<String>>::new());
        
        let module_graph = ModuleGraph::new();
        assert_eq!(module_graph.compilation_order(), Some(Vec::new()));
        assert!(!module_graph.has_circular_dependencies());
        assert_eq!(module_graph.circular_dependency_groups().len(), 0);
    }
}