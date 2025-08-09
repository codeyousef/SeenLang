// JSON Parser Benchmark - C++ Implementation
#include <iostream>
#include <string>
#include <vector>
#include <map>
#include <memory>
#include <chrono>
#include <fstream>
#include <sstream>
#include <variant>
#include <stdexcept>

class JsonValue {
public:
    using Object = std::map<std::string, std::shared_ptr<JsonValue>>;
    using Array = std::vector<std::shared_ptr<JsonValue>>;
    using Value = std::variant<std::nullptr_t, bool, double, std::string, Object, Array>;
    
    Value value;
    
    JsonValue() : value(nullptr) {}
    JsonValue(bool b) : value(b) {}
    JsonValue(double d) : value(d) {}
    JsonValue(const std::string& s) : value(s) {}
    JsonValue(const Object& o) : value(o) {}
    JsonValue(const Array& a) : value(a) {}
};

class JsonParser {
private:
    std::string input;
    size_t pos = 0;
    
    void skipWhitespace() {
        while (pos < input.length() && std::isspace(input[pos])) {
            pos++;
        }
    }
    
    char peek() {
        if (pos >= input.length()) return '\0';
        return input[pos];
    }
    
    char consume() {
        if (pos >= input.length()) throw std::runtime_error("Unexpected end of input");
        return input[pos++];
    }
    
    std::string parseString() {
        consume(); // Skip opening quote
        std::string result;
        
        while (pos < input.length()) {
            char c = consume();
            if (c == '"') {
                return result;
            } else if (c == '\\') {
                char next = consume();
                switch (next) {
                    case '"': result += '"'; break;
                    case '\\': result += '\\'; break;
                    case '/': result += '/'; break;
                    case 'b': result += '\b'; break;
                    case 'f': result += '\f'; break;
                    case 'n': result += '\n'; break;
                    case 'r': result += '\r'; break;
                    case 't': result += '\t'; break;
                    case 'u': {
                        // Unicode escape
                        std::string hex;
                        for (int i = 0; i < 4; i++) {
                            hex += consume();
                        }
                        int codepoint = std::stoi(hex, nullptr, 16);
                        if (codepoint < 128) {
                            result += static_cast<char>(codepoint);
                        } else {
                            // Simplified - just add placeholder
                            result += '?';
                        }
                        break;
                    }
                    default:
                        result += next;
                }
            } else {
                result += c;
            }
        }
        
        throw std::runtime_error("Unterminated string");
    }
    
    double parseNumber() {
        size_t start = pos;
        
        if (peek() == '-') consume();
        
        // Integer part
        if (peek() == '0') {
            consume();
        } else {
            while (std::isdigit(peek())) {
                consume();
            }
        }
        
        // Fractional part
        if (peek() == '.') {
            consume();
            while (std::isdigit(peek())) {
                consume();
            }
        }
        
        // Exponent part
        if (peek() == 'e' || peek() == 'E') {
            consume();
            if (peek() == '+' || peek() == '-') {
                consume();
            }
            while (std::isdigit(peek())) {
                consume();
            }
        }
        
        std::string numStr = input.substr(start, pos - start);
        return std::stod(numStr);
    }
    
    std::shared_ptr<JsonValue> parseValue();
    
    JsonValue::Array parseArray() {
        consume(); // Skip '['
        JsonValue::Array arr;
        
        skipWhitespace();
        if (peek() == ']') {
            consume();
            return arr;
        }
        
        while (true) {
            skipWhitespace();
            arr.push_back(parseValue());
            skipWhitespace();
            
            if (peek() == ',') {
                consume();
            } else if (peek() == ']') {
                consume();
                break;
            } else {
                throw std::runtime_error("Expected ',' or ']' in array");
            }
        }
        
        return arr;
    }
    
