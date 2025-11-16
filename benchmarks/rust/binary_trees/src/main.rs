use std::time::Instant;

const DEPTH: usize = 20;

struct Node {
    item: i32,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn new(item: i32, depth: usize) -> Option<Box<Node>> {
        if depth == 0 {
            Some(Box::new(Node {
                item,
                left: None,
                right: None,
            }))
        } else {
            Some(Box::new(Node {
                item,
                left: Node::new(2 * item, depth - 1),
                right: Node::new(2 * item + 1, depth - 1),
            }))
        }
    }

    fn checksum(&self) -> i32 {
        self.item
            + self.left.as_ref().map_or(0, |n| n.checksum())
            - self.right.as_ref().map_or(0, |n| n.checksum())
    }
}

fn run_benchmark() -> (i32, usize) {
    let tree = Node::new(0, DEPTH).unwrap();
    let checksum = tree.checksum();
    let memory = std::mem::size_of::<Node>() * ((1 << (DEPTH + 1)) - 1);
    (checksum, memory)
}

fn main() {
    let mut times = Vec::new();
    let mut final_checksum = 0;
    let mut final_memory = 0;

    for i in 0..10 {
        let start = Instant::now();
        let (checksum, memory) = run_benchmark();
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64());
        final_checksum = checksum;
        final_memory = memory;
    }

    let avg_time = times.iter().sum::<f64>() / times.len() as f64;

    println!("Binary Trees (depth={})", DEPTH);
    println!("Checksum: {} (expected: -1)", final_checksum);
    println!("Memory allocated: {} bytes", final_memory);
    println!("Average time: {:.3} ms", avg_time * 1000.0);
}
