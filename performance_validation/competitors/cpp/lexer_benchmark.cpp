// C++ lexer benchmark implementation for fair comparison with Seen
// Uses a realistic tokenizer similar to what Seen's lexer would do

#include <iostream>
#include <string>
#include <vector>
#include <unordered_map>
#include <chrono>
#include <fstream>
#include <filesystem>
#include <sstream>
#include <cctype>
#include <iomanip>

enum class TokenType {
    // Keywords
    FUNC, LET, MUT, IF, ELSE, WHILE, FOR, LOOP, RETURN, BREAK, CONTINUE,
    STRUCT, ENUM, IMPL, TRAIT, PUB, PRIV, MOD, USE, IMPORT, EXPORT,
    MATCH, WHEN, TRY, CATCH, FINALLY, ASYNC, AWAIT, CONST, STATIC,
    TYPE, INTERFACE, CLASS, EXTENDS, IMPLEMENTS, ABSTRACT, OVERRIDE,
    VIRTUAL, FINAL,
    
    // Types
    I8, I16, I32, I64, U8, U16, U32, U64, F32, F64, BOOL, CHAR, STR,
    STRING, VEC, HASHMAP, HASHSET, OPTION, RESULT, BOX, RC, ARC,
    
    // Literals
    INTEGER_LITERAL,
    FLOAT_LITERAL,
    STRING_LITERAL,
    CHAR_LITERAL,
    BOOL_LITERAL,
    
    // Identifiers
    IDENTIFIER,
    
    // Operators
    PLUS, MINUS, STAR, SLASH, PERCENT, EQUAL, EQUAL_EQUAL, NOT_EQUAL,
    LESS, LESS_EQUAL, GREATER, GREATER_EQUAL, AND_AND, OR_OR, NOT,
    AND, OR, XOR, LEFT_SHIFT, RIGHT_SHIFT, PLUS_EQUAL, MINUS_EQUAL,
    STAR_EQUAL, SLASH_EQUAL, PERCENT_EQUAL, AND_EQUAL, OR_EQUAL,
    XOR_EQUAL, LEFT_SHIFT_EQUAL, RIGHT_SHIFT_EQUAL,
    
    // Punctuation
    LEFT_PAREN, RIGHT_PAREN, LEFT_BRACE, RIGHT_BRACE, LEFT_BRACKET,
    RIGHT_BRACKET, SEMICOLON, COMMA, DOT, ARROW, FAT_ARROW, COLON,
    DOUBLE_COLON, QUESTION, AT, DOLLAR, HASH,
    
    // Special
    NEWLINE,
    WHITESPACE,
    COMMENT,
    EOF_TOKEN,
    INVALID
};

struct Token {
    TokenType type;
    std::string lexeme;
    size_t line;
    size_t column;
    
    Token(TokenType t, const std::string& lex, size_t ln, size_t col)
        : type(t), lexeme(lex), line(ln), column(col) {}
};

class Lexer {
private:
    std::string input;
    size_t position;
    size_t line;
    size_t column;
    std::unordered_map<std::string, TokenType> keywords;
    
