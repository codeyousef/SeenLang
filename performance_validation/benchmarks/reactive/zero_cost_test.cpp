// Zero-Cost Reactive Abstractions Test - C++ Implementation
#include <iostream>
#include <vector>
#include <functional>
#include <chrono>
#include <memory>
#include <algorithm>
#include <numeric>

// Simple Observable implementation
template<typename T>
class Observable {
private:
    std::vector<std::function<void(T)>> observers;
    
public:
    void subscribe(std::function<void(T)> observer) {
        observers.push_back(observer);
    }
    
    void emit(T value) {
        for (auto& observer : observers) {
            observer(value);
        }
    }
    
    template<typename U>
    std::shared_ptr<Observable<U>> map(std::function<U(T)> transform) {
        auto result = std::make_shared<Observable<U>>();
        this->subscribe([result, transform](T value) {
            result->emit(transform(value));
        });
        return result;
    }
    
    std::shared_ptr<Observable<T>> filter(std::function<bool(T)> predicate) {
        auto result = std::make_shared<Observable<T>>();
        this->subscribe([result, predicate](T value) {
            if (predicate(value)) {
                result->emit(value);
            }
        });
        return result;
    }
};

// Benchmark different reactive patterns
class ReactiveBenchmark {
public:
    // Test 1: Simple observable chain
    static double testSimpleChain(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            auto source = std::make_shared<Observable<int>>();
            std::vector<int> results;
            
            auto pipeline = source
                ->map<int>([](int x) { return x * 2; })
                ->filter([](int x) { return x % 4 == 0; })
                ->map<int>([](int x) { return x + 1; });
            
            pipeline->subscribe([&results](int value) {
                results.push_back(value);
            });
            
            for (int i = 0; i < dataSize; i++) {
                source->emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    // Test 2: Manual imperative equivalent
    static double testImperative(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            std::vector<int> results;
            
            for (int i = 0; i < dataSize; i++) {
                int value = i;
                value = value * 2;
                if (value % 4 == 0) {
                    value = value + 1;
                    results.push_back(value);
                }
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    // Test 3: Complex observable composition
    static double testComplexComposition(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            auto source1 = std::make_shared<Observable<int>>();
            auto source2 = std::make_shared<Observable<int>>();
            std::vector<int> results;
            
            auto pipeline1 = source1
                ->map<int>([](int x) { return x * 3; })
                ->filter([](int x) { return x > 10; });
            
            auto pipeline2 = source2
                ->map<int>([](int x) { return x * 5; })
                ->filter([](int x) { return x < 100; });
            
            pipeline1->subscribe([&results](int value) {
                results.push_back(value);
            });
            
            pipeline2->subscribe([&results](int value) {
                results.push_back(value);
            });
            
            for (int i = 0; i < dataSize; i++) {
                source1->emit(i);
                if (i % 2 == 0) source2->emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    // Test 4: Backpressure handling
    static double testBackpressure(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            auto source = std::make_shared<Observable<int>>();
            std::vector<int> buffer;
            const int BUFFER_SIZE = 100;
            
            source->subscribe([&buffer, BUFFER_SIZE](int value) {
                if (buffer.size() < BUFFER_SIZE) {
                    buffer.push_back(value);
                } else {
                    // Drop or handle backpressure
                    buffer.erase(buffer.begin());
                    buffer.push_back(value);
                }
            });
            
            for (int i = 0; i < dataSize; i++) {
                source->emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
};

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 1000;
    int dataSize = argc > 2 ? std::atoi(argv[2]) : 1000;
    
    // Output to stderr for debugging, not stdout
    std::cerr << "Testing reactive abstractions (C++)...\n";
    std::cerr << "Iterations: " << iterations << ", Data size: " << dataSize << "\n\n";
    
    // Run benchmarks
    double reactiveTime = ReactiveBenchmark::testSimpleChain(iterations, dataSize);
    double imperativeTime = ReactiveBenchmark::testImperative(iterations, dataSize);
    double complexTime = ReactiveBenchmark::testComplexComposition(iterations, dataSize);
    double backpressureTime = ReactiveBenchmark::testBackpressure(iterations, dataSize);
    
    // Calculate overhead
    double overhead = (reactiveTime / imperativeTime - 1) * 100;
    
    // Output JSON results
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"reactive_zero_cost\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"data_size\": " << dataSize << ",\n";
    std::cout << "  \"results\": {\n";
    std::cout << "    \"simple_reactive\": " << reactiveTime << ",\n";
    std::cout << "    \"imperative\": " << imperativeTime << ",\n";
    std::cout << "    \"complex_composition\": " << complexTime << ",\n";
    std::cout << "    \"backpressure\": " << backpressureTime << ",\n";
    std::cout << "    \"overhead_percent\": " << overhead << "\n";
    std::cout << "  },\n";
    std::cout << "  \"zero_cost\": " << (std::abs(overhead) < 5 ? "true" : "false") << "\n";
    std::cout << "}\n";
    
    return 0;
}