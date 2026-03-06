// LRU Cache Benchmark
// Real HashMap + array-based doubly-linked list for O(1) get/put
// Same algorithm as Seen: HashMap for key->node index, parallel arrays for linked list

use std::collections::HashMap;
use std::time::Instant;

fn benchmark_lru(n: i64, capacity: usize) -> i64 {
    let mut map: HashMap<i64, usize> = HashMap::with_capacity(capacity);

    let mut keys = vec![0i64; capacity];
    let mut values = vec![0i64; capacity];
    let mut prev = vec![usize::MAX; capacity];
    let mut next = vec![usize::MAX; capacity];

    let mut free_list: Vec<usize> = (0..capacity).collect();
    let mut free_count = capacity;

    let mut head: usize = usize::MAX;
    let mut tail: usize = usize::MAX;
    let mut size = 0usize;
    let mut checksum = 0i64;

    let mut rng = 42i64;

    for i in 0..n {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let key = rng.abs();

        let rem2 = i % 10;
        let do_get = rem2 < 7;

        if do_get {
            // GET operation
            if let Some(&node_idx) = map.get(&key) {
                checksum += values[node_idx];

                // Move to front
                if node_idx != head {
                    let p = prev[node_idx];
                    let nx = next[node_idx];
                    if p != usize::MAX {
                        next[p] = nx;
                    }
                    if nx != usize::MAX {
                        prev[nx] = p;
                    }
                    if node_idx == tail {
                        tail = p;
                    }
                    prev[node_idx] = usize::MAX;
                    next[node_idx] = head;
                    if head != usize::MAX {
                        prev[head] = node_idx;
                    }
                    head = node_idx;
                }
            }
        } else {
            // PUT operation
            if let Some(&node_idx) = map.get(&key) {
                values[node_idx] = key * 2;

                // Move to front
                if node_idx != head {
                    let p = prev[node_idx];
                    let nx = next[node_idx];
                    if p != usize::MAX {
                        next[p] = nx;
                    }
                    if nx != usize::MAX {
                        prev[nx] = p;
                    }
                    if node_idx == tail {
                        tail = p;
                    }
                    prev[node_idx] = usize::MAX;
                    next[node_idx] = head;
                    if head != usize::MAX {
                        prev[head] = node_idx;
                    }
                    head = node_idx;
                }
            } else {
                // Insert new
                let node_idx;
                if free_count > 0 {
                    free_count -= 1;
                    node_idx = free_list[free_count];
                    size += 1;
                } else {
                    // Evict LRU (tail)
                    node_idx = tail;
                    let old_key = keys[node_idx];
                    map.remove(&old_key);

                    let p = prev[node_idx];
                    tail = p;
                    if p != usize::MAX {
                        next[p] = usize::MAX;
                    } else {
                        head = usize::MAX;
                    }
                }

                keys[node_idx] = key;
                values[node_idx] = key * 2;
                prev[node_idx] = usize::MAX;
                next[node_idx] = head;
                if head != usize::MAX {
                    prev[head] = node_idx;
                }
                head = node_idx;
                if tail == usize::MAX {
                    tail = node_idx;
                }
                map.insert(key, node_idx);
            }
        }
    }

    checksum
}

fn main() {
    let n = 10_000_000i64;
    let capacity = 100_000usize;

    println!("LRU Cache Benchmark");
    println!("Operations: {}", n);
    println!("Capacity: {}", capacity);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = benchmark_lru(n / 10, capacity);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result_checksum = 0i64;

    for _ in 0..iterations {
        let start = Instant::now();
        let checksum = benchmark_lru(n, capacity);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_checksum = checksum;
        }
    }

    println!("Checksum: {}", result_checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Operations per second: {:.9} million", n as f64 / (min_time / 1000.0) / 1_000_000.0);
}
