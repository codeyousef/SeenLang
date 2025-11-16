// Benchmark 3: Binary Trees
// Measures: Memory allocator performance, pointer chasing

use std::time::Instant;

struct Node {
    item: i32,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn new(item: i32, depth: usize) -> Box<Node> {
        if depth == 0 {
            Box::new(Node {
                item,
                left: None,
                right: None,
            })
        } else {
            Box::new(Node {
                item,
                left: Some(Node::new(item * 2, depth - 1)),
                right: Some(Node::new(item * 2 + 1, depth - 1)),
            })
        }
    }

    fn checksum(&self) -> i32 {
        self.item
            + self.left.as_ref().map_or(0, |n| n.checksum())
            - self.right.as_ref().map_or(0, |n| n.checksum())
    }
}

fn benchmark_iteration(depth: usize) -> (i32, usize) {
    let tree = Node::new(0, depth);
    let checksum = tree.checksum();
    let nodes = (1 << (depth + 1)) - 1; // 2^(depth+1) - 1
    (checksum, nodes * std::mem::size_of::<Node>())
}

fn main() {
    const DEPTH: usize = 20;
    const ITERATIONS: usize = 10;

    println!("Benchmark 3: Binary Trees");
    println!("Depth: {}", DEPTH);
    println!("Expected nodes: {} ({})", (1 << (DEPTH + 1)) - 1, 1_048_575);

    // Warmup
    for _ in 0..3 {
        let _ = benchmark_iteration(DEPTH);
    }

    // Measured runs
    let mut times = Vec::new();
    let mut total_checksum = 0;
    let mut total_memory = 0;

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let (checksum, memory) = benchmark_iteration(DEPTH);
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64() * 1000.0);
        total_checksum += checksum;
        total_memory = memory; // Same for all iterations
    }

    let avg_time = times.iter().sum::<f64>() / times.len() as f64;
    let avg_checksum = total_checksum / ITERATIONS as i32;

    println!("Average checksum: {} (expected -1)", avg_checksum);
    println!("Memory per tree: {} bytes", total_memory);
    println!("Average time: {:.2} ms", avg_time);
    println!("All times: {:?}", times.iter().map(|t| format!("{:.2}", t)).collect::<Vec<_>>());
}
