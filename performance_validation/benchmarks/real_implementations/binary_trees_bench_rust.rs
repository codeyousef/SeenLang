use std::time::Instant;

struct Node {
    val: i32,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn new(val: i32) -> Self {
        Node { val, left: None, right: None }
    }
}

fn create_tree(depth: i32) -> Option<Box<Node>> {
    if depth == 0 {
        return None;
    }
    let mut node = Node::new(depth);
    node.left = create_tree(depth - 1);
    node.right = create_tree(depth - 1);
    Some(Box::new(node))
}

fn check_tree(node: &Option<Box<Node>>) -> i32 {
    match node {
        Some(n) => n.val + check_tree(&n.left) + check_tree(&n.right),
        None => 0,
    }
}

fn main() {
    let start = Instant::now();
    
    let depth = 10;
    let tree = create_tree(depth);
    let checksum = check_tree(&tree);
    
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;
    
    let total_nodes = 1_u32 << (depth + 1);
    let allocations_per_sec = total_nodes as f64 / duration.as_secs_f64();
    let memory_mb = total_nodes as f64 * std::mem::size_of::<Node>() as f64 / (1024.0 * 1024.0);
    
    // Output: creation_time_ms allocations_per_sec memory_mb checksum
    println!("{} {} {} {}", duration_ms, allocations_per_sec, memory_mb, checksum);
}