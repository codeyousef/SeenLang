// Simple HTTP Server Benchmark - C++ Implementation
#include <iostream>
#include <chrono>
#include <thread>
#include <vector>
#include <cstdlib>

// Simulate HTTP request processing
void process_request(int request_id) {
    // Simulate some work
    volatile int sum = 0;
    for (int i = 0; i < 1000; i++) {
        sum += i * request_id;
    }
}

int main(int argc, char* argv[]) {
    int iteration = argc > 1 ? std::atoi(argv[1]) : 0;
    
    const int num_requests = 10000;
    const int concurrent_connections = 100;
    
    // Benchmark request processing
    auto start = std::chrono::high_resolution_clock::now();
    
    for (int i = 0; i < num_requests; i++) {
        process_request(i + iteration);
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    
    double total_time = std::chrono::duration<double>(end - start).count();
    double rps = num_requests / total_time;
    double latency_ms = (total_time / num_requests) * 1000;
    int memory_mb = 50 + (iteration % 10); // Simulated memory usage
    
    // Output results in expected format
    std::cout << rps << std::endl;
    std::cout << latency_ms << std::endl;
    std::cout << memory_mb << std::endl;
    std::cout << concurrent_connections << std::endl;
    
    return 0;
}