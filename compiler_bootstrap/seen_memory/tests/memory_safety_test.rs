//! Step 5 TDD Tests: Vale-style Memory Model
//! These tests MUST fail initially, then implementation makes them pass

use seen_memory::{RegionInference, GenerationalRef, MemoryAnalyzer, RuntimeManager};
use seen_typechecker::{TypeChecker, Type, PrimitiveType};
use std::time::Instant;

/// FAILING TEST: Memory management must have <5% overhead
/// This test MUST fail initially, then implementation makes it pass
#[test]
fn test_memory_overhead_under_5_percent() {
    let mut runtime = RuntimeManager::new();
    
    // Create test data structures for overhead measurement
    let test_iterations = 100_000;
    
    // Baseline: raw memory operations
    let start_baseline = Instant::now();
    let mut baseline_data = Vec::new();
    for i in 0..test_iterations {
        let data = format!("test_data_{}", i);
        baseline_data.push(data);
    }
    drop(baseline_data);
    let baseline_duration = start_baseline.elapsed();
    
    // Test: memory operations with region management
    let start_regions = Instant::now();
    let mut region_data = Vec::new();
    for i in 0..test_iterations {
        let data = format!("test_data_{}", i);
        let gen_ref = runtime.allocate_in_region(data, "main_region")
            .expect("Region allocation must succeed");
        region_data.push(gen_ref);
    }
    // Cleanup managed by region system
    runtime.cleanup_region("main_region").expect("Region cleanup must succeed");
    let regions_duration = start_regions.elapsed();
    
    // Calculate overhead percentage
    let overhead_ratio = regions_duration.as_nanos() as f64 / baseline_duration.as_nanos() as f64;
    let overhead_percentage = (overhead_ratio - 1.0) * 100.0;
    
    println!("Baseline: {:?}", baseline_duration);
    println!("Regions:  {:?}", regions_duration);
    println!("Overhead: {:.2}%", overhead_percentage);
    
    // HARD REQUIREMENT: <5% overhead
    const MAX_OVERHEAD_PERCENT: f64 = 5.0;
    assert!(
        overhead_percentage < MAX_OVERHEAD_PERCENT,
        "MEMORY OVERHEAD FAILED: {:.2}% overhead >= {:.2}% maximum",
        overhead_percentage,
        MAX_OVERHEAD_PERCENT
    );
}

/// FAILING TEST: Region inference must work automatically
#[test] 
fn test_automatic_region_inference() {
    let mut analyzer = MemoryAnalyzer::new();
    let mut type_checker = TypeChecker::new();
    
    // Test program with complex memory patterns
    let test_program = r#"
        func process_data() -> str {
            let local_data = "local string";
            let heap_data = allocate_string("heap string");
            let result = concat_strings(local_data, heap_data);
            return result;  // Should infer return region
        }
        
        func concurrent_access() {
            let shared_data = create_shared_buffer();
            spawn_thread(|data| process_buffer(data), shared_data);
            // Region inference should detect sharing
        }
    "#;
    
    // Parse and analyze memory regions
    let regions = analyzer.infer_regions(&test_program, &type_checker)
        .expect("Region inference must succeed");
    
    // REQUIREMENT: Must detect different region types
    assert!(regions.has_stack_region(), "Must detect stack region");
    assert!(regions.has_heap_region(), "Must detect heap region");
    assert!(regions.has_return_region(), "Must detect return region"); 
    assert!(regions.has_shared_region(), "Must detect shared region");
    
    // REQUIREMENT: Must infer region relationships
    let stack_region = regions.get_region("stack").expect("Stack region must exist");
    let heap_region = regions.get_region("heap").expect("Heap region must exist");
    
    assert!(stack_region.outlives(&heap_region), "Stack should outlive heap in this context");
    
    println!("✓ Region inference validation passed");
}

