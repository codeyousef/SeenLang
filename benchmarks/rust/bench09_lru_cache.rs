use std::collections::HashMap;
use std::time::Instant;

const CAPACITY: usize = 100_000;
const OPERATIONS: usize = 10_000_000;
const KEY_RANGE: i64 = 1_000_000;

struct Node {
    key: i64,
    value: i64,
    prev: usize,
    next: usize,
}

struct LRUCache {
    capacity: usize,
    map: HashMap<i64, usize>,
    nodes: Vec<Node>,
    head: usize,
    tail: usize,
    free_list: Vec<usize>,
}

impl LRUCache {
    fn new(capacity: usize) -> Self {
        let mut nodes = Vec::with_capacity(capacity + 2);
        nodes.push(Node { key: 0, value: 0, prev: 0, next: 1 });
        nodes.push(Node { key: 0, value: 0, prev: 0, next: 1 });

        LRUCache {
            capacity,
            map: HashMap::with_capacity((capacity as f64 * 1.5) as usize),
            nodes,
            head: 0,
            tail: 1,
            free_list: Vec::new(),
        }
    }

    fn get(&mut self, key: i64) -> Option<i64> {
        if let Some(&node_idx) = self.map.get(&key) {
            let value = self.nodes[node_idx].value;
            self.move_to_front(node_idx);
            Some(value)
        } else {
            None
        }
    }

    fn put(&mut self, key: i64, value: i64) {
        if let Some(&node_idx) = self.map.get(&key) {
            self.nodes[node_idx].value = value;
            self.move_to_front(node_idx);
        } else {
            if self.map.len() >= self.capacity {
                self.evict_lru();
            }

            let node_idx = if let Some(idx) = self.free_list.pop() {
                idx
            } else {
                let idx = self.nodes.len();
                self.nodes.push(Node { key, value, prev: 0, next: 0 });
                idx
            };

            self.nodes[node_idx].key = key;
            self.nodes[node_idx].value = value;
            self.map.insert(key, node_idx);
            self.add_to_front(node_idx);
        }
    }

    fn move_to_front(&mut self, node_idx: usize) {
        self.remove_node(node_idx);
        self.add_to_front(node_idx);
    }

    fn remove_node(&mut self, node_idx: usize) {
        let prev = self.nodes[node_idx].prev;
        let next = self.nodes[node_idx].next;
        self.nodes[prev].next = next;
        self.nodes[next].prev = prev;
    }

    fn add_to_front(&mut self, node_idx: usize) {
        let old_next = self.nodes[self.head].next;
        self.nodes[self.head].next = node_idx;
        self.nodes[node_idx].prev = self.head;
        self.nodes[node_idx].next = old_next;
        self.nodes[old_next].prev = node_idx;
    }

    fn evict_lru(&mut self) {
        let lru_idx = self.nodes[self.tail].prev;
        let key = self.nodes[lru_idx].key;
        self.remove_node(lru_idx);
        self.map.remove(&key);
        self.free_list.push(lru_idx);
    }
}

fn main() {
    let mut rng_state: u64 = 42;
    let mut next_random = || -> i64 {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        ((rng_state / 65536) % KEY_RANGE as u64) as i64
    };

    let mut cache = LRUCache::new(CAPACITY);

    for _ in 0..50_000 {
        let key = next_random();
        cache.put(key, key * 2);
    }

    rng_state = 42;
    for _ in 0..50_000 {
        let _ = next_random();
    }

    let start = Instant::now();
    let mut sum = 0i64;

    for i in 0..OPERATIONS {
        let key = next_random();

        if i % 10 < 7 {
            if let Some(value) = cache.get(key) {
                sum = sum.wrapping_add(value);
            }
        } else {
            cache.put(key, key * 2);
        }
    }

    let elapsed = start.elapsed();

    println!("Sum of values: {}", sum);
    println!("Time: {:.3} ms", elapsed.as_secs_f64() * 1000.0);
    println!("Operations/sec: {:.2}M", OPERATIONS as f64 / elapsed.as_secs_f64() / 1_000_000.0);
}
