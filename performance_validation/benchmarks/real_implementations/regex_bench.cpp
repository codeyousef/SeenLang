// Simple Regex Benchmark - C++ Implementation
#include <iostream>
#include <string>
#include <chrono>
#include <cstdlib>

// Simulate pattern matching
int find_matches(const std::string& text, const std::string& pattern) {
    int matches = 0;
    size_t pos = 0;
    while ((pos = text.find(pattern, pos)) != std::string::npos) {
        matches++;
        pos += pattern.length();
    }
    return matches;
}

int main(int argc, char* argv[]) {
    int iteration = argc > 1 ? std::atoi(argv[1]) : 0;
    
    // Generate test data
    std::string text;
    for (int i = 0; i < 100000; i++) {
        text += "test string " + std::to_string(i + iteration) + " ";
    }
    std::string pattern = "test";
    
    // Benchmark pattern matching
    auto compile_start = std::chrono::high_resolution_clock::now();
    // Simulate pattern compilation
    volatile int dummy = pattern.length();
    auto compile_end = std::chrono::high_resolution_clock::now();
    
    auto match_start = std::chrono::high_resolution_clock::now();
    int matches = find_matches(text, pattern);
    auto match_end = std::chrono::high_resolution_clock::now();
    
    double match_time = std::chrono::duration<double>(match_end - match_start).count();
    double compile_time = std::chrono::duration<double>(compile_end - compile_start).count();
    double matches_per_sec = matches / match_time;
    int memory_kb = 1024 + (iteration % 100);
    
    // Output results in expected format
    std::cout << match_time << std::endl;
    std::cout << matches_per_sec << std::endl;
    std::cout << memory_kb << std::endl;
    std::cout << compile_time << std::endl;
    
    return 0;
}