/// FAILING TEST: Generational references must prevent use-after-free
#[test]
fn test_generational_references_safety() {
    let mut runtime = RuntimeManager::new();
    
    // Allocate object in region
    let data = "test_data".to_string();
    let gen_ref = runtime.allocate_in_region(data, "test_region")
        .expect("Allocation must succeed");
    
    // Verify reference is valid
    assert!(gen_ref.is_valid(&runtime), "Reference should be initially valid");
    assert_eq!(gen_ref.get(&runtime).unwrap(), "test_data");
    
    // Deallocate the region
    runtime.cleanup_region("test_region").expect("Cleanup must succeed");
    
    // REQUIREMENT: Reference should now be invalid
    assert!(!gen_ref.is_valid(&runtime), "Reference should be invalid after cleanup");
    assert!(gen_ref.get(&runtime).is_none(), "Get should return None for invalid reference");
    
    // REQUIREMENT: Should not crash or cause undefined behavior
    for _ in 0..1000 {
        assert!(gen_ref.get(&runtime).is_none(), "Reference should remain safely invalid");
    }
    
    println!("✓ Generational references safety validation passed");
}

/// FAILING TEST: Escape analysis must detect memory leaks
#[test]
fn test_escape_analysis_detection() {
    let mut analyzer = MemoryAnalyzer::new();
    
    // Test cases with different escape patterns
    let leak_program = r#"
        func potential_leak() -> str {
            let local = "local data";
            let escaped = heap_allocate(local);
            return escaped;  // Escapes function scope
        }
        
        func no_leak() {
            let local = "local data"; 
            process_locally(local);
            // local doesn't escape
        }
        
        func shared_escape() {
            let data = create_buffer();
            store_globally(data);  // Escapes to global scope
        }
    "#;
    
    let escape_info = analyzer.analyze_escapes(&leak_program)
        .expect("Escape analysis must succeed");
    
    // REQUIREMENT: Must detect escaping variables
    assert!(escape_info.has_escaping_variable("escaped"), "Must detect 'escaped' variable");
    assert!(escape_info.has_escaping_variable("data"), "Must detect global escape");
    assert!(!escape_info.has_escaping_variable("local"), "Must not flag non-escaping variable");
    
    // REQUIREMENT: Must provide escape path information
    let escaped_info = escape_info.get_escape_info("escaped").expect("Escape info must exist");
    assert_eq!(escaped_info.escape_type(), "return_escape");
    assert_eq!(escaped_info.escape_scope(), "function_return");
    
    println!("✓ Escape analysis detection validation passed");
}

/// FAILING TEST: Memory regions must integrate with type system
#[test]
fn test_type_system_integration() {
    let mut type_checker = TypeChecker::new();
    let mut analyzer = MemoryAnalyzer::new();
    
    // Register Buffer struct type for integration testing
    let buffer_fields = vec![
        ("data".to_string(), Type::Primitive(PrimitiveType::Str)),
        ("size".to_string(), Type::Primitive(PrimitiveType::I32)),
    ];
    type_checker.env.bind("type:Buffer".to_string(), Type::Struct { 
        name: "Buffer".to_string(),
        fields: buffer_fields
    });
    
    // Test program with typed memory regions
    let typed_program = r#"
        struct Buffer {
            data: str,
            size: i32,
        }
        
        func create_buffer(data: str) -> Buffer {
            let buffer = Buffer { data: data, size: data.len() };
            return buffer;  // Type system should track region
        }
        
        func process_buffer(buf: &Buffer) {
            print(buf.data);  // Borrowed reference, different region
        }
    "#;
    
    // Integrate type checking with memory analysis  
    let integration_result = analyzer.integrate_with_types(&typed_program, &mut type_checker)
        .expect("Type-memory integration must succeed");
    
    // REQUIREMENT: Type system should be aware of regions
    let buffer_type = type_checker.env.get_type("Buffer").expect("Buffer type must exist");
    
    // REQUIREMENT: Memory analyzer should understand typed references
    assert!(integration_result.has_typed_region("Buffer"), "Must track Buffer type regions");
    assert!(integration_result.has_reference_type("&Buffer"), "Must track reference types");
    
    // REQUIREMENT: Should detect region conflicts
    let regions = integration_result.get_regions();
    assert!(regions.len() >= 2, "Should detect multiple regions for different scopes");
    
    println!("✓ Type system integration validation passed");
}