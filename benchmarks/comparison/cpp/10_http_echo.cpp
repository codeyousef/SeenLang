// HTTP Echo Server Benchmark
// Same algorithm as Seen: string building for HTTP response, N=5,000,000
#include <cstdio>
#include <cstdint>
#include <string>
#include <chrono>

static std::string process_request(const std::string& body) {
    std::string response;
    response.reserve(256);
    response.append("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: Seen/1.0\r\n\r\n");
    response.append("{\"echo\":\"");
    response.append(body);
    response.append("\"}");
    return response;
}

static int64_t benchmark_http(int64_t n) {
    std::string body = "{\"user\":\"test\",\"action\":\"ping\",\"timestamp\":1234567890}";
    int64_t total_length = 0;

    for (int64_t i = 0; i < n; i++) {
        std::string response = process_request(body);
        total_length += (int64_t)response.size();
    }

    return total_length;
}

int main() {
    int64_t n = 5000000;

    printf("HTTP Echo Server Benchmark\n");
    printf("Requests to process: %ld\n", (long)n);

    int warmup_runs = 3;
    printf("Warming up (%d runs at n/10)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)benchmark_http(n / 10);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        int64_t total_length = benchmark_http(n);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = total_length;
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