    JsonValue::Object parseObject() {
        consume(); // Skip '{'
        JsonValue::Object obj;
        
        skipWhitespace();
        if (peek() == '}') {
            consume();
            return obj;
        }
        
        while (true) {
            skipWhitespace();
            
            if (peek() != '"') {
                throw std::runtime_error("Expected string key in object");
            }
            
            std::string key = parseString();
            skipWhitespace();
            
            if (consume() != ':') {
                throw std::runtime_error("Expected ':' after object key");
            }
            
            skipWhitespace();
            obj[key] = parseValue();
            skipWhitespace();
            
            if (peek() == ',') {
                consume();
            } else if (peek() == '}') {
                consume();
                break;
            } else {
                throw std::runtime_error("Expected ',' or '}' in object");
            }
        }
        
        return obj;
    }
    
    std::shared_ptr<JsonValue> parseValue() {
        skipWhitespace();
        
        char c = peek();
        
        if (c == '"') {
            return std::make_shared<JsonValue>(parseString());
        } else if (c == '{') {
            return std::make_shared<JsonValue>(parseObject());
        } else if (c == '[') {
            return std::make_shared<JsonValue>(parseArray());
        } else if (c == 't') {
            if (input.substr(pos, 4) == "true") {
                pos += 4;
                return std::make_shared<JsonValue>(true);
            }
        } else if (c == 'f') {
            if (input.substr(pos, 5) == "false") {
                pos += 5;
                return std::make_shared<JsonValue>(false);
            }
        } else if (c == 'n') {
            if (input.substr(pos, 4) == "null") {
                pos += 4;
                return std::make_shared<JsonValue>();
            }
        } else if (c == '-' || std::isdigit(c)) {
            return std::make_shared<JsonValue>(parseNumber());
        }
        
        throw std::runtime_error("Unexpected character in JSON");
    }
    
public:
    std::shared_ptr<JsonValue> parse(const std::string& json) {
        input = json;
        pos = 0;
        auto result = parseValue();
        skipWhitespace();
        if (pos < input.length()) {
            throw std::runtime_error("Unexpected characters after JSON value");
        }
        return result;
    }
};

// Generate test JSON
std::string generateTestJson(int depth = 3, int breadth = 4) {
    if (depth <= 0) {
        return "\"leaf\"";
    }
    
    std::stringstream ss;
    ss << "{";
    for (int i = 0; i < breadth; i++) {
        if (i > 0) ss << ",";
        ss << "\"field" << i << "\":";
        
        if (i % 3 == 0) {
            ss << "[";
            for (int j = 0; j < 3; j++) {
                if (j > 0) ss << ",";
                ss << generateTestJson(depth - 1, breadth);
            }
            ss << "]";
        } else if (i % 3 == 1) {
            ss << generateTestJson(depth - 1, breadth);
        } else {
            ss << i * 123.456;
        }
    }
    ss << "}";
    return ss.str();
}

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 100;
    
    // Generate complex test data
    std::string testJson = generateTestJson(5, 5);
    std::cout << "Test JSON size: " << testJson.length() << " bytes\n";
    
    // Warmup
    JsonParser warmupParser;
    for (int i = 0; i < 10; i++) {
        auto result = warmupParser.parse(testJson);
    }
    
    // Benchmark
    std::vector<double> times;
    
    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        
        JsonParser parser;
        auto result = parser.parse(testJson);
        
        auto end = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
        times.push_back(duration.count() / 1000000.0); // Convert to seconds
    }
    
    // Calculate statistics
    double sum = 0;
    for (double t : times) sum += t;
    double avg = sum / times.size();
    
    // Output results in JSON format
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"json_parser\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"json_size\": " << testJson.length() << ",\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        std::cout << times[i];
        if (i < times.size() - 1) std::cout << ", ";
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << avg << ",\n";
    std::cout << "  \"throughput_mb_per_sec\": " << (testJson.length() / avg / 1048576.0) << "\n";
    std::cout << "}\n";
    
    return 0;
}