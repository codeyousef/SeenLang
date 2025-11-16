use std::time::Instant;

const BUFFER_SIZE: usize = 1_000_000_000;

fn create_complement_table() -> [u8; 256] {
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

fn generate_fasta_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let nucleotides = b"ACGT";
    let mut rng = 42u64;

    data.extend_from_slice(b">Sequence_1\n");

    let mut count = 0;
    while data.len() < size {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let nucleotide = nucleotides[(rng % 4) as usize];
        data.push(nucleotide);
        count += 1;

        if count % 60 == 0 {
            data.push(b'\n');
            if data.len() + 100 > size {
                break;
            }
            if count % 3600 == 0 {
                data.extend_from_slice(b">Sequence_");
                data.extend_from_slice((count / 3600).to_string().as_bytes());
                data.push(b'\n');
            }
        }
    }

    data
}

fn reverse_complement(data: &[u8]) -> Vec<u8> {
    let table = create_complement_table();
    let mut result = Vec::with_capacity(data.len());
    let mut sequences = Vec::new();
    let mut current_seq = Vec::new();

    for &byte in data {
        if byte == b'>' {
            if !current_seq.is_empty() {
                sequences.push(std::mem::take(&mut current_seq));
            }
            current_seq.push(byte);
        } else if byte == b'\n' && current_seq.first() == Some(&b'>') {
            let header = std::mem::take(&mut current_seq);
            sequences.push(header);
            current_seq = Vec::new();
        } else if byte != b'\n' {
            current_seq.push(byte);
        } else if byte == b'\n' && !current_seq.is_empty() {
            continue;
        }
    }

    if !current_seq.is_empty() {
        sequences.push(current_seq);
    }

    let mut i = 0;
    while i < sequences.len() {
        if sequences[i].first() == Some(&b'>') {
            result.extend_from_slice(&sequences[i]);
            result.push(b'\n');
            i += 1;

            if i < sequences.len() {
                let seq = &sequences[i];
                for j in (0..seq.len()).rev() {
                    result.push(table[seq[j] as usize]);
                    if (seq.len() - j) % 60 == 0 && j > 0 {
                        result.push(b'\n');
                    }
                }
                result.push(b'\n');
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    result
}

fn main() {
    println!("Generating {} bytes of FASTA data...", BUFFER_SIZE);
    let data = generate_fasta_data(BUFFER_SIZE);
    println!("Generated {} bytes", data.len());

    let mut times = Vec::new();
    let mut final_checksum = String::new();

    for i in 0..10 {
        let start = Instant::now();
        let result = reverse_complement(&data);
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64());

        if i == 9 {
            final_checksum = format!("{:x}", md5::compute(&result));
        }
    }

    let min_time = times.iter().copied().fold(f64::INFINITY, f64::min);

    println!("Reverse Complement (size={} bytes)", data.len());
    println!("MD5 checksum: {}", final_checksum);
    println!("Min time: {:.3} ms", min_time * 1000.0);
}
