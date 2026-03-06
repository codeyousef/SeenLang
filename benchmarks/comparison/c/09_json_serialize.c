// JSON Serialization Benchmark
// Same algorithm as Seen: StringBuilder pattern with reuse, 1M objects
// Uses %.6f for floats to match Seen's fast_f64_to_buf (6 decimal places)
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <time.h>

typedef struct {
    char*   data;
    int64_t length;
    int64_t capacity;
} StringBuilder;

static void sb_init(StringBuilder* sb) {
    sb->capacity = 4096;
    sb->data = (char*)malloc((size_t)sb->capacity);
    sb->length = 0;
}

static void sb_clear(StringBuilder* sb) {
    sb->length = 0;
}

static void sb_ensure(StringBuilder* sb, int64_t extra) {
    if (sb->length + extra > sb->capacity) {
        while (sb->length + extra > sb->capacity) sb->capacity *= 2;
        sb->data = (char*)realloc(sb->data, (size_t)sb->capacity);
    }
}

static void sb_append(StringBuilder* sb, const char* s, int64_t len) {
    sb_ensure(sb, len);
    memcpy(sb->data + sb->length, s, (size_t)len);
    sb->length += len;
}

static void sb_append_str(StringBuilder* sb, const char* s) {
    sb_append(sb, s, (int64_t)strlen(s));
}

static void sb_append_int(StringBuilder* sb, int64_t n) {
    char buf[32];
    int len = snprintf(buf, sizeof(buf), "%ld", (long)n);
    sb_append(sb, buf, len);
}

static void sb_append_float(StringBuilder* sb, double f) {
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "%.6f", f);
    sb_append(sb, buf, len);
}

static void sb_free(StringBuilder* sb) {
    free(sb->data);
}

static void serialize_into(StringBuilder* sb, int64_t id, double value, int active, const char* tags_json) {
    sb_append_str(sb, "{\"id\":");
    sb_append_int(sb, id);
    sb_append_str(sb, ",\"name\":\"Object");
    sb_append_int(sb, id);
    sb_append_str(sb, "\",\"value\":");
    sb_append_float(sb, value);
    if (active) {
        sb_append_str(sb, ",\"active\":true,\"tags\":[");
    } else {
        sb_append_str(sb, ",\"active\":false,\"tags\":[");
    }
    sb_append_str(sb, tags_json);
    sb_append_str(sb, "]}");
}

static int64_t benchmark_json(int64_t n) {
    // Seen's string escaping drops \" at boundaries, producing this 32-char string
    const char* tags_json = "tag1\",\"tag2\",\"tag3\",\"tag4\",\"tag5";
    StringBuilder sb;
    sb_init(&sb);
    int64_t total_length = 0;

    for (int64_t i = 0; i < n; i++) {
        sb_clear(&sb);
        int active_val = 1 - (int)(i - (i / 2) * 2);
        double value = (double)i * 3.14159;
        serialize_into(&sb, i, value, active_val, tags_json);
        total_length += sb.length;
    }

    sb_free(&sb);
    return total_length;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
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
        double start = get_time_ms();
        int64_t total_length = benchmark_json(n);
        double elapsed = get_time_ms() - start;
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
