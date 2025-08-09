// Reactive Programming Benchmark - C++ Implementation
#include <iostream>
#include <vector>
#include <chrono>
#include <functional>
#include <memory>
#include <queue>

// Simple Observable implementation
template<typename T>
class Observable {
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
    
    // Map operator
    template<typename U>
    std::shared_ptr<Observable<U>> map(std::function<U(T)> mapper) {
        auto result = std::make_shared<Observable<U>>();
        subscribe([result, mapper](T value) {
            result->emit(mapper(value));
        });
        return result;
    }
    
    // Filter operator
    std::shared_ptr<Observable<T>> filter(std::function<bool(T)> predicate) {
        auto result = std::make_shared<Observable<T>>();
        subscribe([result, predicate](T value) {
            if (predicate(value)) {
                result->emit(value);
            }
        });
        return result;
    }
};

class ReactiveBenchmark {
public:
    static double testImperative(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            std::vector<int> data(dataSize);
            // Generate data
            for (int i = 0; i < dataSize; i++) {
                data[i] = i;
            }
            
            // Process imperatively
            std::vector<int> result;
            result.reserve(dataSize / 2);  // Reserve space for efficiency
            for (int val : data) {
                if (val % 2 == 0) {
                    result.push_back(val * 2);
                }
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    static double testSimpleReactive(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            Observable<int> source;
            std::vector<int> result;
            result.reserve(dataSize / 2);  // Reserve space like imperative version
            
            auto filtered = source.filter([](int val) { return val % 2 == 0; });
            auto mapped = filtered->map<int>([](int val) { return val * 2; });
            mapped->subscribe([&result](int val) { result.push_back(val); });
            
            // Emit data
            for (int i = 0; i < dataSize; i++) {
                source.emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    static double testComplexComposition(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            Observable<int> source;
            std::vector<int> result;
            
            auto chain = source
                .filter([](int val) { return val > 10; })
                ->map<int>([](int val) { return val * 3; })
                ->filter([](int val) { return val < 1000; })
                ->map<int>([](int val) { return val / 2; });
            
            chain->subscribe([&result](int val) { result.push_back(val); });
            
            for (int i = 0; i < dataSize; i++) {
                source.emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
    
    static double testBackpressure(int iterations, int dataSize) {
        auto start = std::chrono::high_resolution_clock::now();
        
        for (int iter = 0; iter < iterations; iter++) {
            Observable<int> source;
            std::queue<int> buffer;
            std::vector<int> result;
            const int bufferLimit = 100;
            
            source.subscribe([&buffer, &result, bufferLimit](int val) {
                if (buffer.size() < bufferLimit) {
                    buffer.push(val);
                }
                // Process buffered items
                while (!buffer.empty() && result.size() < bufferLimit) {
                    result.push_back(buffer.front() * 2);
                    buffer.pop();
                }
            });
            
            for (int i = 0; i < dataSize; i++) {
                source.emit(i);
            }
        }
        
        auto end = std::chrono::high_resolution_clock::now();
        return std::chrono::duration<double>(end - start).count();
    }
};

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 1000;
    int dataSize = argc > 2 ? std::atoi(argv[2]) : 1000;
    
    double imperativeTime = ReactiveBenchmark::testImperative(iterations, dataSize);
    double simpleReactiveTime = ReactiveBenchmark::testSimpleReactive(iterations, dataSize);
    double complexTime = ReactiveBenchmark::testComplexComposition(iterations, dataSize);
    double backpressureTime = ReactiveBenchmark::testBackpressure(iterations, dataSize);
    
    double overhead = ((simpleReactiveTime - imperativeTime) / imperativeTime) * 100;
    bool zeroCost = std::abs(overhead) < 5.0;  // Less than 5% overhead
    
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"reactive_zero_cost\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"data_size\": " << dataSize << ",\n";
    std::cout << "  \"results\": {\n";
    std::cout << "    \"imperative\": " << imperativeTime << ",\n";
    std::cout << "    \"simple_reactive\": " << simpleReactiveTime << ",\n";
    std::cout << "    \"complex_composition\": " << complexTime << ",\n";
    std::cout << "    \"backpressure\": " << backpressureTime << ",\n";
    std::cout << "    \"overhead_percent\": " << overhead << "\n";
    std::cout << "  },\n";
    std::cout << "  \"zero_cost\": " << (zeroCost ? "true" : "false") << "\n";
    std::cout << "}\n";
    
    return 0;
}