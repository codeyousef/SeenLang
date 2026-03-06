// HTTP Echo Request/Response String Building Benchmark
// N=5,000,000 requests, checksum = sum of response string lengths
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

static int64_t run_http_echo(int n) {
    const char* body = "{\"user\":\"test\",\"action\":\"ping\",\"timestamp\":1234567890}";
    // Response format:
    // "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: Seen/1.0\r\n\r\n{\"echo\":\"<body>\"}"
    // Pre-calculate: header part is fixed, body gets embedded
    char response[512];
    int64_t total_length = 0;

    for (int i = 0; i < n; i++) {
        int len = snprintf(response, sizeof(response),
            "HTTP/1.1 200 OK\r\n"
            "Content-Type: application/json\r\n"
            "Server: Seen/1.0\r\n"
            "\r\n"
            "{\"echo\":\"%s\"}",
            body);
        total_length += (int64_t)len;
    }

    return total_length;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int n = 5000000;

    printf("HTTP Echo Request/Response Benchmark\n");
    printf("N: %d requests\n", n);

    int warmup_n = n / 10;
    printf("Warming up (3 runs at n=%d)...\n", warmup_n);
    for (int w = 0; w < 3; w++) {
        (void)run_http_echo(warmup_n);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t checksum = run_http_echo(n);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = checksum;
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
