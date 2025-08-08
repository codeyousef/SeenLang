// C++ JSON parser benchmark implementation for fair comparison with Seen
// High-performance JSON parser using similar architecture to Seen implementation

#include <iostream>
#include <string>
#include <vector>
#include <unordered_map>
#include <chrono>
#include <fstream>
#include <filesystem>
#include <sstream>
#include <memory>
#include <variant>
#include <iomanip>
#include <cctype>
#include <stdexcept>

// Forward declaration
class JsonValue;

using JsonObject = std::unordered_map<std::string, JsonValue>;
using JsonArray = std::vector<JsonValue>;

class JsonValue {
public:
    using Value = std::variant<
        std::nullptr_t,
        bool,
        double,
        std::string,
        JsonArray,
        JsonObject
    >;

    Value value;

    JsonValue() : value(nullptr) {}
    JsonValue(std::nullptr_t) : value(nullptr) {}
    JsonValue(bool b) : value(b) {}
    JsonValue(double d) : value(d) {}
    JsonValue(int i) : value(static_cast<double>(i)) {}
    JsonValue(const std::string& s) : value(s) {}
    JsonValue(std::string&& s) : value(std::move(s)) {}
    JsonValue(const JsonArray& arr) : value(arr) {}
    JsonValue(JsonArray&& arr) : value(std::move(arr)) {}
    JsonValue(const JsonObject& obj) : value(obj) {}
    JsonValue(JsonObject&& obj) : value(std::move(obj)) {}

    bool isNull() const { return std::holds_alternative<std::nullptr_t>(value); }
    bool isBool() const { return std::holds_alternative<bool>(value); }
    bool isNumber() const { return std::holds_alternative<double>(value); }
    bool isString() const { return std::holds_alternative<std::string>(value); }
    bool isArray() const { return std::holds_alternative<JsonArray>(value); }
    bool isObject() const { return std::holds_alternative<JsonObject>(value); }

    bool asBool() const { return std::get<bool>(value); }
    double asNumber() const { return std::get<double>(value); }
    const std::string& asString() const { return std::get<std::string>(value); }
    const JsonArray& asArray() const { return std::get<JsonArray>(value); }
    const JsonObject& asObject() const { return std::get<JsonObject>(value); }

    bool isValid() const {
        return std::visit([](const auto& v) -> bool {
            using T = std::decay_t<decltype(v)>;
            if constexpr (std::is_same_v<T, std::nullptr_t>) {
                return true;
            } else if constexpr (std::is_same_v<T, bool>) {
                return true;
            } else if constexpr (std::is_same_v<T, double>) {
                return std::isfinite(v);
            } else if constexpr (std::is_same_v<T, std::string>) {
                return !v.empty();
            } else if constexpr (std::is_same_v<T, JsonArray>) {
                return std::all_of(v.begin(), v.end(), [](const JsonValue& item) {
                    return item.isValid();
                });
            } else if constexpr (std::is_same_v<T, JsonObject>) {
                return std::all_of(v.begin(), v.end(), [](const auto& pair) {
                    return pair.second.isValid();
                });
            }
            return false;
        }, value);
    }

    size_t size() const {
        return std::visit([](const auto& v) -> size_t {
            using T = std::decay_t<decltype(v)>;
            if constexpr (std::is_same_v<T, std::nullptr_t> || 
                         std::is_same_v<T, bool> || 
                         std::is_same_v<T, double>) {
                return 1;
            } else if constexpr (std::is_same_v<T, std::string>) {
                return v.length();
            } else if constexpr (std::is_same_v<T, JsonArray>) {
                size_t total = 0;
                for (const auto& item : v) {
                    total += item.size();
                }
                return total;
            } else if constexpr (std::is_same_v<T, JsonObject>) {
                size_t total = v.size(); // Count keys
                for (const auto& pair : v) {
                    total += pair.second.size();
                }
                return total;
            }
            return 0;
        }, value);
    }
};

class JsonParser {
private:
    std::string input;
    size_t position;
    size_t line;
    size_t column;

    bool isAtEnd() const {
        return position >= input.length();
    }

    char currentChar() const {
        if (isAtEnd()) return '\0';
        return input[position];
    }

    char peekChar() const {
        if (position + 1 >= input.length()) return '\0';
        return input[position + 1];
    }

    char advance() {
        if (isAtEnd()) return '\0';
        
        char ch = input[position++];
        
        if (ch == '\n') {
            line++;
            column = 1;
        } else {
            column++;
        }
        
        return ch;
    }

    void skipWhitespace() {
        while (!isAtEnd() && std::isspace(currentChar())) {
            advance();
        }
    }

