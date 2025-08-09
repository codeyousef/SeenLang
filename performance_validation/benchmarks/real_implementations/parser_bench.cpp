// Real Parser Benchmark - C++ Implementation
#include <iostream>
#include <fstream>
#include <sstream>
#include <chrono>
#include <vector>
#include <string>
#include <stack>
#include <unordered_map>
#include <memory>

// Simple AST node structure
struct ASTNode {
    std::string type;
    std::string value;
    std::vector<std::shared_ptr<ASTNode>> children;
    
    ASTNode(const std::string& t, const std::string& v = "") : type(t), value(v) {}
};

class SimpleParser {
private:
    std::string input;
    size_t position;
    int nodesCreated;
    
    // Token types
    enum TokenType {
        IDENTIFIER, NUMBER, STRING, KEYWORD, OPERATOR, DELIMITER, END
    };
    
    struct Token {
        TokenType type;
        std::string value;
    };
    
    Token currentToken;
    
    void skipWhitespace() {
        while (position < input.length() && std::isspace(input[position])) {
            position++;
        }
    }
    
    Token nextToken() {
        skipWhitespace();
        
        if (position >= input.length()) {
            return {END, ""};
        }
        
        char c = input[position];
        
        // Identifier or keyword
        if (std::isalpha(c) || c == '_') {
            std::string value;
            while (position < input.length() && (std::isalnum(input[position]) || input[position] == '_')) {
                value += input[position++];
            }
            
            // Check if it's a keyword
            static const std::unordered_map<std::string, bool> keywords = {
                {"fun", true}, {"val", true}, {"var", true}, {"if", true},
                {"else", true}, {"while", true}, {"for", true}, {"return", true},
                {"class", true}, {"interface", true}, {"import", true}
            };
            
            return {keywords.count(value) ? KEYWORD : IDENTIFIER, value};
        }
        
        // Number
        if (std::isdigit(c)) {
            std::string value;
            while (position < input.length() && (std::isdigit(input[position]) || input[position] == '.')) {
                value += input[position++];
            }
            return {NUMBER, value};
        }
        
        // String
        if (c == '"' || c == '\'') {
            char quote = c;
            position++;
            std::string value;
            while (position < input.length() && input[position] != quote) {
                if (input[position] == '\\' && position + 1 < input.length()) {
                    position++; // Skip escape sequence
                }
                value += input[position++];
            }
            if (position < input.length()) position++; // Skip closing quote
            return {STRING, value};
        }
        
        // Operators and delimiters
        static const std::string operators = "+-*/%=<>!&|";
        static const std::string delimiters = "(){}[],;:.";
        
        if (operators.find(c) != std::string::npos) {
            std::string value(1, c);
            position++;
            // Check for two-character operators
            if (position < input.length()) {
                char next = input[position];
                if ((c == '=' && next == '=') || (c == '!' && next == '=') ||
                    (c == '<' && next == '=') || (c == '>' && next == '=') ||
                    (c == '&' && next == '&') || (c == '|' && next == '|')) {
                    value += next;
                    position++;
                }
            }
            return {OPERATOR, value};
        }
        
        if (delimiters.find(c) != std::string::npos) {
            position++;
            return {DELIMITER, std::string(1, c)};
        }
        
        // Unknown character, skip it
        position++;
        return nextToken();
    }
    
public:
    SimpleParser(const std::string& code) : input(code), position(0), nodesCreated(0) {}
    
    std::shared_ptr<ASTNode> parse() {
        auto root = std::make_shared<ASTNode>("Program");
        nodesCreated++;
        
        currentToken = nextToken();
        
        while (currentToken.type != END) {
            auto stmt = parseStatement();
            if (stmt) {
                root->children.push_back(stmt);
            }
        }
        
        return root;
    }
    
    std::shared_ptr<ASTNode> parseStatement() {
        if (currentToken.type == KEYWORD) {
            if (currentToken.value == "fun") {
                return parseFunctionDeclaration();
            } else if (currentToken.value == "val" || currentToken.value == "var") {
                return parseVariableDeclaration();
            } else if (currentToken.value == "if") {
                return parseIfStatement();
            } else if (currentToken.value == "while") {
                return parseWhileStatement();
            } else if (currentToken.value == "for") {
                return parseForStatement();
            } else if (currentToken.value == "return") {
                return parseReturnStatement();
            } else if (currentToken.value == "class") {
                return parseClassDeclaration();
            }
        }
        
        return parseExpression();
    }
    