    void initKeywords() {
        // Keywords
        keywords["func"] = TokenType::FUNC;
        keywords["let"] = TokenType::LET;
        keywords["mut"] = TokenType::MUT;
        keywords["if"] = TokenType::IF;
        keywords["else"] = TokenType::ELSE;
        keywords["while"] = TokenType::WHILE;
        keywords["for"] = TokenType::FOR;
        keywords["loop"] = TokenType::LOOP;
        keywords["return"] = TokenType::RETURN;
        keywords["break"] = TokenType::BREAK;
        keywords["continue"] = TokenType::CONTINUE;
        keywords["struct"] = TokenType::STRUCT;
        keywords["enum"] = TokenType::ENUM;
        keywords["impl"] = TokenType::IMPL;
        keywords["trait"] = TokenType::TRAIT;
        keywords["pub"] = TokenType::PUB;
        keywords["priv"] = TokenType::PRIV;
        keywords["mod"] = TokenType::MOD;
        keywords["use"] = TokenType::USE;
        keywords["import"] = TokenType::IMPORT;
        keywords["export"] = TokenType::EXPORT;
        keywords["match"] = TokenType::MATCH;
        keywords["when"] = TokenType::WHEN;
        keywords["try"] = TokenType::TRY;
        keywords["catch"] = TokenType::CATCH;
        keywords["finally"] = TokenType::FINALLY;
        keywords["async"] = TokenType::ASYNC;
        keywords["await"] = TokenType::AWAIT;
        keywords["const"] = TokenType::CONST;
        keywords["static"] = TokenType::STATIC;
        keywords["type"] = TokenType::TYPE;
        keywords["interface"] = TokenType::INTERFACE;
        keywords["class"] = TokenType::CLASS;
        keywords["extends"] = TokenType::EXTENDS;
        keywords["implements"] = TokenType::IMPLEMENTS;
        keywords["abstract"] = TokenType::ABSTRACT;
        keywords["override"] = TokenType::OVERRIDE;
        keywords["virtual"] = TokenType::VIRTUAL;
        keywords["final"] = TokenType::FINAL;
        
        // Types
        keywords["i8"] = TokenType::I8;
        keywords["i16"] = TokenType::I16;
        keywords["i32"] = TokenType::I32;
        keywords["i64"] = TokenType::I64;
        keywords["u8"] = TokenType::U8;
        keywords["u16"] = TokenType::U16;
        keywords["u32"] = TokenType::U32;
        keywords["u64"] = TokenType::U64;
        keywords["f32"] = TokenType::F32;
        keywords["f64"] = TokenType::F64;
        keywords["bool"] = TokenType::BOOL;
        keywords["char"] = TokenType::CHAR;
        keywords["str"] = TokenType::STR;
        keywords["String"] = TokenType::STRING;
        keywords["Vec"] = TokenType::VEC;
        keywords["HashMap"] = TokenType::HASHMAP;
        keywords["HashSet"] = TokenType::HASHSET;
        keywords["Option"] = TokenType::OPTION;
        keywords["Result"] = TokenType::RESULT;
        keywords["Box"] = TokenType::BOX;
        keywords["Rc"] = TokenType::RC;
        keywords["Arc"] = TokenType::ARC;
        
        // Boolean literals
        keywords["true"] = TokenType::BOOL_LITERAL;
        keywords["false"] = TokenType::BOOL_LITERAL;
    }
    
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
        while (!isAtEnd()) {
            char ch = currentChar();
            if (ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n') {
                advance();
            } else {
                break;
            }
        }
    }
    
    Token scanLineComment(size_t startLine, size_t startColumn) {
        advance(); // consume second '/'
        
        std::string comment = "//";
        
        while (!isAtEnd() && currentChar() != '\n') {
            comment += advance();
        }
        
        return Token(TokenType::COMMENT, comment, startLine, startColumn);
    }
    
    Token scanBlockComment(size_t startLine, size_t startColumn) {
        advance(); // consume '*'
        
        std::string comment = "/*";
        int depth = 1;
        
        while (depth > 0 && !isAtEnd()) {
            char ch = advance();
            comment += ch;
            
            if (ch == '*' && currentChar() == '/') {
                comment += advance();
                depth--;
            } else if (ch == '/' && currentChar() == '*') {
                comment += advance();
                depth++;
            }
        }
        
        return Token(TokenType::COMMENT, comment, startLine, startColumn);
    }
    
    Token scanStringLiteral(size_t startLine, size_t startColumn) {
        std::string value;
        std::string lexeme = "\"";
        
        while (!isAtEnd() && currentChar() != '"') {
            char ch = currentChar();
            lexeme += ch;
            
            if (ch == '\\') {
                advance();
                if (!isAtEnd()) {
                    char escaped = currentChar();
                    lexeme += escaped;
                    
                    switch (escaped) {
                        case 'n': value += '\n'; break;
                        case 't': value += '\t'; break;
                        case 'r': value += '\r'; break;
                        case '\\': value += '\\'; break;
                        case '"': value += '"'; break;
                        case '\'': value += '\''; break;
                        default: value += escaped; break;
                    }
                    advance();
                }
            } else {
                value += ch;
                advance();
            }
        }
        
        if (!isAtEnd()) {
            lexeme += advance(); // closing quote
        }
        
        return Token(TokenType::STRING_LITERAL, lexeme, startLine, startColumn);
    }
    
    Token scanCharLiteral(size_t startLine, size_t startColumn) {
        std::string value;
        std::string lexeme = "'";
        
        if (!isAtEnd()) {
            char ch = currentChar();
            lexeme += ch;
            
            if (ch == '\\') {
                advance();
                if (!isAtEnd()) {
                    char escaped = currentChar();
                    lexeme += escaped;
                    
                    switch (escaped) {
                        case 'n': value += '\n'; break;
                        case 't': value += '\t'; break;
                        case 'r': value += '\r'; break;
                        case '\\': value += '\\'; break;
                        case '"': value += '"'; break;
                        case '\'': value += '\''; break;
                        default: value += escaped; break;
                    }
                    advance();
                }
            } else {
                value += ch;
                advance();
            }
        }
        
        if (!isAtEnd() && currentChar() == '\'') {
            lexeme += advance();
        }
        
        return Token(TokenType::CHAR_LITERAL, lexeme, startLine, startColumn);
    }
    
    Token scanNumber(char firstDigit, size_t startLine, size_t startColumn) {
        std::string lexeme(1, firstDigit);
        bool isFloat = false;
        
        while (!isAtEnd() && std::isdigit(currentChar())) {
            lexeme += advance();
        }
        
        if (!isAtEnd() && currentChar() == '.' && std::isdigit(peekChar())) {
            isFloat = true;
            lexeme += advance(); // consume '.'
            
            while (!isAtEnd() && std::isdigit(currentChar())) {
                lexeme += advance();
            }
        }
        
        TokenType type = isFloat ? TokenType::FLOAT_LITERAL : TokenType::INTEGER_LITERAL;
        return Token(type, lexeme, startLine, startColumn);
    }
    
    Token scanIdentifier(char firstChar, size_t startLine, size_t startColumn) {
        std::string lexeme(1, firstChar);
        
        while (!isAtEnd()) {
            char ch = currentChar();
            if (std::isalnum(ch) || ch == '_') {
                lexeme += advance();
            } else {
                break;
            }
        }
        
        auto it = keywords.find(lexeme);
        TokenType type = (it != keywords.end()) ? it->second : TokenType::IDENTIFIER;
        
        return Token(type, lexeme, startLine, startColumn);
    }
    
    Token scanToken() {
        size_t startLine = line;
        size_t startColumn = column;
        
        if (isAtEnd()) {
            return Token(TokenType::EOF_TOKEN, "", startLine, startColumn);
        }
        
        char ch = advance();
        
        switch (ch) {
            // Single character tokens
            case '(': return Token(TokenType::LEFT_PAREN, "(", startLine, startColumn);
            case ')': return Token(TokenType::RIGHT_PAREN, ")", startLine, startColumn);
            case '{': return Token(TokenType::LEFT_BRACE, "{", startLine, startColumn);
            case '}': return Token(TokenType::RIGHT_BRACE, "}", startLine, startColumn);
            case '[': return Token(TokenType::LEFT_BRACKET, "[", startLine, startColumn);
            case ']': return Token(TokenType::RIGHT_BRACKET, "]", startLine, startColumn);
            case ';': return Token(TokenType::SEMICOLON, ";", startLine, startColumn);
            case ',': return Token(TokenType::COMMA, ",", startLine, startColumn);
            case '.': return Token(TokenType::DOT, ".", startLine, startColumn);
            case '?': return Token(TokenType::QUESTION, "?", startLine, startColumn);
            case '@': return Token(TokenType::AT, "@", startLine, startColumn);
            case '$': return Token(TokenType::DOLLAR, "$", startLine, startColumn);
            case '#': return Token(TokenType::HASH, "#", startLine, startColumn);
            
            // Potentially multi-character tokens
            case ':':
                if (currentChar() == ':') {
                    advance();
                    return Token(TokenType::DOUBLE_COLON, "::", startLine, startColumn);
                }
                return Token(TokenType::COLON, ":", startLine, startColumn);
            
            case '+':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::PLUS_EQUAL, "+=", startLine, startColumn);
                }
                return Token(TokenType::PLUS, "+", startLine, startColumn);
            
            case '-':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::MINUS_EQUAL, "-=", startLine, startColumn);
                } else if (currentChar() == '>') {
                    advance();
                    return Token(TokenType::ARROW, "->", startLine, startColumn);
                }
                return Token(TokenType::MINUS, "-", startLine, startColumn);
            
            case '*':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::STAR_EQUAL, "*=", startLine, startColumn);
                }
                return Token(TokenType::STAR, "*", startLine, startColumn);
            
            case '/':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::SLASH_EQUAL, "/=", startLine, startColumn);
                } else if (currentChar() == '/') {
                    return scanLineComment(startLine, startColumn);
                } else if (currentChar() == '*') {
                    return scanBlockComment(startLine, startColumn);
                }
                return Token(TokenType::SLASH, "/", startLine, startColumn);
            
            case '%':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::PERCENT_EQUAL, "%=", startLine, startColumn);
                }
                return Token(TokenType::PERCENT, "%", startLine, startColumn);
            
            case '=':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::EQUAL_EQUAL, "==", startLine, startColumn);
                } else if (currentChar() == '>') {
                    advance();
                    return Token(TokenType::FAT_ARROW, "=>", startLine, startColumn);
                }
                return Token(TokenType::EQUAL, "=", startLine, startColumn);
            
            case '!':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::NOT_EQUAL, "!=", startLine, startColumn);
                }
                return Token(TokenType::NOT, "!", startLine, startColumn);
            
            case '<':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::LESS_EQUAL, "<=", startLine, startColumn);
                } else if (currentChar() == '<') {
                    advance();
                    if (currentChar() == '=') {
                        advance();
                        return Token(TokenType::LEFT_SHIFT_EQUAL, "<<=", startLine, startColumn);
                    }
                    return Token(TokenType::LEFT_SHIFT, "<<", startLine, startColumn);
                }
                return Token(TokenType::LESS, "<", startLine, startColumn);
            
            case '>':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::GREATER_EQUAL, ">=", startLine, startColumn);
                } else if (currentChar() == '>') {
                    advance();
                    if (currentChar() == '=') {
                        advance();
                        return Token(TokenType::RIGHT_SHIFT_EQUAL, ">>=", startLine, startColumn);
                    }
                    return Token(TokenType::RIGHT_SHIFT, ">>", startLine, startColumn);
                }
                return Token(TokenType::GREATER, ">", startLine, startColumn);
            
            case '&':
                if (currentChar() == '&') {
                    advance();
                    return Token(TokenType::AND_AND, "&&", startLine, startColumn);
                } else if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::AND_EQUAL, "&=", startLine, startColumn);
                }
                return Token(TokenType::AND, "&", startLine, startColumn);
            
            case '|':
                if (currentChar() == '|') {
                    advance();
                    return Token(TokenType::OR_OR, "||", startLine, startColumn);
                } else if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::OR_EQUAL, "|=", startLine, startColumn);
                }
                return Token(TokenType::OR, "|", startLine, startColumn);
            
            case '^':
                if (currentChar() == '=') {
                    advance();
                    return Token(TokenType::XOR_EQUAL, "^=", startLine, startColumn);
                }
                return Token(TokenType::XOR, "^", startLine, startColumn);
            
            // String and character literals
            case '"':
                return scanStringLiteral(startLine, startColumn);
            case '\'':
                return scanCharLiteral(startLine, startColumn);
            
            // Numbers
            case '0': case '1': case '2': case '3': case '4':
            case '5': case '6': case '7': case '8': case '9':
                return scanNumber(ch, startLine, startColumn);
            
            // Identifiers and keywords
            default:
                if (std::isalpha(ch) || ch == '_') {
                    return scanIdentifier(ch, startLine, startColumn);
                }
                
                // Invalid character (including Unicode)
                return Token(TokenType::INVALID, std::string(1, ch), startLine, startColumn);
        }
    }