    std::string parseUnicodeEscape() {
        std::string hex;
        
        for (int i = 0; i < 4; ++i) {
            if (isAtEnd() || !std::isxdigit(currentChar())) {
                throw std::runtime_error("Invalid unicode escape sequence at line " + 
                                       std::to_string(line) + ", column " + std::to_string(column));
            }
            hex += advance();
        }
        
        unsigned int codePoint = std::stoul(hex, nullptr, 16);
        
        if (codePoint <= 0x7F) {
            return std::string(1, static_cast<char>(codePoint));
        } else if (codePoint <= 0x7FF) {
            std::string result;
            result += static_cast<char>(0xC0 | (codePoint >> 6));
            result += static_cast<char>(0x80 | (codePoint & 0x3F));
            return result;
        } else if (codePoint <= 0xFFFF) {
            std::string result;
            result += static_cast<char>(0xE0 | (codePoint >> 12));
            result += static_cast<char>(0x80 | ((codePoint >> 6) & 0x3F));
            result += static_cast<char>(0x80 | (codePoint & 0x3F));
            return result;
        } else {
            // For simplicity, we'll use replacement character for higher code points
            return "\uFFFD";
        }
    }

    JsonValue parseString() {
        advance(); // consume opening quote
        
        std::string value;
        
        while (!isAtEnd() && currentChar() != '"') {
            char ch = currentChar();
            
            if (ch == '\\') {
                advance(); // consume backslash
                if (isAtEnd()) {
                    throw std::runtime_error("Unexpected end of input in string");
                }
                
                char escaped = currentChar();
                switch (escaped) {
                    case '"': value += '"'; break;
                    case '\\': value += '\\'; break;
                    case '/': value += '/'; break;
                    case 'b': value += '\b'; break;
                    case 'f': value += '\f'; break;
                    case 'n': value += '\n'; break;
                    case 'r': value += '\r'; break;
                    case 't': value += '\t'; break;
                    case 'u': {
                        advance(); // consume 'u'
                        value += parseUnicodeEscape();
                        continue; // Don't advance again
                    }
                    default:
                        throw std::runtime_error("Invalid escape sequence '\\" + 
                                               std::string(1, escaped) + "' at line " + 
                                               std::to_string(line) + ", column " + std::to_string(column));
                }
                advance();
            } else {
                value += ch;
                advance();
            }
        }
        
        if (isAtEnd() || currentChar() != '"') {
            throw std::runtime_error("Unterminated string");
        }
        
        advance(); // consume closing quote
        return JsonValue(std::move(value));
    }

    JsonValue parseNumber() {
        std::string number;
        
        // Handle negative sign
        if (currentChar() == '-') {
            number += advance();
        }
        
        // Parse integer part
        if (currentChar() == '0') {
            number += advance();
        } else if (std::isdigit(currentChar())) {
            while (!isAtEnd() && std::isdigit(currentChar())) {
                number += advance();
            }
        } else {
            throw std::runtime_error("Invalid number: missing digits");
        }
        
        // Parse decimal part
        if (!isAtEnd() && currentChar() == '.') {
            number += advance();
            
            if (isAtEnd() || !std::isdigit(currentChar())) {
                throw std::runtime_error("Invalid number: missing digits after decimal point");
            }
            
            while (!isAtEnd() && std::isdigit(currentChar())) {
                number += advance();
            }
        }
        
        // Parse exponent part
        if (!isAtEnd() && (currentChar() == 'e' || currentChar() == 'E')) {
            number += advance();
            
            if (!isAtEnd() && (currentChar() == '+' || currentChar() == '-')) {
                number += advance();
            }
            
            if (isAtEnd() || !std::isdigit(currentChar())) {
                throw std::runtime_error("Invalid number: missing digits in exponent");
            }
            
            while (!isAtEnd() && std::isdigit(currentChar())) {
                number += advance();
            }
        }
        
        try {
            double value = std::stod(number);
            return JsonValue(value);
        } catch (const std::exception&) {
            throw std::runtime_error("Invalid number format: '" + number + "'");
        }
    }

    bool matchKeyword(const std::string& keyword) {
        if (input.substr(position, keyword.length()) == keyword) {
            // Check that the keyword is not part of a larger identifier
            size_t nextPos = position + keyword.length();
            if (nextPos >= input.length() || !std::isalnum(input[nextPos])) {
                for (size_t i = 0; i < keyword.length(); ++i) {
                    advance();
                }
                return true;
            }
        }
        return false;
    }

    JsonValue parseBoolean() {
        if (matchKeyword("true")) {
            return JsonValue(true);
        } else if (matchKeyword("false")) {
            return JsonValue(false);
        } else {
            throw std::runtime_error("Invalid boolean at line " + 
                                   std::to_string(line) + ", column " + std::to_string(column));
        }
    }

