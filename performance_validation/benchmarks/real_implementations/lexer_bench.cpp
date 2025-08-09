// C++ Lexer Benchmark Implementation
#include <iostream>
#include <fstream>
#include <chrono>
#include <vector>
#include <string>
#include <cctype>
#include <unordered_set>

enum class TokenType {
    KEYWORD, IDENTIFIER, NUMBER, STRING, OPERATOR, PUNCTUATION, COMMENT, WHITESPACE, EOF_TOKEN
};

struct Token {
    TokenType type;
    std::string value;
    size_t line;
    size_t column;
};

class Lexer {
private:
    std::string source;
    size_t pos = 0;
    size_t line = 1;
    size_t column = 1;
    
    std::unordered_set<std::string> keywords = {
        "fun", "val", "var", "if", "else", "when", "for", "while",
        "class", "interface", "object", "return", "break", "continue",
        "true", "false", "null", "this", "super", "import", "package"
    };
    
    char current() {
        if (pos >= source.length()) return '\0';
        return source[pos];
    }
    
    char peek(int offset = 1) {
        size_t next_pos = pos + offset;
        if (next_pos >= source.length()) return '\0';
        return source[next_pos];
    }
    
    void advance() {
        if (pos < source.length()) {
            if (source[pos] == '\n') {
                line++;
                column = 1;
            } else {
                column++;
            }
            pos++;
        }
    }
    
    void skipWhitespace() {
        while (std::isspace(current())) {
            advance();
        }
    }
    
    Token scanIdentifier() {
        size_t start = pos;
        size_t start_col = column;
        
        while (std::isalnum(current()) || current() == '_') {
            advance();
        }
        
        std::string value = source.substr(start, pos - start);
        TokenType type = keywords.count(value) ? TokenType::KEYWORD : TokenType::IDENTIFIER;
        
        return {type, value, line, start_col};
    }
    
    Token scanNumber() {
        size_t start = pos;
        size_t start_col = column;
        
        while (std::isdigit(current())) {
            advance();
        }
        
        if (current() == '.' && std::isdigit(peek())) {
            advance(); // consume '.'
            while (std::isdigit(current())) {
                advance();
            }
        }
        
        return {TokenType::NUMBER, source.substr(start, pos - start), line, start_col};
    }
    
    Token scanString() {
        size_t start = pos;
        size_t start_col = column;
        char quote = current();
        advance(); // consume opening quote
        
        while (current() != quote && current() != '\0') {
            if (current() == '\\') {
                advance(); // skip escape char
                if (current() != '\0') advance(); // skip escaped char
            } else {
                advance();
            }
        }
        
        if (current() == quote) {
            advance(); // consume closing quote
        }
        
        return {TokenType::STRING, source.substr(start, pos - start), line, start_col};
    }
    
    Token scanComment() {
        size_t start = pos;
        size_t start_col = column;
        
        if (current() == '/' && peek() == '/') {
            // Single-line comment
            while (current() != '\n' && current() != '\0') {
                advance();
            }
        } else if (current() == '/' && peek() == '*') {
            // Multi-line comment
            advance(); advance(); // consume /*
            while (!(current() == '*' && peek() == '/') && current() != '\0') {
                advance();
            }
            if (current() == '*') {
                advance(); advance(); // consume */
            }
        }
        
        return {TokenType::COMMENT, source.substr(start, pos - start), line, start_col};
    }
    
public:
    Lexer(const std::string& src) : source(src) {}
    
    std::vector<Token> tokenize() {
        std::vector<Token> tokens;
        
        while (pos < source.length()) {
            skipWhitespace();
            
            if (pos >= source.length()) break;
            
            char ch = current();
            
            if (std::isalpha(ch) || ch == '_') {
                tokens.push_back(scanIdentifier());
            } else if (std::isdigit(ch)) {
                tokens.push_back(scanNumber());
            } else if (ch == '"' || ch == '\'') {
                tokens.push_back(scanString());
            } else if (ch == '/' && (peek() == '/' || peek() == '*')) {
                tokens.push_back(scanComment());
            } else {
                // Operators and punctuation
                size_t start_col = column;
                std::string op(1, ch);
                advance();
                
                // Check for multi-character operators
                if ((ch == '=' || ch == '!' || ch == '<' || ch == '>') && current() == '=') {
                    op += current();
                    advance();
                } else if ((ch == '&' && current() == '&') || (ch == '|' && current() == '|')) {
                    op += current();
                    advance();
                }
                
                tokens.push_back({TokenType::OPERATOR, op, line, start_col});
            }
        }
        
        tokens.push_back({TokenType::EOF_TOKEN, "", line, column});
        return tokens;
    }
};

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <input_file> [iterations]" << std::endl;
        return 1;
    }
    
    std::string filename = argv[1];
    int iterations = argc > 2 ? std::atoi(argv[2]) : 1;
    
    // Read file
    std::ifstream file(filename);
    if (!file) {
        std::cerr << "Error: Cannot open file " << filename << std::endl;
        return 1;
    }
    
    std::string source((std::istreambuf_iterator<char>(file)),
                       std::istreambuf_iterator<char>());
    file.close();
    
    // Warmup
    for (int i = 0; i < 5; i++) {
        Lexer warmup_lexer(source);
        auto tokens = warmup_lexer.tokenize();
    }
    
    // Benchmark
    std::vector<double> times;
    size_t total_tokens = 0;
    
    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        
        Lexer lexer(source);
        auto tokens = lexer.tokenize();
        
        auto end = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
        
        times.push_back(duration.count() / 1000000.0); // Convert to seconds
        total_tokens = tokens.size();
    }
    
    // Output results in JSON format
    std::cout << "{" << std::endl;
    std::cout << "  \"language\": \"cpp\"," << std::endl;
    std::cout << "  \"benchmark\": \"lexer\"," << std::endl;
    std::cout << "  \"iterations\": " << iterations << "," << std::endl;
    std::cout << "  \"tokens_processed\": " << total_tokens << "," << std::endl;
    std::cout << "  \"times\": [";
    
    for (size_t i = 0; i < times.size(); i++) {
        std::cout << times[i];
        if (i < times.size() - 1) std::cout << ", ";
    }
    
    std::cout << "]," << std::endl;
    
    // Calculate statistics
    double sum = 0;
    for (double t : times) sum += t;
    double avg = sum / times.size();
    
    double tokens_per_sec = total_tokens / avg;
    
    std::cout << "  \"average_time\": " << avg << "," << std::endl;
    std::cout << "  \"tokens_per_second\": " << tokens_per_sec << std::endl;
    std::cout << "}" << std::endl;
    
    return 0;
}