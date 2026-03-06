// Binary Trees Benchmark
// Faithful port of benchmarks/production/03_binary_trees.seen
// Same flattened tree approach: stores child check values as i64
// Uses Box<TreeNode> to match Seen's heap-allocated class instances

use std::time::Instant;

struct TreeNode {
    item: i64,
    left_val: i64,
    right_val: i64,
    has_left: bool,
    has_right: bool,
}

impl TreeNode {
    fn new_leaf(item: i64) -> Box<TreeNode> {
        Box::new(TreeNode {
            item,
            left_val: 0,
            right_val: 0,
            has_left: false,
            has_right: false,
        })
    }

    fn with_children(item: i64, left_check: i64, right_check: i64) -> Box<TreeNode> {
        Box::new(TreeNode {
            item,
            left_val: left_check,
            right_val: right_check,
            has_left: true,
            has_right: true,
        })
    }

    fn check(&self) -> i64 {
        let mut result = self.item;
        if self.has_left {
            result += self.left_val;
        }
        if self.has_right {
            result -= self.right_val;
        }
        result
    }
}

fn make_tree(depth: i64) -> Box<TreeNode> {
    if depth == 0 {
        return TreeNode::new_leaf(0);
    }
    let left = make_tree(depth - 1);
    let right = make_tree(depth - 1);
    let left_check = left.check();
    let right_check = right.check();
    TreeNode::with_children(0, left_check, right_check)
}

fn run_binary_trees(min_depth: i64, max_depth: i64) -> i64 {
    let stretch_depth = max_depth + 1;

    let stretch_tree = make_tree(stretch_depth);
    let stretch_check = stretch_tree.check();

    let long_lived_tree = make_tree(max_depth);

    let mut total_check = stretch_check;
    let mut depth = min_depth;
    while depth <= max_depth {
        let iterations = 1i64 << (max_depth - depth + min_depth);
        let mut check = 0i64;

        let mut i = 0;
        while i < iterations {
            let temp_tree = make_tree(depth);
            check += temp_tree.check();
            i += 1;
        }

        total_check += check;
        depth += 2;
    }

    total_check += long_lived_tree.check();
    total_check
}

fn main() {
    let min_depth = 4;
    let max_depth = 20;

    println!("Binary Trees Benchmark");
    println!("Max depth: {}", max_depth);

    // Warmup
    println!("Warming up (1 run at depth 16)...");
    let _ = run_binary_trees(min_depth, 16);

    println!("Running measured iterations...");
    let iterations = 3;
    let mut min_time = f64::MAX;
    let mut result_check = 0i64;

    for _ in 0..iterations {
        let start = Instant::now();
        let check = run_binary_trees(min_depth, max_depth);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_check = check;
        }
    }

    println!("Checksum: {}", result_check);
    println!("Min time: {:.9} ms", min_time);
}
