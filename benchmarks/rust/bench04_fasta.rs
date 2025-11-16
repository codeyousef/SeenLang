use std::io::{self, Write};
use std::time::Instant;

const N: usize = 250_000_000;
const LINE_LEN: usize = 60;
const BUFFER_SIZE: usize = 8192;
const IM: u32 = 139968;
const IA: u32 = 3877;
const IC: u32 = 29573;

static NUCLEOTIDES: &[(char, f32)] = &[
    ('A', 0.27),
    ('C', 0.12),
    ('G', 0.12),
    ('T', 0.27),
];

struct Random {
    seed: u32,
}

impl Random {
    fn new() -> Self {
        Random { seed: 42 }
    }

    fn next(&mut self) -> f32 {
        self.seed = (self.seed * IA + IC) % IM;
        self.seed as f32 / IM as f32
    }
}

fn make_cumulative() -> Vec<(char, f32)> {
    let mut cumulative = Vec::new();
    let mut sum = 0.0;
    for &(c, p) in NUCLEOTIDES {
        sum += p;
        cumulative.push((c, sum));
    }
    cumulative
}

fn select_nucleotide(r: f32, cumulative: &[(char, f32)]) -> char {
    for &(c, p) in cumulative {
        if r < p {
            return c;
        }
    }
    cumulative.last().unwrap().0
}

fn main() {
    let cumulative = make_cumulative();
    let mut rng = Random::new();
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut checksum: u64 = 0;

    let sink = io::sink();
    let mut writer = io::BufWriter::with_capacity(BUFFER_SIZE, sink);

    let start = Instant::now();

    writeln!(writer, ">Random sequence").unwrap();

    for i in 0..N {
        let r = rng.next();
        let nucleotide = select_nucleotide(r, &cumulative);
        buffer.push(nucleotide as u8);
        checksum += nucleotide as u64;

        if (i + 1) % LINE_LEN == 0 {
            buffer.push(b'\n');
            writer.write_all(&buffer).unwrap();
            buffer.clear();
        }
    }

    if !buffer.is_empty() {
        buffer.push(b'\n');
        writer.write_all(&buffer).unwrap();
    }

    writer.flush().unwrap();

    let elapsed = start.elapsed();

    println!("Checksum: {}", checksum);
    println!("Time: {:.3} ms", elapsed.as_secs_f64() * 1000.0);
}
