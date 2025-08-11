// C++ Arithmetic Microbenchmark Implementation
// Equivalent to Seen's arithmetic operations for fair comparison

#include <iostream>
#include <vector>
#include <chrono>
#include <string>
#include <cstdint>

struct BenchmarkResult {
    std::string name;
    std::string language;
    int64_t execution_time_ns;
    int64_t memory_peak_bytes;
    double operations_per_second;
    bool success;
    std::string error_message;
};

class ArithmeticBenchmark {
private:
    uint32_t iterations;
    size_t data_size;

public:
    ArithmeticBenchmark(uint32_t iter, size_t size) 
        : iterations(iter), data_size(size) {}

    // 32-bit integer addition benchmark
    BenchmarkResult benchmark_i32_addition() {
        std::vector<int32_t> vec_a(data_size);
        std::vector<int32_t> vec_b(data_size);
        std::vector<int32_t> result_vec(data_size);

        // Initialize test data
        for (size_t i = 0; i < data_size; ++i) {
            vec_a[i] = static_cast<int32_t>(i);
            vec_b[i] = static_cast<int32_t>(i * 2);
            result_vec[i] = 0;
        }

        auto start = std::chrono::high_resolution_clock::now();

        for (uint32_t iter = 0; iter < iterations; ++iter) {
            for (size_t i = 0; i < data_size; ++i) {
                result_vec[i] = vec_a[i] + vec_b[i];
            }
        }

        auto end = std::chrono::high_resolution_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);

        int64_t total_operations = static_cast<int64_t>(iterations) * static_cast<int64_t>(data_size);
        double ops_per_second = static_cast<double>(total_operations) / (elapsed.count() / 1e9);

        return BenchmarkResult{
            "i32_addition",
            "cpp",
            elapsed.count(),
            static_cast<int64_t>(data_size * 3 * sizeof(int32_t)),
            ops_per_second,
            true,
            ""
        };
    }

    // 32-bit integer multiplication benchmark
    BenchmarkResult benchmark_i32_multiplication() {
        std::vector<int32_t> vec_a(data_size);
        std::vector<int32_t> vec_b(data_size);
        std::vector<int32_t> result_vec(data_size);

        for (size_t i = 0; i < data_size; ++i) {
            vec_a[i] = static_cast<int32_t>((i % 1000) + 1);
            vec_b[i] = static_cast<int32_t>((i % 500) + 1);
            result_vec[i] = 0;
        }

        auto start = std::chrono::high_resolution_clock::now();

        for (uint32_t iter = 0; iter < iterations; ++iter) {
            for (size_t i = 0; i < data_size; ++i) {
                result_vec[i] = vec_a[i] * vec_b[i];
            }
        }

        auto end = std::chrono::high_resolution_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);

        int64_t total_operations = static_cast<int64_t>(iterations) * static_cast<int64_t>(data_size);
        double ops_per_second = static_cast<double>(total_operations) / (elapsed.count() / 1e9);

        return BenchmarkResult{
            "i32_multiplication",
            "cpp",
            elapsed.count(),
            static_cast<int64_t>(data_size * 3 * sizeof(int32_t)),
            ops_per_second,
            true,
            ""
        };
    }

    // 64-bit floating-point operations benchmark
    BenchmarkResult benchmark_f64_operations() {
        std::vector<double> vec_a(data_size);
        std::vector<double> vec_b(data_size);
        std::vector<double> result_vec(data_size);

        for (size_t i = 0; i < data_size; ++i) {
            vec_a[i] = i * 0.001 + 0.001;
            vec_b[i] = i * 0.002 + 0.002;
            result_vec[i] = 0.0;
        }

        auto start = std::chrono::high_resolution_clock::now();

        for (uint32_t iter = 0; iter < iterations; ++iter) {
            for (size_t i = 0; i < data_size; ++i) {
                double intermediate = vec_a[i] + vec_b[i];
                double intermediate2 = intermediate * vec_a[i];
                result_vec[i] = intermediate2 / vec_b[i];
            }
        }

        auto end = std::chrono::high_resolution_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);

        int64_t total_operations = static_cast<int64_t>(iterations) * static_cast<int64_t>(data_size) * 3;
        double ops_per_second = static_cast<double>(total_operations) / (elapsed.count() / 1e9);

        return BenchmarkResult{
            "f64_mixed_operations",
            "cpp",
            elapsed.count(),
            static_cast<int64_t>(data_size * 3 * sizeof(double)),
            ops_per_second,
            true,
            ""
        };
    }

    // Bitwise operations benchmark
    BenchmarkResult benchmark_bitwise_operations() {
        std::vector<uint32_t> vec_a(data_size);
        std::vector<uint32_t> vec_b(data_size);
        std::vector<uint32_t> result_vec(data_size);

        for (size_t i = 0; i < data_size; ++i) {
            vec_a[i] = static_cast<uint32_t>(i);
            vec_b[i] = static_cast<uint32_t>(i) * 0x9E3779B9;
            result_vec[i] = 0;
        }

        auto start = std::chrono::high_resolution_clock::now();

        for (uint32_t iter = 0; iter < iterations; ++iter) {
            for (size_t i = 0; i < data_size; ++i) {
                uint32_t a = vec_a[i];
                uint32_t b = vec_b[i];
                uint32_t and_result = a & b;
                uint32_t or_result = and_result | a;
                uint32_t xor_result = or_result ^ b;
                result_vec[i] = xor_result;
            }
        }

        auto end = std::chrono::high_resolution_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);

        int64_t total_operations = static_cast<int64_t>(iterations) * static_cast<int64_t>(data_size) * 3;
        double ops_per_second = static_cast<double>(total_operations) / (elapsed.count() / 1e9);

        return BenchmarkResult{
            "bitwise_operations",
            "cpp",
            elapsed.count(),
            static_cast<int64_t>(data_size * 3 * sizeof(uint32_t)),
            ops_per_second,
            true,
            ""
        };
    }

    std::vector<BenchmarkResult> run_all() {
        return {
            benchmark_i32_addition(),
            benchmark_i32_multiplication(),
            benchmark_f64_operations(),
            benchmark_bitwise_operations()
        };
    }
};

int main() {
    ArithmeticBenchmark benchmark(1000, 100000);
    auto results = benchmark.run_all();

    for (const auto& result : results) {
        std::cout << result.name << ": " 
                  << result.operations_per_second << " ops/sec ("
                  << (result.execution_time_ns / 1000000.0) << "ms)" << std::endl;
    }

    return 0;
}