    JsonValue parseNull() {
        if (matchKeyword("null")) {
            return JsonValue(nullptr);
        } else {
            throw std::runtime_error("Invalid null at line " + 
                                   std::to_string(line) + ", column " + std::to_string(column));
        }
    }

    JsonValue parseArray() {
        advance(); // consume '['
        skipWhitespace();
        
        JsonArray elements;
        
        // Handle empty array
        if (!isAtEnd() && currentChar() == ']') {
            advance();
            return JsonValue(std::move(elements));
        }
        
        while (true) {
            elements.push_back(parseValue());
            
            skipWhitespace();
            
            if (isAtEnd()) {
                throw std::runtime_error("Unexpected end of input in array");
            }
            
            char ch = currentChar();
            if (ch == ',') {
                advance();
                skipWhitespace();
            } else if (ch == ']') {
                advance();
                break;
            } else {
                throw std::runtime_error("Expected ',' or ']' but found '" + 
                                       std::string(1, ch) + "' at line " + 
                                       std::to_string(line) + ", column " + std::to_string(column));
            }
        }
        
        return JsonValue(std::move(elements));
    }

    JsonValue parseObject() {
        advance(); // consume '{'
        skipWhitespace();
        
        JsonObject object;
        
        // Handle empty object
        if (!isAtEnd() && currentChar() == '}') {
            advance();
            return JsonValue(std::move(object));
        }
        
        while (true) {
            // Parse key (must be string)
            JsonValue keyValue = parseString();
            if (!keyValue.isString()) {
                throw std::runtime_error("Object key must be a string");
            }
            std::string key = keyValue.asString();
            
            skipWhitespace();
            
            // Expect colon
            if (isAtEnd() || currentChar() != ':') {
                throw std::runtime_error("Expected ':' after object key at line " + 
                                       std::to_string(line) + ", column " + std::to_string(column));
            }
            advance();
            
            skipWhitespace();
            
            // Parse value
            JsonValue value = parseValue();
            object[std::move(key)] = std::move(value);
            
            skipWhitespace();
            
            if (isAtEnd()) {
                throw std::runtime_error("Unexpected end of input in object");
            }
            
            char ch = currentChar();
            if (ch == ',') {
                advance();
                skipWhitespace();
            } else if (ch == '}') {
                advance();
                break;
            } else {
                throw std::runtime_error("Expected ',' or '}' but found '" + 
                                       std::string(1, ch) + "' at line " + 
                                       std::to_string(line) + ", column " + std::to_string(column));
            }
        }
        
        return JsonValue(std::move(object));
    }

    JsonValue parseValue() {
        skipWhitespace();
        
        if (isAtEnd()) {
            throw std::runtime_error("Unexpected end of input");
        }
        
        char ch = currentChar();
        
        switch (ch) {
            case '"':
                return parseString();
            case '[':
                return parseArray();
            case '{':
                return parseObject();
            case 't':
            case 'f':
                return parseBoolean();
            case 'n':
                return parseNull();
            default:
                if (ch == '-' || std::isdigit(ch)) {
                    return parseNumber();
                } else {
                    throw std::runtime_error("Unexpected character '" + 
                                           std::string(1, ch) + "' at line " + 
                                           std::to_string(line) + ", column " + std::to_string(column));
                }
        }
    }

public:
    JsonParser(const std::string& input) 
        : input(input), position(0), line(1), column(1) {}

    JsonValue parse() {
        JsonValue value = parseValue();
        skipWhitespace();
        
        if (!isAtEnd()) {
            throw std::runtime_error("Unexpected content after JSON value at line " + 
                                   std::to_string(line) + ", column " + std::to_string(column));
        }
        
        return value;
    }
};

// Utility functions
std::string readFile(const std::string& filename) {
    std::ifstream file(filename);
    if (!file.is_open()) {
        throw std::runtime_error("Cannot open file: " + filename);
    }
    
    std::stringstream buffer;
    buffer << file.rdbuf();
    return buffer.str();
}

std::string generateDeeplyNestedJson(size_t depth) {
    std::string json;
    
    for (size_t i = 0; i < depth; ++i) {
        json += "{\"nested\":";
    }
    
    json += "\"value\"";
    
    for (size_t i = 0; i < depth; ++i) {
        json += "}";
    }
    
    return json;
}

std::string generateWideJson(size_t count) {
    std::string json = "{";
    
    for (size_t i = 0; i < count; ++i) {
        if (i > 0) {
            json += ",";
        }
        json += "\"key" + std::to_string(i) + "\":" + std::to_string(i);
    }
    
    json += "}";
    return json;
}

