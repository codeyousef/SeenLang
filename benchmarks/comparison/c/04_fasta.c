// FASTA DNA Sequence Generation Benchmark
// LCG RNG, ALU repeat, IUB/HomoSapiens random frequency tables
// N=5,000,000 nucleotides, initial seed=42
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

#define IM 139968
#define IA 3877
#define IC 29573

static int64_t g_seed;

static double lcg_random(void) {
    g_seed = (g_seed * IA + IC) % IM;
    return (double)g_seed / (double)IM;
}

static const char ALU[] =
    "GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAAC"
    "ATGGTGAAACCCCGTCTCTACTAAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAAT"
    "CGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCGCCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";

#define ALU_LEN 287

typedef struct {
    char c;
    double p;
} AminoAcid;

static AminoAcid iub[] = {
    {'a', 0.27}, {'c', 0.12}, {'g', 0.12}, {'t', 0.27},
    {'B', 0.02}, {'D', 0.02}, {'H', 0.02}, {'K', 0.02},
    {'M', 0.02}, {'N', 0.02}, {'R', 0.02}, {'S', 0.02},
    {'V', 0.02}, {'W', 0.02}, {'Y', 0.02}
};
#define IUB_LEN 15

static AminoAcid homo_sapiens[] = {
    {'a', 0.3029549426680}, {'c', 0.1979883004921},
    {'g', 0.1975473066391}, {'t', 0.3015094502008}
};
#define HS_LEN 4

static void make_cumulative(AminoAcid* table, int len) {
    double cp = 0.0;
    for (int i = 0; i < len; i++) {
        cp += table[i].p;
        table[i].p = cp;
    }
}

static char select_random(AminoAcid* table, int len) {
    double r = lcg_random();
    for (int i = 0; i < len; i++) {
        if (r < table[i].p) return table[i].c;
    }
    return table[len - 1].c;
}

static int64_t repeat_fasta(char* out, int n) {
    int64_t checksum = 0;
    for (int i = 0; i < n; i++) {
        char c = ALU[i % ALU_LEN];
        out[i] = c;
        checksum += (int64_t)(unsigned char)c;
    }
    return checksum;
}

static int64_t random_fasta(char* out, int n, AminoAcid* table, int table_len) {
    int64_t checksum = 0;
    for (int i = 0; i < n; i++) {
        char c = select_random(table, table_len);
        out[i] = c;
        checksum += (int64_t)(unsigned char)c;
    }
    return checksum;
}

static int64_t run_fasta(int n) {
    g_seed = 42;

    int repeat_n = n * 2;
    int iub_n = n * 3;
    int hs_n = n * 5;

    char* buf1 = (char*)malloc((size_t)repeat_n);
    char* buf2 = (char*)malloc((size_t)iub_n);
    char* buf3 = (char*)malloc((size_t)hs_n);

    make_cumulative(iub, IUB_LEN);
    make_cumulative(homo_sapiens, HS_LEN);

    int64_t checksum = 0;
    checksum += repeat_fasta(buf1, repeat_n);
    checksum += random_fasta(buf2, iub_n, iub, IUB_LEN);
    checksum += random_fasta(buf3, hs_n, homo_sapiens, HS_LEN);

    free(buf1);
    free(buf2);
    free(buf3);

    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int n = 5000000;

    printf("FASTA DNA Sequence Generation Benchmark\n");
    printf("N: %d nucleotides\n", n);

    // Reset cumulative tables for each run since make_cumulative modifies in-place
    // We need to reset the original probabilities before each run_fasta call
    // Actually, make_cumulative is called inside run_fasta, but the tables are global
    // and get modified. We need to reset them each time.

    // Store original probabilities
    double iub_orig[IUB_LEN] = {0.27, 0.12, 0.12, 0.27, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02};
    double hs_orig[HS_LEN] = {0.3029549426680, 0.1979883004921, 0.1975473066391, 0.3015094502008};

    printf("Warming up (3 runs)...\n");
    for (int w = 0; w < 3; w++) {
        // Reset tables
        for (int i = 0; i < IUB_LEN; i++) iub[i].p = iub_orig[i];
        for (int i = 0; i < HS_LEN; i++) homo_sapiens[i].p = hs_orig[i];
        (void)run_fasta(n);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        // Reset tables
        for (int j = 0; j < IUB_LEN; j++) iub[j].p = iub_orig[j];
        for (int j = 0; j < HS_LEN; j++) homo_sapiens[j].p = hs_orig[j];

        double start = get_time_ms();
        int64_t checksum = run_fasta(n);
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
