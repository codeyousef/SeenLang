// Memory Benchmark - C++ Implementation
#include <iostream>
#include <vector>
#include <chrono>
#include <memory>
#include <cstring>

class MemoryBenchmark {
public:
    static double testAllocations(int iterations, int size) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int i = 0; i < iterations; i++) {
            // Test various allocation patterns
            std::vector<int> vec;
            vec.reserve(size);
            for (int j = 0; j < size; j++) {
                vec.push_back(j);
            }
            
            // Dynamic allocation
            int* arr = new int[size];
            for (int j = 0; j < size; j++) {
                arr[j] = j * 2;
            }
            delete[] arr;
            
            // Smart pointers
            auto ptr = std::make_unique<int[]>(size);
            for (int j = 0; j < size; j++) {
                ptr[j] = j * 3;
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
};

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 30;
    int size = argc > 2 ? std::atoi(argv[2]) : 10000;
    
    std::vector<double> times;
    for (int i = 0; i < iterations; i++) {
        times.push_back(MemoryBenchmark::testAllocations(100, size));
    }
    
    double sum = 0;
    for (double t : times) sum += t;
    double mean = sum / times.size();
    
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"memory\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"allocations\": " << (size * 300) << ",\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        if (i > 0) std::cout << ", ";
        std::cout << times[i];
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << mean << ",\n";
    std::cout << "  \"allocations_per_second\": " << ((size * 300) / mean) << "\n";
    std::cout << "}\n";
    
    return 0;
}