// FASTA Generation Benchmark
// Faithful port of benchmarks/production/04_fasta.seen
// Same LCG RNG, same frequency tables, same sequence sizes

use std::time::Instant;

struct Random {
    seed: i64,
}

impl Random {
    fn new(seed: i64) -> Random {
        Random { seed }
    }

    fn next(&mut self) -> f64 {
        self.seed = (self.seed * 3877 + 29573) % 139968;
        self.seed as f64 / 139968.0
    }
}

fn make_cumulative(freq_probs: &mut [f64]) {
    let mut cumulative = 0.0;
    for prob in freq_probs.iter_mut() {
        cumulative += *prob;
        *prob = cumulative;
    }
}

fn repeat_fasta(s: &[i64], n: usize) -> Vec<i64> {
    let mut result = Vec::with_capacity(n);
    let len = s.len();
    let mut i = 0;
    while i < n {
        result.push(s[i % len]);
        i += 1;
    }
    result
}

fn random_fasta(freq_chars: &[i64], freq_probs: &[f64], n: usize, rng: &mut Random) -> Vec<i64> {
    let mut result = Vec::with_capacity(n);
    let num_freqs = freq_probs.len();
    let mut i = 0;
    while i < n {
        let r = rng.next();
        let mut found = false;
        let mut si = 0;
        while si < num_freqs {
            if !found {
                if r < freq_probs[si] {
                    result.push(freq_chars[si]);
                    found = true;
                }
            }
            si += 1;
        }
        if !found {
            result.push(freq_chars[num_freqs - 1]);
        }
        i += 1;
    }
    result
}

fn compute_checksum(data: &[i64]) -> i64 {
    let mut sum = 0i64;
    for &val in data {
        sum += val;
    }
    sum
}

fn main() {
    let n: usize = 5_000_000;

    println!("FASTA Generation Benchmark");
    println!("Generating {} nucleotides", n);

    let alu_str = b"GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACTAAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAATCGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCGCCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";
    let alu: Vec<i64> = alu_str.iter().map(|&b| b as i64).collect();

    let hs_chars: Vec<i64> = vec![97, 99, 103, 116];
    let mut hs_probs: Vec<f64> = vec![0.3029549426680, 0.1979883004921, 0.1975473066391, 0.3015094502008];
    make_cumulative(&mut hs_probs);

    let iub_chars: Vec<i64> = vec![97, 99, 103, 116, 66, 68, 72, 75, 77, 78, 82, 83, 86, 87, 89];
    let mut iub_probs: Vec<f64> = vec![
        0.27, 0.12, 0.12, 0.27, 0.02, 0.02, 0.02, 0.02,
        0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02,
    ];
    make_cumulative(&mut iub_probs);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let mut rng_warmup = Random::new(42);
        let _ = repeat_fasta(&alu, n * 2);
        let _ = random_fasta(&iub_chars, &iub_probs, n * 3, &mut rng_warmup);
        let _ = random_fasta(&hs_chars, &hs_probs, n * 5, &mut rng_warmup);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut best_seq1: Vec<i64> = Vec::new();
    let mut best_seq2: Vec<i64> = Vec::new();
    let mut best_seq3: Vec<i64> = Vec::new();

    for _ in 0..iterations {
        let mut rng = Random::new(42);

        let start = Instant::now();
        let seq1 = repeat_fasta(&alu, n * 2);
        let seq2 = random_fasta(&iub_chars, &iub_probs, n * 3, &mut rng);
        let seq3 = random_fasta(&hs_chars, &hs_probs, n * 5, &mut rng);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            best_seq1 = seq1;
            best_seq2 = seq2;
            best_seq3 = seq3;
        }
    }

    let checksum = compute_checksum(&best_seq1) + compute_checksum(&best_seq2) + compute_checksum(&best_seq3);
    let total_bp = best_seq1.len() + best_seq2.len() + best_seq3.len();

    println!("Total base pairs: {}", total_bp);
    println!("Checksum: {}", checksum);
    println!("Min time: {:.9} ms", min_time);
    println!("Throughput: {:.9} Mbp/s", total_bp as f64 / (min_time / 1000.0) / 1_000_000.0);
}
