// JSON Serialization Benchmark
// Same algorithm as Seen: string append pattern with reuse, 1M objects
// Uses %.6f for floats to match Seen's fast_f64_to_buf (6 decimal places)
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <string>
#include <chrono>

static void serialize_into(std::string& sb, int64_t id, double value, int active,
                           const char* tags_json) {
    char buf[64];
    sb.append("{\"id\":");
    int len = snprintf(buf, sizeof(buf), "%ld", (long)id);
    sb.append(buf, (size_t)len);
    sb.append(",\"name\":\"Object");
    sb.append(buf, (size_t)len); // same id
    sb.append("\",\"value\":");
    len = snprintf(buf, sizeof(buf), "%.6f", value);
    sb.append(buf, (size_t)len);
    if (active) {
        sb.append(",\"active\":true,\"tags\":[");
    } else {
        sb.append(",\"active\":false,\"tags\":[");
    }
    sb.append(tags_json);
    sb.append("]}");
}

static int64_t benchmark_json(int64_t n) {
    // Seen's string escaping drops \" at boundaries, producing this 32-char string
    const char* tags_json = "tag1\",\"tag2\",\"tag3\",\"tag4\",\"tag5";
    std::string sb;
    sb.reserve(4096);
    int64_t total_length = 0;

    for (int64_t i = 0; i < n; i++) {
        sb.clear();
        int active_val = 1 - (int)(i - (i / 2) * 2);
        double value = (double)i * 3.14159;
        serialize_into(sb, i, value, active_val, tags_json);
        total_length += (int64_t)sb.size();
    }

    return total_length;
}

int main() {
    int64_t n = 1000000;

    printf("JSON Serialization Benchmark\n");
    printf("Objects to serialize: %ld\n", (long)n);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        (void)benchmark_json(n / 10);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_length = 0;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        int64_t total_length = benchmark_json(n);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_length = total_length;
        }
    }

    printf("Total JSON length: %ld\n", (long)result_length);
    printf("Min time: %.6f ms\n", min_time);
    printf("Objects per second: %.6f thousand\n", (double)n / (min_time / 1000.0) / 1000.0);
    printf("Throughput: %.6f MB/s\n", (double)result_length / (min_time / 1000.0) / 1e6);
    return 0;
}
