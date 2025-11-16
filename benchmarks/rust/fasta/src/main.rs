use std::time::Instant;
use std::io::{Write, sink};

const N: usize = 250_000_000;
const IM: u32 = 139968;
const IA: u32 = 3877;
const BUFFER_SIZE: usize = 8192;

struct Random {
    last: u32,
}

impl Random {
    fn new(seed: u32) -> Self {
        Random { last: seed }
    }

    fn next(&mut self) -> f32 {
        self.last = (self.last * IA) % IM;
        self.last as f32 / IM as f32
    }
}

fn generate_fasta(n: usize) -> (u64, Vec<u8>) {
    let nucleotides = [b'A', b'C', b'G', b'T'];
    let probabilities = [0.25, 0.5, 0.75, 1.0];

    let mut rng = Random::new(42);
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut checksum = 0u64;
    let mut count = 0;

    buffer.extend_from_slice(b">HEADER\n");

    for _ in 0..n {
        let r = rng.next();
        let nucleotide = nucleotides
            .iter()
            .zip(probabilities.iter())
            .find(|(_, &p)| r < p)
            .map(|(&n, _)| n)
            .unwrap_or(b'T');

        buffer.push(nucleotide);
        checksum = checksum.wrapping_add(nucleotide as u64);
        count += 1;

        if count % 60 == 0 {
            buffer.push(b'\n');
        }

        if buffer.len() >= BUFFER_SIZE - 100 {
            buffer.clear();
        }
    }

    if count % 60 != 0 {
        buffer.push(b'\n');
    }

    (checksum, buffer)
}

fn main() {
    let mut times = Vec::new();
    let mut final_checksum = 0;

    for _ in 0..5 {
        let start = Instant::now();
        let (checksum, _buffer) = generate_fasta(N);
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64());
        final_checksum = checksum;
    }

    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];

    println!("FASTA Generation (n={})", N);
    println!("Checksum: {}", final_checksum);
    println!("Median time: {:.3} ms", median * 1000.0);
}
