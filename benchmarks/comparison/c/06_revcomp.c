// Reverse Complement Benchmark
// 256-entry complement lookup table, LCG-generated DNA sequence
// N=5,000,000
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

static unsigned char complement_table[256];

static void init_complement_table(void) {
    // Initialize all entries to identity
    for (int i = 0; i < 256; i++) {
        complement_table[i] = (unsigned char)i;
    }

    // Uppercase complements
    complement_table['A'] = 'T';
    complement_table['T'] = 'A';
    complement_table['C'] = 'G';
    complement_table['G'] = 'C';
    complement_table['U'] = 'A';
    complement_table['M'] = 'K';
    complement_table['K'] = 'M';
    complement_table['R'] = 'Y';
    complement_table['Y'] = 'R';
    complement_table['W'] = 'W';
    complement_table['S'] = 'S';
    complement_table['V'] = 'B';
    complement_table['B'] = 'V';
    complement_table['H'] = 'D';
    complement_table['D'] = 'H';
    complement_table['N'] = 'N';

    // Lowercase complements
    complement_table['a'] = 't';
    complement_table['t'] = 'a';
    complement_table['c'] = 'g';
    complement_table['g'] = 'c';
    complement_table['u'] = 'a';
    complement_table['m'] = 'k';
    complement_table['k'] = 'm';
    complement_table['r'] = 'y';
    complement_table['y'] = 'r';
    complement_table['w'] = 'w';
    complement_table['s'] = 's';
    complement_table['v'] = 'b';
    complement_table['b'] = 'v';
    complement_table['h'] = 'd';
    complement_table['d'] = 'h';
    complement_table['n'] = 'n';
}

static void generate_sequence(unsigned char* seq, int n) {
    static const unsigned char bases[4] = {'A', 'C', 'G', 'T'};
    int64_t seed = 42;
    for (int i = 0; i < n; i++) {
        seed = (seed * 1103515245 + 12345) % 2147483647;
        if (seed < 0) seed = -seed;
        seq[i] = bases[seed % 4];
    }
}

static void reverse_complement(const unsigned char* seq, unsigned char* result, int n) {
    for (int i = 0; i < n; i++) {
        result[i] = complement_table[seq[n - 1 - i]];
    }
}

static int64_t compute_checksum(const unsigned char* result, int n) {
    int64_t sum = 0;
    for (int i = 0; i < n; i++) {
        sum += (int64_t)result[i];
    }
    return sum;
}

static int64_t run_revcomp(int n) {
    unsigned char* seq = (unsigned char*)malloc((size_t)n);
    unsigned char* result = (unsigned char*)malloc((size_t)n);

    generate_sequence(seq, n);
    reverse_complement(seq, result, n);

    int64_t checksum = compute_checksum(result, n);

    free(seq);
    free(result);
    return checksum;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
    int n = 5000000;

    printf("Reverse Complement Benchmark\n");
    printf("N: %d\n", n);

    init_complement_table();

    printf("Warming up (3 runs)...\n");
    for (int w = 0; w < 3; w++) {
        (void)run_revcomp(n);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        double start = get_time_ms();
        int64_t checksum = run_revcomp(n);
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