void benchmarkJsonParserRealWorld() {
    std::vector<std::string> testFiles = {
        "../../test_data/json_files/twitter.json",
        "../../test_data/json_files/canada.json",
        "../../test_data/json_files/citm_catalog.json",
        "../../test_data/json_files/large.json"
    };
    
    uint64_t totalElements = 0;
    uint64_t totalBytes = 0;
    std::chrono::duration<double> totalTime{0};
    
    for (const auto& filePath : testFiles) {
        if (!std::filesystem::exists(filePath)) {
            std::cout << "Warning: Test file " << filePath << " not found, skipping..." << std::endl;
            continue;
        }
        
        try {
            std::string content = readFile(filePath);
            size_t fileSize = content.size();
            
            std::cout << "Testing C++ JSON parser performance on " << filePath 
                      << " (" << fileSize << " bytes)" << std::endl;
            
            // Run multiple iterations for statistical accuracy
            const int iterations = 10;
            size_t fileElements = 0;
            std::chrono::duration<double> fileTime{0};
            
            for (int i = 0; i < iterations; ++i) {
                JsonParser parser(content);
                
                auto startTime = std::chrono::high_resolution_clock::now();
                JsonValue result = parser.parse();
                auto endTime = std::chrono::high_resolution_clock::now();
                
                if (!result.isValid()) {
                    throw std::runtime_error("Invalid JSON result");
                }
                
                fileElements = result.size();
                fileTime += endTime - startTime;
            }
            
            auto avgTime = fileTime / iterations;
            double bytesPerSecond = avgTime.count() > 0.0 ? 
                static_cast<double>(fileSize) / avgTime.count() : 0.0;
            
            std::cout << "  Elements: " << fileElements 
                      << ", Avg Time: " << std::fixed << std::setprecision(6) << avgTime.count() << "s"
                      << ", Bytes/sec: " << std::fixed << std::setprecision(0) << bytesPerSecond << std::endl;
            
            totalElements += fileElements;
            totalBytes += fileSize;
            totalTime += avgTime;
            
        } catch (const std::exception& e) {
            std::cerr << "Error processing " << filePath << ": " << e.what() << std::endl;
        }
    }
    
    double overallBytesPerSec = totalTime.count() > 0.0 ? 
        static_cast<double>(totalBytes) / totalTime.count() : 0.0;
    
    std::cout << "\nC++ JSON Parser Overall Performance:" << std::endl;
    std::cout << "  Total elements: " << totalElements << std::endl;
    std::cout << "  Total bytes: " << totalBytes << std::endl;
    std::cout << "  Total time: " << std::fixed << std::setprecision(6) << totalTime.count() << "s" << std::endl;
    std::cout << "  Average bytes/second: " << std::fixed << std::setprecision(0) << overallBytesPerSec << std::endl;
    std::cout << "  Average MB/sec: " << std::fixed << std::setprecision(2) << overallBytesPerSec / (1024.0 * 1024.0) << std::endl;
}

void benchmarkJsonParserStressTest() {
    std::cout << "Running C++ JSON parser stress tests..." << std::endl;
    
    try {
        // Test deeply nested structures
        std::string deeplyNested = generateDeeplyNestedJson(1000);
        JsonParser parser1(deeplyNested);
        
        auto start = std::chrono::high_resolution_clock::now();
        JsonValue result1 = parser1.parse();
        auto elapsed = std::chrono::high_resolution_clock::now() - start;
        
        if (!result1.isValid()) {
            throw std::runtime_error("Invalid deeply nested JSON result");
        }
        
        std::cout << "  Deeply nested (1000 levels): " 
                  << std::chrono::duration_cast<std::chrono::microseconds>(elapsed).count() 
                  << "μs" << std::endl;
        
        // Test wide structures
        std::string wideStructure = generateWideJson(10000);
        JsonParser parser2(wideStructure);
        
        start = std::chrono::high_resolution_clock::now();
        JsonValue result2 = parser2.parse();
        elapsed = std::chrono::high_resolution_clock::now() - start;
        
        if (!result2.isValid()) {
            throw std::runtime_error("Invalid wide JSON result");
        }
        
        std::cout << "  Wide structure (10000 keys): " 
                  << std::chrono::duration_cast<std::chrono::microseconds>(elapsed).count() 
                  << "μs" << std::endl;
        
    } catch (const std::exception& e) {
        std::cerr << "Error in stress test: " << e.what() << std::endl;
    }
}

int main() {
    std::cout << "Running C++ JSON Parser Benchmarks..." << std::endl;
    
    try {
        benchmarkJsonParserRealWorld();
        benchmarkJsonParserStressTest();
        
        std::cout << "C++ JSON parser benchmarks completed successfully!" << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Error running C++ JSON parser benchmark: " << e.what() << std::endl;
        return 1;
    }
}