    std::shared_ptr<ASTNode> parseFunctionDeclaration() {
        auto node = std::make_shared<ASTNode>("FunctionDecl");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'fun'
        if (currentToken.type == IDENTIFIER) {
            node->value = currentToken.value;
            currentToken = nextToken();
        }
        
        // Parse parameters
        if (currentToken.type == DELIMITER && currentToken.value == "(") {
            currentToken = nextToken();
            while (currentToken.type != DELIMITER || currentToken.value != ")") {
                if (currentToken.type == IDENTIFIER) {
                    auto param = std::make_shared<ASTNode>("Parameter", currentToken.value);
                    nodesCreated++;
                    node->children.push_back(param);
                }
                currentToken = nextToken();
            }
            currentToken = nextToken(); // Skip ')'
        }
        
        // Parse body
        if (currentToken.type == DELIMITER && currentToken.value == "{") {
            node->children.push_back(parseBlock());
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseVariableDeclaration() {
        auto node = std::make_shared<ASTNode>("VarDecl", currentToken.value);
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'val' or 'var'
        if (currentToken.type == IDENTIFIER) {
            auto id = std::make_shared<ASTNode>("Identifier", currentToken.value);
            nodesCreated++;
            node->children.push_back(id);
            currentToken = nextToken();
        }
        
        if (currentToken.type == OPERATOR && currentToken.value == "=") {
            currentToken = nextToken();
            node->children.push_back(parseExpression());
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseIfStatement() {
        auto node = std::make_shared<ASTNode>("IfStatement");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'if'
        if (currentToken.type == DELIMITER && currentToken.value == "(") {
            currentToken = nextToken();
            node->children.push_back(parseExpression());
            if (currentToken.type == DELIMITER && currentToken.value == ")") {
                currentToken = nextToken();
            }
        }
        
        node->children.push_back(parseStatement());
        
        if (currentToken.type == KEYWORD && currentToken.value == "else") {
            currentToken = nextToken();
            node->children.push_back(parseStatement());
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseWhileStatement() {
        auto node = std::make_shared<ASTNode>("WhileStatement");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'while'
        if (currentToken.type == DELIMITER && currentToken.value == "(") {
            currentToken = nextToken();
            node->children.push_back(parseExpression());
            if (currentToken.type == DELIMITER && currentToken.value == ")") {
                currentToken = nextToken();
            }
        }
        
        node->children.push_back(parseStatement());
        return node;
    }
    
    std::shared_ptr<ASTNode> parseForStatement() {
        auto node = std::make_shared<ASTNode>("ForStatement");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'for'
        if (currentToken.type == DELIMITER && currentToken.value == "(") {
            currentToken = nextToken();
            // Simple for loop parsing
            while (currentToken.type != DELIMITER || currentToken.value != ")") {
                currentToken = nextToken();
            }
            currentToken = nextToken(); // Skip ')'
        }
        
        node->children.push_back(parseStatement());
        return node;
    }
    
    std::shared_ptr<ASTNode> parseReturnStatement() {
        auto node = std::make_shared<ASTNode>("ReturnStatement");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'return'
        if (currentToken.type != DELIMITER || currentToken.value != ";") {
            node->children.push_back(parseExpression());
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseClassDeclaration() {
        auto node = std::make_shared<ASTNode>("ClassDecl");
        nodesCreated++;
        
        currentToken = nextToken(); // Skip 'class'
        if (currentToken.type == IDENTIFIER) {
            node->value = currentToken.value;
            currentToken = nextToken();
        }
        
        if (currentToken.type == DELIMITER && currentToken.value == "{") {
            node->children.push_back(parseBlock());
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseBlock() {
        auto node = std::make_shared<ASTNode>("Block");
        nodesCreated++;
        
        if (currentToken.type == DELIMITER && currentToken.value == "{") {
            currentToken = nextToken();
            while (currentToken.type != DELIMITER || currentToken.value != "}") {
                auto stmt = parseStatement();
                if (stmt) {
                    node->children.push_back(stmt);
                }
                if (currentToken.type == DELIMITER && currentToken.value == ";") {
                    currentToken = nextToken();
                }
            }
            currentToken = nextToken(); // Skip '}'
        }
        
        return node;
    }
    
    std::shared_ptr<ASTNode> parseExpression() {
        return parseBinaryExpression(0);
    }
    
    std::shared_ptr<ASTNode> parseBinaryExpression(int minPrecedence) {
        auto left = parsePrimaryExpression();
        
        while (currentToken.type == OPERATOR) {
            std::string op = currentToken.value;
            int precedence = getOperatorPrecedence(op);
            
            if (precedence < minPrecedence) {
                break;
            }
            
            currentToken = nextToken();
            auto right = parseBinaryExpression(precedence + 1);
            
            auto binOp = std::make_shared<ASTNode>("BinaryOp", op);
            nodesCreated++;
            binOp->children.push_back(left);
            binOp->children.push_back(right);
            left = binOp;
        }
        
        return left;
    }
    
    std::shared_ptr<ASTNode> parsePrimaryExpression() {
        if (currentToken.type == NUMBER) {
            auto node = std::make_shared<ASTNode>("Number", currentToken.value);
            nodesCreated++;
            currentToken = nextToken();
            return node;
        }
        
        if (currentToken.type == STRING) {
            auto node = std::make_shared<ASTNode>("String", currentToken.value);
            nodesCreated++;
            currentToken = nextToken();
            return node;
        }
        
        if (currentToken.type == IDENTIFIER) {
            auto node = std::make_shared<ASTNode>("Identifier", currentToken.value);
            nodesCreated++;
            currentToken = nextToken();
            
            // Check for function call
            if (currentToken.type == DELIMITER && currentToken.value == "(") {
                auto call = std::make_shared<ASTNode>("FunctionCall");
                nodesCreated++;
                call->children.push_back(node);
                
                currentToken = nextToken();
                while (currentToken.type != DELIMITER || currentToken.value != ")") {
                    call->children.push_back(parseExpression());
                    if (currentToken.type == DELIMITER && currentToken.value == ",") {
                        currentToken = nextToken();
                    }
                }
                currentToken = nextToken(); // Skip ')'
                return call;
            }
            
            return node;
        }
        
        if (currentToken.type == DELIMITER && currentToken.value == "(") {
            currentToken = nextToken();
            auto expr = parseExpression();
            if (currentToken.type == DELIMITER && currentToken.value == ")") {
                currentToken = nextToken();
            }
            return expr;
        }
        
        // Skip unknown tokens
        currentToken = nextToken();
        return nullptr;
    }
    
    int getOperatorPrecedence(const std::string& op) {
        static const std::unordered_map<std::string, int> precedence = {
            {"=", 1},
            {"||", 2},
            {"&&", 3},
            {"==", 4}, {"!=", 4},
            {"<", 5}, {">", 5}, {"<=", 5}, {">=", 5},
            {"+", 6}, {"-", 6},
            {"*", 7}, {"/", 7}, {"%", 7}
        };
        
        auto it = precedence.find(op);
        return it != precedence.end() ? it->second : 0;
    }
    
    int getNodeCount() const { return nodesCreated; }
};

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <input_file> [iterations]" << std::endl;
        return 1;
    }
    
    // Read input file
    std::ifstream file(argv[1]);
    if (!file) {
        std::cerr << "Error: Cannot open file " << argv[1] << std::endl;
        return 1;
    }
    
    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string input = buffer.str();
    
    int iterations = argc > 2 ? std::atoi(argv[2]) : 30;
    
    // Warm-up
    for (int i = 0; i < 5; i++) {
        SimpleParser warmup(input);
        warmup.parse();
    }
    
    // Benchmark
    std::vector<double> times;
    int totalNodes = 0;
    
    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        
        SimpleParser parser(input);
        auto ast = parser.parse();
        
        auto end = std::chrono::high_resolution_clock::now();
        double elapsed = std::chrono::duration<double>(end - start).count();
        
        times.push_back(elapsed);
        totalNodes = parser.getNodeCount();
    }
    
    // Calculate statistics
    double sum = 0;
    for (double t : times) sum += t;
    double mean = sum / times.size();
    
    // Output JSON results
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"parser\",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"nodes_processed\": " << totalNodes << ",\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        if (i > 0) std::cout << ", ";
        std::cout << times[i];
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << mean << ",\n";
    std::cout << "  \"nodes_per_second\": " << (totalNodes / mean) << "\n";
    std::cout << "}\n";
    
    return 0;
}