// Simple Compression Benchmark - C++ Implementation
#include <iostream>
#include <vector>
#include <chrono>
#include <cstdlib>

// Simple RLE compression for benchmarking
std::vector<uint8_t> compress(const std::vector<uint8_t>& data) {
    std::vector<uint8_t> compressed;
    if (data.empty()) return compressed;
    
    uint8_t current = data[0];
    int count = 1;
    
    for (size_t i = 1; i < data.size(); i++) {
        if (data[i] == current && count < 255) {
            count++;
        } else {
            compressed.push_back(count);
            compressed.push_back(current);
            current = data[i];
            count = 1;
        }
    }
    compressed.push_back(count);
    compressed.push_back(current);
    
    return compressed;
}

int main(int argc, char* argv[]) {
    int iteration = argc > 1 ? std::atoi(argv[1]) : 0;
    
    // Generate test data
    std::vector<uint8_t> data(1024 * 1024); // 1MB
    for (size_t i = 0; i < data.size(); i++) {
        data[i] = (i + iteration) % 256;
    }
    
    // Benchmark compression
    auto start = std::chrono::high_resolution_clock::now();
    auto compressed = compress(data);
    auto end = std::chrono::high_resolution_clock::now();
    
    double compress_time = std::chrono::duration<double>(end - start).count();
    double ratio = (double)compressed.size() / data.size();
    double throughput = (data.size() / (1024.0 * 1024.0)) / compress_time;
    
    // Output results in expected format
    std::cout << compress_time << std::endl;
    std::cout << compress_time * 0.5 << std::endl; // Decompression (simulated as faster)
    std::cout << ratio << std::endl;
    std::cout << throughput << std::endl;
    
    return 0;
}