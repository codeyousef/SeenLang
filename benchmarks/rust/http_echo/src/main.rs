use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const PORT: u16 = 8080;
const N_CONNECTIONS: usize = 10_000;
const N_REQUESTS: usize = 10;
const PAYLOAD_SIZE: usize = 100;

async fn handle_client(mut socket: TcpStream, counter: Arc<AtomicUsize>) {
    let mut buffer = vec![0u8; PAYLOAD_SIZE];

    for _ in 0..N_REQUESTS {
        match socket.read_exact(&mut buffer).await {
            Ok(_) => {
                if socket.write_all(&buffer).await.is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }

    counter.fetch_add(N_REQUESTS, Ordering::Relaxed);
}

async fn run_server(counter: Arc<AtomicUsize>) {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", PORT))
        .await
        .expect("Failed to bind");

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let counter = counter.clone();
                tokio::spawn(async move {
                    handle_client(socket, counter).await;
                });
            }
            Err(_) => continue,
        }
    }
}

async fn run_client() -> usize {
    let payload = vec![b'X'; PAYLOAD_SIZE];
    let mut buffer = vec![0u8; PAYLOAD_SIZE];

    let handles: Vec<_> = (0..N_CONNECTIONS)
        .map(|_| {
            let payload = payload.clone();
            tokio::spawn(async move {
                let mut socket = match TcpStream::connect(format!("127.0.0.1:{}", PORT)).await {
                    Ok(s) => s,
                    Err(_) => return 0,
                };

                let mut success = 0;
                for _ in 0..N_REQUESTS {
                    if socket.write_all(&payload).await.is_err() {
                        break;
                    }

                    let mut buf = vec![0u8; PAYLOAD_SIZE];
                    if socket.read_exact(&mut buf).await.is_err() {
                        break;
                    }

                    success += 1;
                }
                success
            })
        })
        .collect();

    let mut total = 0;
    for handle in handles {
        total += handle.await.unwrap_or(0);
    }

    total
}

#[tokio::main]
async fn main() {
    let counter = Arc::new(AtomicUsize::new(0));
    let server_counter = counter.clone();

    tokio::spawn(async move {
        run_server(server_counter).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let start = Instant::now();
    let successful = run_client().await;
    let elapsed = start.elapsed();

    let total_requests = N_CONNECTIONS * N_REQUESTS;
    let success_rate = (successful as f64 / total_requests as f64) * 100.0;
    let rps = successful as f64 / elapsed.as_secs_f64();

    println!("HTTP Echo Server");
    println!("Connections: {}", N_CONNECTIONS);
    println!("Requests per connection: {}", N_REQUESTS);
    println!("Successful requests: {}/{} ({:.1}%)", successful, total_requests, success_rate);
    println!("Requests/second: {:.0}", rps);
    println!("Total time: {:.3} s", elapsed.as_secs_f64());
}
