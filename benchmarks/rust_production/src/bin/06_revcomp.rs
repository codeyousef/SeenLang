// Reverse Complement Benchmark
// Faithful port of benchmarks/production/06_revcomp.seen
// Same complement table, same LCG, same sequence generation

use std::time::Instant;

fn create_complement_table() -> Vec<i64> {
    let mut table: Vec<i64> = (0..256).map(|i| i as i64).collect();

    // Uppercase
    table[65] = 84;   // A -> T
    table[67] = 71;   // C -> G
    table[71] = 67;   // G -> C
    table[84] = 65;   // T -> A
    table[85] = 65;   // U -> A
    table[77] = 75;   // M -> K
    table[82] = 89;   // R -> Y
    table[87] = 87;   // W -> W
    table[83] = 83;   // S -> S
    table[89] = 82;   // Y -> R
    table[75] = 77;   // K -> M
    table[86] = 66;   // V -> B
    table[72] = 68;   // H -> D
    table[68] = 72;   // D -> H
    table[66] = 86;   // B -> V
    table[78] = 78;   // N -> N

    // Lowercase
    table[97] = 116;  // a -> t
    table[99] = 103;  // c -> g
    table[103] = 99;  // g -> c
    table[116] = 97;  // t -> a
    table[117] = 97;  // u -> a
    table[109] = 107;  // m -> k
    table[114] = 121;  // r -> y
    table[119] = 119;  // w -> w
    table[115] = 115;  // s -> s
    table[121] = 114;  // y -> r
    table[107] = 109;  // k -> m
    table[118] = 98;   // v -> b
    table[104] = 100;  // h -> d
    table[100] = 104;  // d -> h
    table[98] = 118;   // b -> v
    table[110] = 110;  // n -> n

    table
}

fn reverse_complement(seq: &[i64], table: &[i64]) -> Vec<i64> {
    let len = seq.len();
    let mut result = Vec::with_capacity(len);

    let mut i = len as i64 - 1;
    while i >= 0 {
        let complement = table[seq[i as usize] as usize];
        result.push(complement);
        i -= 1;
    }

    result
}

fn generate_sequence(n: usize, seed: i64) -> Vec<i64> {
    let mut seq = Vec::with_capacity(n);
    let bases: [i64; 4] = [65, 67, 71, 84];

    let mut current_seed = seed;
    let mut i = 0;
    while i < n {
        current_seed = (current_seed * 1103515245 + 12345) % 2147483647;
        if current_seed < 0 {
            current_seed = -current_seed;
        }
        let idx = (current_seed % 4) as usize;
        seq.push(bases[idx]);
        i += 1;
    }

    seq
}

fn compute_checksum(seq: &[i64]) -> i64 {
    let mut sum = 0i64;
    for &val in seq {
        sum += val;
    }
    sum
}

fn main() {
    let n: usize = 5_000_000;

    println!("Reverse Complement Benchmark");
    println!("Sequence length: {}", n);

    let table = create_complement_table();
    let seq = generate_sequence(n, 42);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = reverse_complement(&seq, &table);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut best_result: Vec<i64> = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();
        let result = reverse_complement(&seq, &table);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            best_result = result;
        }
    }

    let checksum = compute_checksum(&best_result);

    println!("Checksum: {}", checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Throughput: {:.9} Mbp/s", n as f64 / (min_time / 1000.0) / 1_000_000.0);
}
