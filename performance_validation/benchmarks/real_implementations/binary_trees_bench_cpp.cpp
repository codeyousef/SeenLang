#include <iostream>
#include <chrono>
#include <memory>

struct Node {
    int val;
    std::unique_ptr<Node> left, right;
    Node(int v) : val(v) {}
};

std::unique_ptr<Node> createTree(int depth) {
    if (depth == 0) return nullptr;
    auto node = std::make_unique<Node>(depth);
    node->left = createTree(depth - 1);
    node->right = createTree(depth - 1);
    return node;
}

int checkTree(const std::unique_ptr<Node>& node) {
    if (!node) return 0;
    return node->val + checkTree(node->left) + checkTree(node->right);
}

int main() {
    auto start = std::chrono::high_resolution_clock::now();
    
    int depth = 10;
    auto tree = createTree(depth);
    int checksum = checkTree(tree);
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration<double, std::milli>(end - start);
    
    double allocations_per_sec = (1 << (depth + 1)) / (duration.count() / 1000.0);
    double memory_mb = (1 << (depth + 1)) * sizeof(Node) / (1024.0 * 1024.0);
    
    // Output: creation_time_ms allocations_per_sec memory_mb checksum
    std::cout << duration.count() << " " << allocations_per_sec << " " << memory_mb << " " << checksum << std::endl;
    
    return 0;
}