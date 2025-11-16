use std::time::Instant;

const DATA_SIZE: usize = 1_000_000_000;
const LINE_LEN: usize = 60;

fn make_complement_table() -> [u8; 256] {
    let mut table = [0u8; 256];
    for i in 0..256 {
        table[i] = i as u8;
    }
    table[b'A' as usize] = b'T';
    table[b'T' as usize] = b'A';
    table[b'C' as usize] = b'G';
    table[b'G' as usize] = b'C';
    table[b'a' as usize] = b't';
    table[b't' as usize] = b'a';
    table[b'c' as usize] = b'g';
    table[b'g' as usize] = b'c';
    table
}

fn reverse_complement(data: &mut Vec<u8>, complement: &[u8; 256]) {
    let mut i = 0;
    let mut j = data.len() - 1;

    while i < j {
        if data[i] == b'\n' {
            i += 1;
            continue;
        }
        if data[j] == b'\n' {
            j -= 1;
            continue;
        }

        let ci = complement[data[i] as usize];
        let cj = complement[data[j] as usize];
        data[i] = cj;
        data[j] = ci;

        i += 1;
        j -= 1;
    }

    if i == j && data[i] != b'\n' {
        data[i] = complement[data[i] as usize];
    }
}

fn main() {
    let complement = make_complement_table();

    let mut data = Vec::with_capacity(DATA_SIZE + DATA_SIZE / LINE_LEN);
    let nucleotides = b"ACGT";
    let mut rng_state: u64 = 42;

    for _ in 0..(DATA_SIZE / LINE_LEN) {
        for _ in 0..LINE_LEN {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let idx = ((rng_state / 65536) % 4) as usize;
            data.push(nucleotides[idx]);
        }
        data.push(b'\n');
    }

    let warmup_runs = 3;
    for _ in 0..warmup_runs {
        let mut temp = data.clone();
        reverse_complement(&mut temp, &complement);
    }

    let runs = 10;
    let mut times = Vec::with_capacity(runs);

    for _ in 0..runs {
        let mut temp = data.clone();
        let start = Instant::now();
        reverse_complement(&mut temp, &complement);
        let elapsed = start.elapsed();
        times.push(elapsed.as_secs_f64() * 1000.0);
    }

    let min_time = times.iter().cloned().fold(f64::INFINITY, f64::min);

    let mut test_data = data.clone();
    reverse_complement(&mut test_data, &complement);
    let checksum: u64 = test_data.iter().map(|&b| b as u64).sum();

    println!("Checksum: {}", checksum);
    println!("Min time: {:.3} ms", min_time);
    println!("Throughput: {:.2} MB/s", (DATA_SIZE as f64 / 1_000_000.0) / (min_time / 1000.0));
}
