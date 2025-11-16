use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

const CONNECTIONS: usize = 1000;
const REQUESTS_PER_CONN: usize = 10;
const PAYLOAD_SIZE: usize = 100;

fn handle_client(mut stream: TcpStream, counter: Arc<AtomicUsize>) {
    stream.set_nonblocking(false).ok();
    let mut buffer = [0u8; PAYLOAD_SIZE];

    for _ in 0..REQUESTS_PER_CONN {
        if let Ok(n) = stream.read(&mut buffer) {
            if n == 0 {
                break;
            }
            stream.write_all(&buffer[..n]).ok();
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn run_server(counter: Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    listener.set_nonblocking(false).ok();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let counter = counter.clone();
                thread::spawn(move || {
                    handle_client(stream, counter);
                });
            }
            Err(_) => break,
        }
    }
}

fn run_client() -> usize {
    let mut success_count = 0;
    let payload = vec![42u8; PAYLOAD_SIZE];

    for _ in 0..CONNECTIONS {
        if let Ok(mut stream) = TcpStream::connect("127.0.0.1:8080") {
            for _ in 0..REQUESTS_PER_CONN {
                if stream.write_all(&payload).is_ok() {
                    let mut buffer = vec![0u8; PAYLOAD_SIZE];
                    if stream.read_exact(&mut buffer).is_ok() {
                        success_count += 1;
                    }
                }
            }
        }
    }

    success_count
}

fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let server_thread = thread::spawn(move || {
        run_server(counter_clone);
    });

    thread::sleep(Duration::from_millis(100));

    let start = Instant::now();
    let success_count = run_client();
    let elapsed = start.elapsed();

    let total_requests = CONNECTIONS * REQUESTS_PER_CONN;
    let rps = total_requests as f64 / elapsed.as_secs_f64();
    let success_rate = (success_count as f64 / total_requests as f64) * 100.0;

    println!("Total requests: {}", total_requests);
    println!("Successful: {}", success_count);
    println!("Success rate: {:.2}%", success_rate);
    println!("Time: {:.3} s", elapsed.as_secs_f64());
    println!("Requests/second: {:.2}", rps);

    drop(server_thread);
}
