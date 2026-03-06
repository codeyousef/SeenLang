// Reverse Complement Benchmark
// Same algorithm as Seen: 256-entry complement table, LCG sequence generation
// N=5,000,000
#include <cstdio>
#include <cstdint>
#include <vector>
#include <chrono>

static std::vector<int> create_complement_table() {
    std::vector<int> table(256);
    for (int i = 0; i < 256; i++) {
        table[i] = i;
    }

    // Uppercase
    table[65] = 84;   // A -> T
    table[67] = 71;   // C -> G
    table[71] = 67;   // G -> C
    table[84] = 65;   // T -> A
    table[85] = 65;   // U -> A
    table[77] = 75;   // M -> K
    table[82] = 89;   // R -> Y
    table[87] = 87;   // W -> W
    table[83] = 83;   // S -> S
    table[89] = 82;   // Y -> R
    table[75] = 77;   // K -> M
    table[86] = 66;   // V -> B
    table[72] = 68;   // H -> D
    table[68] = 72;   // D -> H
    table[66] = 86;   // B -> V
    table[78] = 78;   // N -> N

    // Lowercase
    table[97] = 116;  // a -> t
    table[99] = 103;  // c -> g
    table[103] = 99;  // g -> c
    table[116] = 97;  // t -> a
    table[117] = 97;  // u -> a
    table[109] = 107; // m -> k
    table[114] = 121; // r -> y
    table[119] = 119; // w -> w
    table[115] = 115; // s -> s
    table[121] = 114; // y -> r
    table[107] = 109; // k -> m
    table[118] = 98;  // v -> b
    table[104] = 100; // h -> d
    table[100] = 104; // d -> h
    table[98] = 118;  // b -> v
    table[110] = 110; // n -> n

    return table;
}

static std::vector<int> generate_sequence(int64_t n, int64_t seed) {
    std::vector<int> seq((size_t)n);
    int bases[4] = {65, 67, 71, 84}; // A, C, G, T

    int64_t current_seed = seed;
    for (int64_t i = 0; i < n; i++) {
        current_seed = (current_seed * 1103515245 + 12345) % 2147483647;
        if (current_seed < 0) current_seed = -current_seed;
        int idx = (int)(current_seed % 4);
        seq[(size_t)i] = bases[idx];
    }

    return seq;
}

static std::vector<int> reverse_complement(const std::vector<int>& seq,
                                            const std::vector<int>& table) {
    int64_t len = (int64_t)seq.size();
    std::vector<int> result((size_t)len);
    for (int64_t i = 0; i < len; i++) {
        result[(size_t)i] = table[(size_t)seq[(size_t)(len - 1 - i)]];
    }
    return result;
}

static int64_t compute_checksum(const std::vector<int>& seq) {
    int64_t sum = 0;
    for (size_t i = 0; i < seq.size(); i++) {
        sum += seq[i];
    }
    return sum;
}

int main() {
    int64_t n = 5000000;

    printf("Reverse Complement Benchmark\n");
    printf("Sequence length: %ld\n", (long)n);

    auto table = create_complement_table();
    auto seq = generate_sequence(n, 42);

    int warmup_runs = 3;
    printf("Warming up (%d runs)...\n", warmup_runs);
    for (int w = 0; w < warmup_runs; w++) {
        auto result = reverse_complement(seq, table);
    }

    printf("Running measured iterations...\n");
    int iterations = 5;
    double min_time = 1e18;
    int64_t result_checksum = 0;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        auto result = reverse_complement(seq, table);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_checksum = compute_checksum(result);
        }
    }

    printf("Checksum: %ld\n", (long)result_checksum);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
