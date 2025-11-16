use std::collections::HashMap;
use std::time::Instant;

const CAPACITY: usize = 100_000;
const N_OPS: usize = 10_000_000;

struct Node {
    key: usize,
    value: i32,
    prev: Option<usize>,
    next: Option<usize>,
}

struct LRUCache {
    capacity: usize,
    map: HashMap<usize, usize>,
    nodes: Vec<Node>,
    head: Option<usize>,
    tail: Option<usize>,
    free_list: Vec<usize>,
}

impl LRUCache {
    fn new(capacity: usize) -> Self {
        LRUCache {
            capacity,
            map: HashMap::with_capacity((capacity as f64 * 1.5) as usize),
            nodes: Vec::with_capacity(capacity),
            head: None,
            tail: None,
            free_list: Vec::new(),
        }
    }

    fn get(&mut self, key: usize) -> Option<i32> {
        if let Some(&node_idx) = self.map.get(&key) {
            let value = self.nodes[node_idx].value;
            self.move_to_front(node_idx);
            Some(value)
        } else {
            None
        }
    }

    fn put(&mut self, key: usize, value: i32) {
        if let Some(&node_idx) = self.map.get(&key) {
            self.nodes[node_idx].value = value;
            self.move_to_front(node_idx);
        } else {
            if self.map.len() >= self.capacity {
                self.evict_lru();
            }

            let node_idx = if let Some(idx) = self.free_list.pop() {
                self.nodes[idx] = Node {
                    key,
                    value,
                    prev: None,
                    next: self.head,
                };
                idx
            } else {
                let idx = self.nodes.len();
                self.nodes.push(Node {
                    key,
                    value,
                    prev: None,
                    next: self.head,
                });
                idx
            };

            if let Some(head_idx) = self.head {
                self.nodes[head_idx].prev = Some(node_idx);
            } else {
                self.tail = Some(node_idx);
            }

            self.head = Some(node_idx);
            self.map.insert(key, node_idx);
        }
    }

    fn move_to_front(&mut self, node_idx: usize) {
        if self.head == Some(node_idx) {
            return;
        }

        let prev_idx = self.nodes[node_idx].prev;
        let next_idx = self.nodes[node_idx].next;

        if let Some(prev) = prev_idx {
            self.nodes[prev].next = next_idx;
        }

        if let Some(next) = next_idx {
            self.nodes[next].prev = prev_idx;
        } else {
            self.tail = prev_idx;
        }

        self.nodes[node_idx].prev = None;
        self.nodes[node_idx].next = self.head;

        if let Some(head_idx) = self.head {
            self.nodes[head_idx].prev = Some(node_idx);
        }

        self.head = Some(node_idx);
    }

    fn evict_lru(&mut self) {
        if let Some(tail_idx) = self.tail {
            let key = self.nodes[tail_idx].key;
            self.map.remove(&key);

            if let Some(prev_idx) = self.nodes[tail_idx].prev {
                self.nodes[prev_idx].next = None;
                self.tail = Some(prev_idx);
            } else {
                self.head = None;
                self.tail = None;
            }

            self.free_list.push(tail_idx);
        }
    }
}

fn generate_trace() -> Vec<(bool, usize, i32)> {
    let mut rng = 42u64;
    let mut trace = Vec::with_capacity(N_OPS);

    for _ in 0..N_OPS {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let is_get = (rng % 100) < 70;
        let key = ((rng / 65536) % 1_000_000) as usize;
        let value = ((rng / 256) % 1000) as i32;
        trace.push((is_get, key, value));
    }

    trace
}

fn main() {
    let trace = generate_trace();

    let mut cache = LRUCache::new(CAPACITY);
    for i in 0..50_000 {
        cache.put(i, (i * 7) as i32);
    }

    let start = Instant::now();
    let mut sum = 0i64;

    for &(is_get, key, value) in &trace {
        if is_get {
            if let Some(v) = cache.get(key) {
                sum += v as i64;
            }
        } else {
            cache.put(key, value);
        }
    }

    let elapsed = start.elapsed();

    println!("LRU Cache (capacity={}, ops={})", CAPACITY, N_OPS);
    println!("Sum of Get values: {}", sum);
    println!("Time: {:.3} ms", elapsed.as_secs_f64() * 1000.0);
}
