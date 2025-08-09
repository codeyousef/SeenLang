#include <iostream>
#include <chrono>
#include <vector>
#include <cmath>

double A(int i, int j) {
    return 1.0 / ((i + j) * (i + j + 1) / 2 + i + 1);
}

void multiplyAtAv(const std::vector<double>& v, std::vector<double>& atAv, int n) {
    std::vector<double> u(n);
    
    // Multiply by A
    for (int i = 0; i < n; i++) {
        u[i] = 0;
        for (int j = 0; j < n; j++) {
            u[i] += A(i, j) * v[j];
        }
    }
    
    // Multiply by A transpose
    for (int i = 0; i < n; i++) {
        atAv[i] = 0;
        for (int j = 0; j < n; j++) {
            atAv[i] += A(j, i) * u[j];
        }
    }
}

int main() {
    auto start = std::chrono::high_resolution_clock::now();
    
    int n = 100;
    std::vector<double> u(n, 1.0);
    std::vector<double> v(n);
    
    for (int i = 0; i < 10; i++) {
        multiplyAtAv(u, v, n);
        multiplyAtAv(v, u, n);
    }
    
    double vBv = 0, vv = 0;
    for (int i = 0; i < n; i++) {
        vBv += u[i] * v[i];
        vv += v[i] * v[i];
    }
    
    double norm = std::sqrt(vBv / vv);
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration<double, std::milli>(end - start);
    
    double flops_per_sec = (n * n * 20.0) / (duration.count() / 1000.0);
    double memory_mb = (n * 2 * sizeof(double)) / (1024.0 * 1024.0);
    
    // Output: computation_time_ms flops_per_sec memory_mb spectral_norm
    std::cout << duration.count() << " " << flops_per_sec << " " << memory_mb << " " << norm << std::endl;
    
    return 0;
}