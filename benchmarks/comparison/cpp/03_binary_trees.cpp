// Binary Trees Benchmark
// Same flattened algorithm as Seen: stores child check values, not pointers
// min_depth=4, max_depth=20
#include <cstdio>
#include <cstdint>
#include <chrono>

struct TreeNode {
    int64_t item;
    int64_t left_val;
    int64_t right_val;
    bool has_left;
    bool has_right;
};

static TreeNode* tree_new_leaf(int64_t item) {
    return new TreeNode{item, 0, 0, false, false};
}

static TreeNode* tree_with_children(int64_t item, int64_t left_check, int64_t right_check) {
    return new TreeNode{item, left_check, right_check, true, true};
}

static int64_t tree_check(const TreeNode* n) {
    int64_t result = n->item;
    if (n->has_left) result += n->left_val;
    if (n->has_right) result -= n->right_val;
    return result;
}

static TreeNode* make_tree(int64_t depth) {
    if (depth == 0) return tree_new_leaf(0);
    TreeNode* left = make_tree(depth - 1);
    TreeNode* right = make_tree(depth - 1);
    int64_t lc = tree_check(left);
    int64_t rc = tree_check(right);
    delete left;
    delete right;
    return tree_with_children(0, lc, rc);
}

static int64_t run_binary_trees(int64_t min_depth, int64_t max_depth) {
    int64_t stretch_depth = max_depth + 1;
    TreeNode* stretch_tree = make_tree(stretch_depth);
    int64_t stretch_check = tree_check(stretch_tree);
    delete stretch_tree;

    TreeNode* long_lived_tree = make_tree(max_depth);
    int64_t total_check = stretch_check;

    for (int64_t depth = min_depth; depth <= max_depth; depth += 2) {
        int64_t iterations = 1LL << (max_depth - depth + min_depth);
        int64_t check = 0;
        for (int64_t i = 0; i < iterations; i++) {
            TreeNode* temp = make_tree(depth);
            check += tree_check(temp);
            delete temp;
        }
        total_check += check;
    }

    total_check += tree_check(long_lived_tree);
    delete long_lived_tree;
    return total_check;
}

int main() {
    int64_t min_depth = 4;
    int64_t max_depth = 20;

    printf("Binary Trees Benchmark\n");
    printf("Max depth: %ld\n", (long)max_depth);

    printf("Warming up (1 run at depth 16)...\n");
    (void)run_binary_trees(min_depth, 16);

    printf("Running measured iterations...\n");
    int iterations = 3;
    double min_time = 1e18;
    int64_t result_check = 0;

    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        int64_t check = run_binary_trees(min_depth, max_depth);
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double, std::milli>(end - start).count();
        if (elapsed < min_time) {
            min_time = elapsed;
            result_check = check;
        }
    }

    printf("Checksum: %ld\n", (long)result_check);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
