// Binary Trees Benchmark
// Same flattened algorithm as Seen: stores child check values, not pointers
// min_depth=4, max_depth=20
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>

typedef struct {
    int64_t item;
    int64_t left_val;
    int64_t right_val;
    int has_left;
    int has_right;
} TreeNode;

static TreeNode* tree_new_leaf(int64_t item) {
    TreeNode* n = (TreeNode*)malloc(sizeof(TreeNode));
    n->item = item;
    n->left_val = 0;
    n->right_val = 0;
    n->has_left = 0;
    n->has_right = 0;
    return n;
}

static TreeNode* tree_with_children(int64_t item, int64_t left_check, int64_t right_check) {
    TreeNode* n = (TreeNode*)malloc(sizeof(TreeNode));
    n->item = item;
    n->left_val = left_check;
    n->right_val = right_check;
    n->has_left = 1;
    n->has_right = 1;
    return n;
}

static int64_t tree_check(const TreeNode* n) {
    int64_t result = n->item;
    if (n->has_left) result += n->left_val;
    if (n->has_right) result -= n->right_val;
    return result;
}

static TreeNode* make_tree(int64_t depth) {
    if (depth == 0) {
        return tree_new_leaf(0);
    }
    TreeNode* left = make_tree(depth - 1);
    TreeNode* right = make_tree(depth - 1);
    int64_t left_check = tree_check(left);
    int64_t right_check = tree_check(right);
    free(left);
    free(right);
    return tree_with_children(0, left_check, right_check);
}

static int64_t run_binary_trees(int64_t min_depth, int64_t max_depth) {
    int64_t stretch_depth = max_depth + 1;
    TreeNode* stretch_tree = make_tree(stretch_depth);
    int64_t stretch_check = tree_check(stretch_tree);
    free(stretch_tree);

    TreeNode* long_lived_tree = make_tree(max_depth);
    int64_t total_check = stretch_check;

    for (int64_t depth = min_depth; depth <= max_depth; depth += 2) {
        int64_t iterations = 1LL << (max_depth - depth + min_depth);
        int64_t check = 0;
        for (int64_t i = 0; i < iterations; i++) {
            TreeNode* temp = make_tree(depth);
            check += tree_check(temp);
            free(temp);
        }
        total_check += check;
    }

    total_check += tree_check(long_lived_tree);
    free(long_lived_tree);
    return total_check;
}

static double get_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1000000.0;
}

int main(void) {
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
        double start = get_time_ms();
        int64_t check = run_binary_trees(min_depth, max_depth);
        double elapsed = get_time_ms() - start;
        if (elapsed < min_time) {
            min_time = elapsed;
            result_check = check;
        }
    }

    printf("Checksum: %ld\n", (long)result_check);
    printf("Min time: %.6f ms\n", min_time);
    return 0;
}
