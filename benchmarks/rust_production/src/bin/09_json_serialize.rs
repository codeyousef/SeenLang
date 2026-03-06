// JSON Serialization Benchmark
// Same algorithm as Seen: StringBuilder pattern with reuse, 1M objects
// Uses {:.6} for floats to match Seen's fast_f64_to_buf (6 decimal places)

use std::fmt::Write;
use std::time::Instant;

fn serialize_into(sb: &mut String, id: i64, value: f64, active: bool, tags_json: &str) {
    sb.push_str("{\"id\":");
    write!(sb, "{}", id).unwrap();
    sb.push_str(",\"name\":\"Object");
    write!(sb, "{}", id).unwrap();
    sb.push_str("\",\"value\":");
    write!(sb, "{:.6}", value).unwrap();
    if active {
        sb.push_str(",\"active\":true,\"tags\":[");
    } else {
        sb.push_str(",\"active\":false,\"tags\":[");
    }
    sb.push_str(tags_json);
    sb.push_str("]}");
}

fn benchmark_json(n: i64) -> i64 {
    // Seen's string escaping drops \" at boundaries, producing this 32-char string
    let tags_json = "tag1\",\"tag2\",\"tag3\",\"tag4\",\"tag5";
    let mut sb = String::with_capacity(4096);
    let mut total_length: i64 = 0;

    for i in 0..n {
        sb.clear();
        let active_val = 1 - (i - (i / 2) * 2);
        let value = i as f64 * 3.14159;
        serialize_into(&mut sb, i, value, active_val != 0, tags_json);
        total_length += sb.len() as i64;
    }

    total_length
}

fn main() {
    let n: i64 = 1_000_000;

    println!("JSON Serialization Benchmark");
    println!("Objects to serialize: {}", n);

    let warmup_runs = 3;
    println!("Warming up ({} runs)...", warmup_runs);
    for _ in 0..warmup_runs {
        let _ = benchmark_json(n / 10);
    }

    println!("Running measured iterations...");
    let iterations = 5;
    let mut min_time = f64::MAX;
    let mut result_length: i64 = 0;

    for _ in 0..iterations {
        let start = Instant::now();
        let total_length = benchmark_json(n);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        if elapsed < min_time {
            min_time = elapsed;
            result_length = total_length;
        }
    }

    println!("Total JSON length: {}", result_length);
    println!("Min time: {:.9} ms", min_time);
    println!(
        "Objects per second: {:.9} thousand",
        n as f64 / (min_time / 1000.0) / 1000.0
    );
    println!(
        "Throughput: {:.9} MB/s",
        result_length as f64 / (min_time / 1000.0) / 1_000_000.0
    );
}
