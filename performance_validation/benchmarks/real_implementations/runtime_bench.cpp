// Runtime Benchmark - C++ Implementation
#include <iostream>
#include <vector>
#include <chrono>
#include <cmath>

class RuntimeBenchmark {
public:
    static double fibonacci(int n) {
        if (n <= 1) return n;
        return fibonacci(n - 1) + fibonacci(n - 2);
    }
    
    static double testRuntime(int iterations) {
        auto start = std::chrono::high_resolution_clock::now();
        
        double result = 0;
        for (int i = 0; i < iterations; i++) {
            // Various runtime operations
            result += fibonacci(20);
            
            // Math operations
            for (int j = 0; j < 1000; j++) {
                result += std::sin(j) * std::cos(j);
            }
            
            // String operations
            std::string str = "Hello";
            for (int j = 0; j < 100; j++) {
                str += " World";
            }
            
            // Array operations
            std::vector<int> vec(1000);
            for (int j = 0; j < 1000; j++) {
                vec[j] = j * j;
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
};

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 30;
    
    std::vector<double> times;
    for (int i = 0; i < iterations; i++) {
        times.push_back(RuntimeBenchmark::testRuntime(10));
    }
    
    double sum = 0;
    for (double t : times) sum += t;
    double mean = sum / times.size();
    
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"runtime\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"operations\": 67650,\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        if (i > 0) std::cout << ", ";
        std::cout << times[i];
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << mean << ",\n";
    std::cout << "  \"ops_per_second\": " << (67650 / mean) << "\n";
    std::cout << "}\n";
    
    return 0;
}