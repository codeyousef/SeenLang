// JSON Parser Benchmark - C++ Implementation
#include <iostream>
#include <fstream>
#include <sstream>
#include <chrono>
#include <string>
#include <vector>
#include <unordered_map>
#include <memory>
#include <variant>

// Simple JSON value representation
class JSONValue;
using JSONObject = std::unordered_map<std::string, std::shared_ptr<JSONValue>>;
using JSONArray = std::vector<std::shared_ptr<JSONValue>>;
using JSONVariant = std::variant<std::nullptr_t, bool, double, std::string, JSONObject, JSONArray>;

class JSONValue {
public:
    JSONVariant value;
    
    JSONValue() : value(nullptr) {}
    JSONValue(bool b) : value(b) {}
    JSONValue(double d) : value(d) {}
    JSONValue(const std::string& s) : value(s) {}
    JSONValue(const JSONObject& o) : value(o) {}
    JSONValue(const JSONArray& a) : value(a) {}
};

class SimpleJSONParser {
    const std::string& input;
    size_t pos;
    
public:
    SimpleJSONParser(const std::string& json) : input(json), pos(0) {}
    
    std::shared_ptr<JSONValue> parse() {
        skipWhitespace();
        return parseValue();
    }
    
private:
    void skipWhitespace() {
        while (pos < input.length() && std::isspace(input[pos])) {
            pos++;
        }
    }
    
    std::shared_ptr<JSONValue> parseValue() {
        skipWhitespace();
        if (pos >= input.length()) return nullptr;
        
        char c = input[pos];
        if (c == '{') return parseObject();
        if (c == '[') return parseArray();
        if (c == '"') return parseString();
        if (c == 't' || c == 'f') return parseBool();
        if (c == 'n') return parseNull();
        if (c == '-' || std::isdigit(c)) return parseNumber();
        
        return nullptr;
    }
    
    std::shared_ptr<JSONValue> parseObject() {
        JSONObject obj;
        pos++; // skip '{'
        
        while (true) {
            skipWhitespace();
            if (pos >= input.length()) break;
            if (input[pos] == '}') {
                pos++;
                break;
            }
            
            // Parse key
            auto key = parseString();
            if (!key) break;
            std::string keyStr = std::get<std::string>(key->value);
            
            skipWhitespace();
            if (pos >= input.length() || input[pos] != ':') break;
            pos++; // skip ':'
            
            // Parse value
            auto value = parseValue();
            if (!value) break;
            obj[keyStr] = value;
            
            skipWhitespace();
            if (pos >= input.length()) break;
            if (input[pos] == ',') {
                pos++;
                continue;
            }
            if (input[pos] == '}') {
                pos++;
                break;
            }
        }
        
        return std::make_shared<JSONValue>(obj);
    }
    
    std::shared_ptr<JSONValue> parseArray() {
        JSONArray arr;
        pos++; // skip '['
        
        while (true) {
            skipWhitespace();
            if (pos >= input.length()) break;
            if (input[pos] == ']') {
                pos++;
                break;
            }
            
            auto value = parseValue();
            if (!value) break;
            arr.push_back(value);
            
            skipWhitespace();
            if (pos >= input.length()) break;
            if (input[pos] == ',') {
                pos++;
                continue;
            }
            if (input[pos] == ']') {
                pos++;
                break;
            }
        }
        
        return std::make_shared<JSONValue>(arr);
    }
    
    std::shared_ptr<JSONValue> parseString() {
        if (input[pos] != '"') return nullptr;
        pos++; // skip '"'
        
        std::string str;
        while (pos < input.length() && input[pos] != '"') {
            if (input[pos] == '\\' && pos + 1 < input.length()) {
                pos++;
                // Simple escape handling
                switch (input[pos]) {
                    case 'n': str += '\n'; break;
                    case 't': str += '\t'; break;
                    case 'r': str += '\r'; break;
                    case '\\': str += '\\'; break;
                    case '"': str += '"'; break;
                    default: str += input[pos];
                }
            } else {
                str += input[pos];
            }
            pos++;
        }
        
        if (pos < input.length()) pos++; // skip closing '"'
        return std::make_shared<JSONValue>(str);
    }
    
    std::shared_ptr<JSONValue> parseNumber() {
        size_t start = pos;
        if (input[pos] == '-') pos++;
        
        while (pos < input.length() && std::isdigit(input[pos])) pos++;
        
        if (pos < input.length() && input[pos] == '.') {
            pos++;
            while (pos < input.length() && std::isdigit(input[pos])) pos++;
        }
        
        if (pos < input.length() && (input[pos] == 'e' || input[pos] == 'E')) {
            pos++;
            if (pos < input.length() && (input[pos] == '+' || input[pos] == '-')) pos++;
            while (pos < input.length() && std::isdigit(input[pos])) pos++;
        }
        
        double num = std::stod(input.substr(start, pos - start));
        return std::make_shared<JSONValue>(num);
    }
    
    std::shared_ptr<JSONValue> parseBool() {
        if (input.substr(pos, 4) == "true") {
            pos += 4;
            return std::make_shared<JSONValue>(true);
        } else if (input.substr(pos, 5) == "false") {
            pos += 5;
            return std::make_shared<JSONValue>(false);
        }
        return nullptr;
    }
    
    std::shared_ptr<JSONValue> parseNull() {
        if (input.substr(pos, 4) == "null") {
            pos += 4;
            return std::make_shared<JSONValue>();
        }
        return nullptr;
    }
};

std::string generateTestJSON(int depth, int arraySize) {
    std::stringstream json;
    json << "{";
    json << "\"name\": \"Test Object\",";
    json << "\"value\": 42.5,";
    json << "\"active\": true,";
    json << "\"items\": [";
    for (int i = 0; i < arraySize; i++) {
        if (i > 0) json << ",";
        json << "{\"id\": " << i << ", \"data\": \"item_" << i << "\"}";
    }
    json << "],";
    json << "\"nested\": {";
    json << "\"level\": " << depth << ",";
    json << "\"description\": \"Nested object for testing\"";
    json << "}";
    json << "}";
    return json.str();
}

int main(int argc, char* argv[]) {
    int iterations = argc > 1 ? std::atoi(argv[1]) : 30;
    std::string testFile = argc > 2 ? argv[2] : "";
    
    std::string jsonContent;
    
    if (!testFile.empty()) {
        // Read from file
        std::ifstream file(testFile);
        if (file.is_open()) {
            std::stringstream buffer;
            buffer << file.rdbuf();
            jsonContent = buffer.str();
        } else {
            // Generate test data if file not found
            jsonContent = generateTestJSON(3, 100);
        }
    } else {
        // Generate test data
        jsonContent = generateTestJSON(3, 100);
    }
    
    std::vector<double> times;
    
    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        
        SimpleJSONParser parser(jsonContent);
        auto result = parser.parse();
        
        auto end = std::chrono::high_resolution_clock::now();
        times.push_back(std::chrono::duration<double>(end - start).count());
    }
    
    double sum = 0;
    for (double t : times) sum += t;
    double mean = sum / times.size();
    
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"json_parser\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"json_size_bytes\": " << jsonContent.size() << ",\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        if (i > 0) std::cout << ", ";
        std::cout << times[i];
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << mean << ",\n";
    std::cout << "  \"throughput_mb_per_sec\": " << (jsonContent.size() / 1024.0 / 1024.0) / mean << "\n";
    std::cout << "}\n";
    
    return 0;
}