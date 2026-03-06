// FASTA DNA Sequence Generation Benchmark
// Same algorithm as Seen: LCG RNG, repeat_fasta, random_fasta with cumulative probs
// N=5,000,000
#include <cstdio>
#include <cstdint>
#include <vector>
#include <chrono>

struct Random {
    int64_t seed;
    Random(int64_t s) : seed(s) {}
    double next() {
        seed = (seed * 3877 + 29573) % 139968;
        return (double)seed / 139968.0;
    }
};

static void make_cumulative(std::vector<double>& probs) {
    double cumulative = 0.0;
    for (size_t i = 0; i < probs.size(); i++) {
        cumulative += probs[i];
        probs[i] = cumulative;
    }
}

static std::vector<int> repeat_fasta(const std::vector<int>& s, int64_t n) {
    std::vector<int> result((size_t)n);
    int64_t len = (int64_t)s.size();
    for (int64_t i = 0; i < n; i++) {
        result[(size_t)i] = s[(size_t)(i % len)];
    }
    return result;
}

static std::vector<int> random_fasta(const std::vector<int>& freq_chars,
                                      const std::vector<double>& freq_probs,
                                      int64_t n, Random& rng) {
    std::vector<int> result((size_t)n);
    int64_t num_freqs = (int64_t)freq_probs.size();
    for (int64_t i = 0; i < n; i++) {
        double r = rng.next();
        int64_t si = 0;
        while (si < num_freqs) {
            if (r < freq_probs[(size_t)si]) {
                result[(size_t)i] = freq_chars[(size_t)si];
                break;
            }
            si++;
        }
        if (si == num_freqs) {
            result[(size_t)i] = freq_chars[(size_t)(num_freqs - 1)];
        }
    }
    return result;
}

static int64_t compute_checksum(const std::vector<int>& data) {
    int64_t sum = 0;
    for (size_t i = 0; i < data.size(); i++) {
        sum += data[i];
    }
    return sum;
}

int main() {
    int64_t n = 5000000;

    printf("FASTA Generation Benchmark\n");
    printf("Generating %ld nucleotides\n", (long)n);

    const char* alu_str = "GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACTAAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAATCGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCGCCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";
    std::vector<int> alu;
    for (int i = 0; i < 287; i++) {
        alu.push_back((int)(unsigned char)alu_str[i]);
    }

    // HomoSapiens frequencies
    std::vector<int> hs_chars = {97, 99, 103, 116};
    std::vector<double> hs_probs = {0.3029549426680, 0.1979883004921, 0.1975473066391, 0.3015094502008};
    make_cumulative(hs_probs);

    // IUB frequencies
    std::vector<int> iub_chars = {97, 99, 103, 116, 66, 68, 72, 75, 77, 78, 82, 83, 86, 87, 89};
    std::vector<double> iub_probs = {0.27, 0.12, 0.12, 0.27, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02, 0.02};
    make_cumulative(iub_probs);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        Random rng_warmup(42);
        auto seq1 = repeat_fasta(alu, n * 2);
        auto seq2 = random_fasta(iub_chars, iub_probs, n * 3, rng_warmup);
        auto seq3 = random_fasta(hs_chars, hs_probs, n * 5, rng_warmup);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        Random rng(42);

        auto start = std::chrono::high_resolution_clock::now();
        auto seq1 = repeat_fasta(alu, n * 2);
        auto seq2 = random_fasta(iub_chars, iub_probs, n * 3, rng);
        auto seq3 = random_fasta(hs_chars, hs_probs, n * 5, rng);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();

        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = compute_checksum(seq1) + compute_checksum(seq2) + compute_checksum(seq3);
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
