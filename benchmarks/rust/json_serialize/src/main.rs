use std::time::Instant;

const N: usize = 1_000_000;

struct User {
    id: usize,
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
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"),
            '\x0C' => result.push_str("\\f"),
            _ => result.push(c),
        }
    }
    result
}

fn serialize_users(users: &[User]) -> String {
    let estimated_size = users.len() * 80;
    let mut result = String::with_capacity(estimated_size);

    result.push('[');

    for (i, user) in users.iter().enumerate() {
        if i > 0 {
            result.push(',');
        }

        result.push_str("{\"id\":");
        result.push_str(&user.id.to_string());
        result.push_str(",\"name\":\"");
        result.push_str(&escape_string(&user.name));
        result.push_str("\",\"active\":");
        result.push_str(if user.active { "true" } else { "false" });
        result.push_str(",\"score\":");
        result.push_str(&format!("{}", user.score));
        result.push('}');
    }

    result.push(']');
    result
}

fn main() {
    let users: Vec<User> = (0..N)
        .map(|id| User {
            id,
            name: format!("User_{:05}", id),
            active: id % 2 == 0,
            score: id as f64 * 1.5,
        })
        .collect();

    let mut times = Vec::new();
    let mut final_checksum = String::new();
    let mut final_size = 0;

    for _ in 0..10 {
        let start = Instant::now();
        let json = serialize_users(&users);
        let elapsed = start.elapsed();

        times.push(elapsed.as_secs_f64());
        final_size = json.len();
        final_checksum = format!("{:x}", md5::compute(json.as_bytes()));
    }

    let min_time = times.iter().copied().fold(f64::INFINITY, f64::min);

    println!("JSON Serialization (n={})", N);
    println!("MD5 checksum: {}", final_checksum);
    println!("Total bytes: {}", final_size);
    println!("Min time: {:.3} ms", min_time * 1000.0);
}
