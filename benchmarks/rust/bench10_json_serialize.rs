use std::time::Instant;

const N_OBJECTS: usize = 1_000_000;

#[derive(Clone)]
struct Record {
    id: i32,
    name: String,
    active: bool,
    score: f64,
}

fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\x08' => result.push_str("\\b"),
            '\x0c' => result.push_str("\\f"),
            _ => result.push(c),
        }
    }
    result
}

fn serialize_records(records: &[Record]) -> String {
    let estimated_size = N_OBJECTS * 80;
    let mut json = String::with_capacity(estimated_size);

    json.push('[');

    for (i, record) in records.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }

        json.push_str("{\"id\":");
        json.push_str(&record.id.to_string());
        json.push_str(",\"name\":\"");
        json.push_str(&escape_string(&record.name));
        json.push_str("\",\"active\":");
        json.push_str(if record.active { "true" } else { "false" });
        json.push_str(",\"score\":");
        json.push_str(&format!("{}", record.score));
        json.push('}');
    }

    json.push(']');
    json
}

fn main() {
    let mut records = Vec::with_capacity(N_OBJECTS);
    for i in 0..N_OBJECTS {
        records.push(Record {
            id: i as i32,
            name: format!("User_{:05}", i),
            active: i % 2 == 0,
            score: (i as f64) * 1.5,
        });
    }

    for _ in 0..3 {
        let _ = serialize_records(&records);
    }

    let runs = 10;
    let mut times = Vec::with_capacity(runs);

    for _ in 0..runs {
        let start = Instant::now();
        let json = serialize_records(&records);
        let elapsed = start.elapsed();
        times.push((elapsed.as_secs_f64() * 1000.0, json.len()));
    }

    let min_time = times.iter().map(|(t, _)| *t).fold(f64::INFINITY, f64::min);
    let bytes_written = times[0].1;

    let json = serialize_records(&records);
    let checksum: u64 = json.bytes().map(|b| b as u64).sum();

    println!("Bytes written: {}", bytes_written);
    println!("Checksum: {}", checksum);
    println!("Min time: {:.3} ms", min_time);
    println!("Throughput: {:.2} MB/s", (bytes_written as f64 / 1_000_000.0) / (min_time / 1000.0));
}