public:
    explicit Lexer(const std::string& input) 
        : input(input), position(0), line(1), column(1) {
        initKeywords();
    }
    
    std::vector<Token> tokenize() {
        std::vector<Token> tokens;
        
        while (!isAtEnd()) {
            skipWhitespace();
            if (isAtEnd()) break;
            
            tokens.push_back(scanToken());
        }
        
        tokens.emplace_back(TokenType::EOF_TOKEN, "", line, column);
        return tokens;
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

void benchmarkLexerRealWorld() {
    std::vector<std::string> testFiles = {
        "../../test_data/large_codebases/large_codebase.seen",
        "../../test_data/large_codebases/minified_code.seen",
        "../../test_data/large_codebases/sparse_code.seen",
        "../../test_data/large_codebases/unicode_heavy.seen"
    };
    
    uint64_t totalTokens = 0;
    std::chrono::duration<double> totalTime{0};
    
    for (const auto& filePath : testFiles) {
        if (!std::filesystem::exists(filePath)) {
            std::cout << "Warning: Test file " << filePath << " not found, skipping..." << std::endl;
            continue;
        }
        
        try {
            std::string content = readFile(filePath);
            size_t fileSize = content.size();
            
            std::cout << "Testing C++ lexer performance on " << filePath 
                      << " (" << fileSize << " bytes)" << std::endl;
            
            // Run multiple iterations for statistical accuracy
            const int iterations = 10;
            size_t fileTokens = 0;
            std::chrono::duration<double> fileTime{0};
            
            for (int i = 0; i < iterations; ++i) {
                Lexer lexer(content);
                
                auto startTime = std::chrono::high_resolution_clock::now();
                auto tokens = lexer.tokenize();
                auto endTime = std::chrono::high_resolution_clock::now();
                
                fileTokens = tokens.size();
                fileTime += endTime - startTime;
            }
            
            auto avgTime = fileTime / iterations;
            double tokensPerSecond = avgTime.count() > 0.0 ? 
                static_cast<double>(fileTokens) / avgTime.count() : 0.0;
            
            std::cout << "  Tokens: " << fileTokens 
                      << ", Avg Time: " << std::fixed << std::setprecision(6) << avgTime.count() << "s"
                      << ", Tokens/sec: " << std::fixed << std::setprecision(0) << tokensPerSecond << std::endl;
            
            totalTokens += fileTokens;
            totalTime += avgTime;
            
        } catch (const std::exception& e) {
            std::cerr << "Error processing " << filePath << ": " << e.what() << std::endl;
        }
    }
    
    double overallTokensPerSec = totalTime.count() > 0.0 ? 
        static_cast<double>(totalTokens) / totalTime.count() : 0.0;
    
    std::cout << "\nC++ Lexer Overall Performance:" << std::endl;
    std::cout << "  Total tokens: " << totalTokens << std::endl;
    std::cout << "  Total time: " << std::fixed << std::setprecision(6) << totalTime.count() << "s" << std::endl;
    std::cout << "  Average tokens/second: " << std::fixed << std::setprecision(0) << overallTokensPerSec << std::endl;
    
    // Check if it meets the 14M tokens/sec claim
    if (overallTokensPerSec >= 14000000.0) {
        std::cout << "✅ C++ BASELINE: Achieved " << std::fixed << std::setprecision(1) 
                  << overallTokensPerSec / 1000000.0 << "M tokens/sec" << std::endl;
    } else {
        std::cout << "❌ C++ BASELINE: Achieved " << std::fixed << std::setprecision(1) 
                  << overallTokensPerSec / 1000000.0 << "M tokens/sec (target: 14M)" << std::endl;
    }
}

int main() {
    try {
        benchmarkLexerRealWorld();
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Error running C++ lexer benchmark: " << e.what() << std::endl;
        return 1;
    }
}