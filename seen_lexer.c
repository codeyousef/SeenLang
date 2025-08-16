#include "seen_lexer.h"
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>

// Simple TOML parser for keyword loading
static char* load_file(const char* path) {
    FILE* file = fopen(path, "r");
    if (!file) return NULL;
    
    fseek(file, 0, SEEK_END);
    long size = ftell(file);
    fseek(file, 0, SEEK_SET);
    
    char* content = malloc(size + 1);
    if (!content) {
        fclose(file);
        return NULL;
    }
    
    fread(content, 1, size, file);
    content[size] = '\0';
    fclose(file);
    return content;
}

// Parse TOML keywords section
static void parse_keywords_from_toml(KeywordManager* manager, const char* content) {
    char* line_start = (char*)content;
    char* line_end;
    
    bool in_keywords_section = false;
    
    while ((line_end = strchr(line_start, '\n')) != NULL) {
        *line_end = '\0';
        
        // Trim whitespace
        char* line = line_start;
        while (isspace(*line)) line++;
        
        // Check for [keywords] section
        if (strncmp(line, "[keywords]", 10) == 0) {
            in_keywords_section = true;
        }
        // Check for new section
        else if (*line == '[' && in_keywords_section) {
            in_keywords_section = false;
        }
        // Parse keyword line
        else if (in_keywords_section && *line != '\0' && *line != '#') {
            char* equals = strchr(line, '=');
            if (equals) {
                *equals = '\0';
                char* key = line;
                char* value = equals + 1;
                
                // Trim spaces
                while (isspace(*key)) key++;
                char* key_end = key + strlen(key) - 1;
                while (key_end > key && isspace(*key_end)) *key_end-- = '\0';
                
                while (isspace(*value)) value++;
                if (*value == '"') value++;
                char* value_end = value + strlen(value) - 1;
                while (value_end > value && (isspace(*value_end) || *value_end == '"')) *value_end-- = '\0';
                
                // Map keywords to token types
                TokenType token_type = TOKEN_IDENTIFIER;
                if (strcmp(key, "let") == 0) token_type = TOKEN_LET;
                else if (strcmp(key, "var") == 0) token_type = TOKEN_VAR;
                else if (strcmp(key, "const") == 0) token_type = TOKEN_CONST;
                else if (strcmp(key, "fun") == 0) token_type = TOKEN_FUN;
                else if (strcmp(key, "struct") == 0) token_type = TOKEN_STRUCT;
                else if (strcmp(key, "class") == 0) token_type = TOKEN_CLASS;
                else if (strcmp(key, "interface") == 0) token_type = TOKEN_INTERFACE;
                else if (strcmp(key, "enum") == 0) token_type = TOKEN_ENUM;
                else if (strcmp(key, "if") == 0) token_type = TOKEN_IF;
                else if (strcmp(key, "else") == 0) token_type = TOKEN_ELSE;
                else if (strcmp(key, "match") == 0) token_type = TOKEN_MATCH;
                else if (strcmp(key, "for") == 0) token_type = TOKEN_FOR;
                else if (strcmp(key, "while") == 0) token_type = TOKEN_WHILE;
                else if (strcmp(key, "loop") == 0) token_type = TOKEN_LOOP;
                else if (strcmp(key, "break") == 0) token_type = TOKEN_BREAK;
                else if (strcmp(key, "continue") == 0) token_type = TOKEN_CONTINUE;
                else if (strcmp(key, "return") == 0) token_type = TOKEN_RETURN;
                else if (strcmp(key, "and") == 0) token_type = TOKEN_AND;
                else if (strcmp(key, "or") == 0) token_type = TOKEN_OR;
                else if (strcmp(key, "not") == 0) token_type = TOKEN_NOT;
                else if (strcmp(key, "is") == 0) token_type = TOKEN_IS;
                else if (strcmp(key, "in") == 0) token_type = TOKEN_IN;
                else if (strcmp(key, "as") == 0) token_type = TOKEN_AS;
                else if (strcmp(key, "try") == 0) token_type = TOKEN_TRY;
                else if (strcmp(key, "catch") == 0) token_type = TOKEN_CATCH;
                else if (strcmp(key, "throw") == 0) token_type = TOKEN_THROW;
                else if (strcmp(key, "async") == 0) token_type = TOKEN_ASYNC;
                else if (strcmp(key, "await") == 0) token_type = TOKEN_AWAIT;
                else if (strcmp(key, "yield") == 0) token_type = TOKEN_YIELD;
                else if (strcmp(key, "import") == 0) token_type = TOKEN_IMPORT;
                else if (strcmp(key, "export") == 0) token_type = TOKEN_EXPORT;
                else if (strcmp(key, "module") == 0) token_type = TOKEN_MODULE;
                else if (strcmp(key, "use") == 0) token_type = TOKEN_USE;
                else if (strcmp(key, "move") == 0) token_type = TOKEN_MOVE;
                else if (strcmp(key, "borrow") == 0) token_type = TOKEN_BORROW;
                else if (strcmp(key, "mut") == 0) token_type = TOKEN_MUT;
                else if (strcmp(key, "comptime") == 0) token_type = TOKEN_COMPTIME;
                else if (strcmp(key, "effect") == 0) token_type = TOKEN_EFFECT;
                else if (strcmp(key, "react") == 0) token_type = TOKEN_REACT;
                else if (strcmp(key, "observe") == 0) token_type = TOKEN_OBSERVE;
                else if (strcmp(key, "subscribe") == 0) token_type = TOKEN_SUBSCRIBE;
                else if (strcmp(key, "true") == 0 || strcmp(key, "false") == 0) token_type = TOKEN_BOOL_LITERAL;
                else if (strcmp(key, "null") == 0) token_type = TOKEN_NULL_LITERAL;
                
                // Add mapping
                if (manager->mapping_count >= manager->mapping_capacity) {
                    manager->mapping_capacity = manager->mapping_capacity ? manager->mapping_capacity * 2 : 32;
                    manager->mappings = realloc(manager->mappings, 
                        sizeof(KeywordMapping) * manager->mapping_capacity);
                }
                
                manager->mappings[manager->mapping_count].keyword = strdup(value);
                manager->mappings[manager->mapping_count].token_type = token_type;
                manager->mapping_count++;
            }
        }
        
        line_start = line_end + 1;
        *line_end = '\n';
    }
}

// Keyword Manager Implementation
KeywordManager* keyword_manager_create(const char* language) {
    KeywordManager* manager = malloc(sizeof(KeywordManager));
    if (!manager) return NULL;
    
    manager->mappings = NULL;
    manager->mapping_count = 0;
    manager->mapping_capacity = 0;
    manager->language = strdup(language ? language : "en");
    
    return manager;
}

void keyword_manager_destroy(KeywordManager* manager) {
    if (!manager) return;
    
    for (int i = 0; i < manager->mapping_count; i++) {
        free(manager->mappings[i].keyword);
    }
    free(manager->mappings);
    free(manager->language);
    free(manager);
}

bool keyword_manager_load_from_toml(KeywordManager* manager, const char* toml_path) {
    if (!manager) return false;
    
    char* content = load_file(toml_path);
    if (!content) return false;
    
    parse_keywords_from_toml(manager, content);
    free(content);
    
    return true;
}

TokenType keyword_manager_get_token_type(KeywordManager* manager, const char* word) {
    if (!manager || !word) return TOKEN_IDENTIFIER;
    
    for (int i = 0; i < manager->mapping_count; i++) {
        if (strcmp(manager->mappings[i].keyword, word) == 0) {
            return manager->mappings[i].token_type;
        }
    }
    
    return TOKEN_IDENTIFIER;
}

// Lexer Implementation
Lexer* lexer_create(const char* source, const char* filename, const char* language) {
    Lexer* lexer = malloc(sizeof(Lexer));
    if (!lexer) return NULL;
    
    lexer->source = strdup(source);
    lexer->length = strlen(source);
    lexer->position = 0;
    lexer->line = 1;
    lexer->column = 1;
    lexer->filename = strdup(filename ? filename : "<unknown>");
    lexer->tokens = NULL;
    lexer->token_count = 0;
    lexer->token_capacity = 0;
    lexer->has_errors = false;
    lexer->errors = NULL;
    lexer->error_count = 0;
    
    // Create keyword manager and load language file
    lexer->keyword_manager = keyword_manager_create(language);
    if (lexer->keyword_manager) {
        char toml_path[256];
        snprintf(toml_path, sizeof(toml_path), "languages/%s.toml", language ? language : "en");
        keyword_manager_load_from_toml(lexer->keyword_manager, toml_path);
    }
    
    return lexer;
}

void lexer_destroy(Lexer* lexer) {
    if (!lexer) return;
    
    free(lexer->source);
    free(lexer->filename);
    
    for (int i = 0; i < lexer->token_count; i++) {
        free(lexer->tokens[i].value);
    }
    free(lexer->tokens);
    
    for (int i = 0; i < lexer->error_count; i++) {
        free(lexer->errors[i]);
    }
    free(lexer->errors);
    
    keyword_manager_destroy(lexer->keyword_manager);
    free(lexer);
}

static char lexer_current(Lexer* lexer) {
    if (lexer->position >= lexer->length) return '\0';
    return lexer->source[lexer->position];
}

static char lexer_peek(Lexer* lexer, int offset) {
    int pos = lexer->position + offset;
    if (pos >= lexer->length) return '\0';
    return lexer->source[pos];
}

static void lexer_advance(Lexer* lexer) {
    if (lexer->position < lexer->length) {
        if (lexer->source[lexer->position] == '\n') {
            lexer->line++;
            lexer->column = 1;
        } else {
            lexer->column++;
        }
        lexer->position++;
    }
}

static bool lexer_match(Lexer* lexer, char expected) {
    if (lexer_current(lexer) == expected) {
        lexer_advance(lexer);
        return true;
    }
    return false;
}

static void lexer_add_token(Lexer* lexer, TokenType type, const char* value) {
    if (lexer->token_count >= lexer->token_capacity) {
        lexer->token_capacity = lexer->token_capacity ? lexer->token_capacity * 2 : 64;
        lexer->tokens = realloc(lexer->tokens, sizeof(Token) * lexer->token_capacity);
    }
    
    Token* token = &lexer->tokens[lexer->token_count++];
    token->type = type;
    token->value = strdup(value);
    token->start.line = lexer->line;
    token->start.column = lexer->column;
    token->start.offset = lexer->position;
    token->start.filename = lexer->filename;
    token->length = strlen(value);
}

static void lexer_add_error(Lexer* lexer, const char* message) {
    lexer->has_errors = true;
    lexer->errors = realloc(lexer->errors, sizeof(char*) * (lexer->error_count + 1));
    lexer->errors[lexer->error_count++] = strdup(message);
}

static void lexer_skip_whitespace(Lexer* lexer) {
    while (isspace(lexer_current(lexer)) && lexer_current(lexer) != '\n') {
        lexer_advance(lexer);
    }
}

static void lexer_scan_string(Lexer* lexer) {
    char quote = lexer_current(lexer);
    lexer_advance(lexer); // Skip opening quote
    
    char* buffer = malloc(1024);
    int buffer_size = 1024;
    int buffer_pos = 0;
    
    bool is_multiline = false;
    
    // Check for triple quotes
    if (quote == '"' && lexer_current(lexer) == '"' && lexer_peek(lexer, 1) == '"') {
        is_multiline = true;
        lexer_advance(lexer); // Skip second quote
        lexer_advance(lexer); // Skip third quote
    }
    
    while (lexer_current(lexer) != '\0') {
        char c = lexer_current(lexer);
        
        if (!is_multiline && c == quote) {
            lexer_advance(lexer);
            break;
        } else if (is_multiline && c == '"' && lexer_peek(lexer, 1) == '"' && lexer_peek(lexer, 2) == '"') {
            lexer_advance(lexer); // Skip first quote
            lexer_advance(lexer); // Skip second quote
            lexer_advance(lexer); // Skip third quote
            break;
        } else if (c == '\\') {
            lexer_advance(lexer);
            char escaped = lexer_current(lexer);
            switch (escaped) {
                case 'n': buffer[buffer_pos++] = '\n'; break;
                case 't': buffer[buffer_pos++] = '\t'; break;
                case 'r': buffer[buffer_pos++] = '\r'; break;
                case '\\': buffer[buffer_pos++] = '\\'; break;
                case '"': buffer[buffer_pos++] = '"'; break;
                case '\'': buffer[buffer_pos++] = '\''; break;
                default: 
                    buffer[buffer_pos++] = '\\';
                    buffer[buffer_pos++] = escaped;
                    break;
            }
            lexer_advance(lexer);
        } else {
            buffer[buffer_pos++] = c;
            lexer_advance(lexer);
            
            if (buffer_pos >= buffer_size - 1) {
                buffer_size *= 2;
                buffer = realloc(buffer, buffer_size);
            }
        }
    }
    
    buffer[buffer_pos] = '\0';
    lexer_add_token(lexer, TOKEN_STRING_LITERAL, buffer);
    free(buffer);
}

static void lexer_scan_char(Lexer* lexer) {
    lexer_advance(lexer); // Skip opening quote
    
    char c = lexer_current(lexer);
    char result[2] = {c, '\0'};
    
    if (c == '\\') {
        lexer_advance(lexer);
        char escaped = lexer_current(lexer);
        switch (escaped) {
            case 'n': result[0] = '\n'; break;
            case 't': result[0] = '\t'; break;
            case 'r': result[0] = '\r'; break;
            case '\\': result[0] = '\\'; break;
            case '\'': result[0] = '\''; break;
            default: result[0] = escaped; break;
        }
    }
    
    lexer_advance(lexer);
    
    if (lexer_current(lexer) == '\'') {
        lexer_advance(lexer);
        lexer_add_token(lexer, TOKEN_CHAR_LITERAL, result);
    } else {
        lexer_add_error(lexer, "Unterminated character literal");
    }
}

static void lexer_scan_number(Lexer* lexer) {
    char* buffer = malloc(64);
    int buffer_pos = 0;
    bool is_float = false;
    
    // Scan integer part
    while (isdigit(lexer_current(lexer)) || lexer_current(lexer) == '_') {
        if (lexer_current(lexer) != '_') {
            buffer[buffer_pos++] = lexer_current(lexer);
        }
        lexer_advance(lexer);
    }
    
    // Check for decimal point
    if (lexer_current(lexer) == '.' && isdigit(lexer_peek(lexer, 1))) {
        is_float = true;
        buffer[buffer_pos++] = '.';
        lexer_advance(lexer);
        
        while (isdigit(lexer_current(lexer)) || lexer_current(lexer) == '_') {
            if (lexer_current(lexer) != '_') {
                buffer[buffer_pos++] = lexer_current(lexer);
            }
            lexer_advance(lexer);
        }
    }
    
    // Check for scientific notation
    if (lexer_current(lexer) == 'e' || lexer_current(lexer) == 'E') {
        is_float = true;
        buffer[buffer_pos++] = lexer_current(lexer);
        lexer_advance(lexer);
        
        if (lexer_current(lexer) == '+' || lexer_current(lexer) == '-') {
            buffer[buffer_pos++] = lexer_current(lexer);
            lexer_advance(lexer);
        }
        
        while (isdigit(lexer_current(lexer))) {
            buffer[buffer_pos++] = lexer_current(lexer);
            lexer_advance(lexer);
        }
    }
    
    // Check for type suffixes
    char c = lexer_current(lexer);
    if (c == 'u' || c == 'U') {
        buffer[buffer_pos++] = c;
        lexer_advance(lexer);
        if (lexer_current(lexer) == 'L' || lexer_current(lexer) == 'l') {
            buffer[buffer_pos++] = lexer_current(lexer);
            lexer_advance(lexer);
        }
    } else if (c == 'L' || c == 'l') {
        buffer[buffer_pos++] = c;
        lexer_advance(lexer);
    } else if (c == 'f' || c == 'F') {
        is_float = true;
        buffer[buffer_pos++] = c;
        lexer_advance(lexer);
    }
    
    buffer[buffer_pos] = '\0';
    lexer_add_token(lexer, is_float ? TOKEN_FLOAT_LITERAL : TOKEN_INTEGER_LITERAL, buffer);
    free(buffer);
}

static void lexer_scan_identifier(Lexer* lexer) {
    char* buffer = malloc(256);
    int buffer_pos = 0;
    
    while (isalnum(lexer_current(lexer)) || lexer_current(lexer) == '_') {
        buffer[buffer_pos++] = lexer_current(lexer);
        lexer_advance(lexer);
    }
    
    buffer[buffer_pos] = '\0';
    
    // Check if it's a keyword using the multilingual keyword manager
    TokenType token_type = keyword_manager_get_token_type(lexer->keyword_manager, buffer);
    
    lexer_add_token(lexer, token_type, buffer);
    free(buffer);
}

static void lexer_scan_comment(Lexer* lexer) {
    char* buffer = malloc(1024);
    int buffer_pos = 0;
    bool is_doc_comment = false;
    
    if (lexer_current(lexer) == '/' && lexer_peek(lexer, 1) == '/') {
        // Single line comment
        lexer_advance(lexer); // Skip first /
        lexer_advance(lexer); // Skip second /
        
        // Check for doc comment ///
        if (lexer_current(lexer) == '/') {
            is_doc_comment = true;
            lexer_advance(lexer);
        }
        
        while (lexer_current(lexer) != '\0' && lexer_current(lexer) != '\n') {
            buffer[buffer_pos++] = lexer_current(lexer);
            lexer_advance(lexer);
        }
    } else if (lexer_current(lexer) == '/' && lexer_peek(lexer, 1) == '*') {
        // Multi-line comment
        lexer_advance(lexer); // Skip /
        lexer_advance(lexer); // Skip *
        
        // Check for doc comment /**
        if (lexer_current(lexer) == '*') {
            is_doc_comment = true;
            lexer_advance(lexer);
        }
        
        while (lexer_current(lexer) != '\0') {
            if (lexer_current(lexer) == '*' && lexer_peek(lexer, 1) == '/') {
                lexer_advance(lexer); // Skip *
                lexer_advance(lexer); // Skip /
                break;
            }
            buffer[buffer_pos++] = lexer_current(lexer);
            lexer_advance(lexer);
        }
    }
    
    buffer[buffer_pos] = '\0';
    lexer_add_token(lexer, is_doc_comment ? TOKEN_DOC_COMMENT : TOKEN_COMMENT, buffer);
    free(buffer);
}

static bool lexer_scan_token(Lexer* lexer) {
    char c = lexer_current(lexer);
    
    switch (c) {
        case '\0': return false;
        case '\n': 
            lexer_add_token(lexer, TOKEN_NEWLINE, "\n");
            lexer_advance(lexer);
            break;
        case ' ': case '\t': case '\r':
            lexer_skip_whitespace(lexer);
            break;
        case '(': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_LEFT_PAREN, "("); break;
        case ')': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_RIGHT_PAREN, ")"); break;
        case '[': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_LEFT_BRACKET, "["); break;
        case ']': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_RIGHT_BRACKET, "]"); break;
        case '{': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_LEFT_BRACE, "{"); break;
        case '}': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_RIGHT_BRACE, "}"); break;
        case ',': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_COMMA, ","); break;
        case ';': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_SEMICOLON, ";"); break;
        case '@': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_AT, "@"); break;
        case '#': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_HASH, "#"); break;
        case '$': lexer_advance(lexer); lexer_add_token(lexer, TOKEN_DOLLAR, "$"); break;
        
        case '+':
            lexer_advance(lexer);
            if (lexer_match(lexer, '+')) lexer_add_token(lexer, TOKEN_INCREMENT, "++");
            else if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_PLUS_ASSIGN, "+=");
            else lexer_add_token(lexer, TOKEN_PLUS, "+");
            break;
            
        case '-':
            lexer_advance(lexer);
            if (lexer_match(lexer, '-')) lexer_add_token(lexer, TOKEN_DECREMENT, "--");
            else if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_MINUS_ASSIGN, "-=");
            else if (lexer_match(lexer, '>')) lexer_add_token(lexer, TOKEN_ARROW, "->");
            else lexer_add_token(lexer, TOKEN_MINUS, "-");
            break;
            
        case '*':
            lexer_advance(lexer);
            if (lexer_match(lexer, '*')) lexer_add_token(lexer, TOKEN_POWER, "**");
            else if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_MULTIPLY_ASSIGN, "*=");
            else lexer_add_token(lexer, TOKEN_MULTIPLY, "*");
            break;
            
        case '/':
            if (lexer_peek(lexer, 1) == '/' || lexer_peek(lexer, 1) == '*') {
                lexer_scan_comment(lexer);
            } else {
                lexer_advance(lexer);
                if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_DIVIDE_ASSIGN, "/=");
                else lexer_add_token(lexer, TOKEN_DIVIDE, "/");
            }
            break;
            
        case '%':
            lexer_advance(lexer);
            if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_MODULO_ASSIGN, "%=");
            else lexer_add_token(lexer, TOKEN_MODULO, "%");
            break;
            
        case '=':
            lexer_advance(lexer);
            if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_EQUAL, "==");
            else if (lexer_match(lexer, '>')) lexer_add_token(lexer, TOKEN_FAT_ARROW, "=>");
            else lexer_add_token(lexer, TOKEN_ASSIGN, "=");
            break;
            
        case '!':
            lexer_advance(lexer);
            if (lexer_match(lexer, '!')) lexer_add_token(lexer, TOKEN_FORCE_UNWRAP, "!!");
            else if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_NOT_EQUAL, "!=");
            else lexer_add_token(lexer, TOKEN_EXCLAMATION, "!");
            break;
            
        case '<':
            lexer_advance(lexer);
            if (lexer_match(lexer, '=')) {
                if (lexer_match(lexer, '>')) lexer_add_token(lexer, TOKEN_SPACESHIP, "<=>");
                else lexer_add_token(lexer, TOKEN_LESS_EQUAL, "<=");
            } else lexer_add_token(lexer, TOKEN_LESS, "<");
            break;
            
        case '>':
            lexer_advance(lexer);
            if (lexer_match(lexer, '=')) lexer_add_token(lexer, TOKEN_GREATER_EQUAL, ">=");
            else lexer_add_token(lexer, TOKEN_GREATER, ">");
            break;
            
        case '?':
            lexer_advance(lexer);
            if (lexer_match(lexer, '.')) lexer_add_token(lexer, TOKEN_SAFE_NAV, "?.");
            else if (lexer_match(lexer, ':')) lexer_add_token(lexer, TOKEN_ELVIS, "?:");
            else lexer_add_token(lexer, TOKEN_QUESTION, "?");
            break;
            
        case ':':
            lexer_advance(lexer);
            if (lexer_match(lexer, ':')) lexer_add_token(lexer, TOKEN_DOUBLE_COLON, "::");
            else lexer_add_token(lexer, TOKEN_COLON, ":");
            break;
            
        case '.':
            if (isdigit(lexer_peek(lexer, 1))) {
                lexer_scan_number(lexer);
            } else {
                lexer_advance(lexer);
                if (lexer_match(lexer, '.')) {
                    if (lexer_match(lexer, '<')) lexer_add_token(lexer, TOKEN_RANGE_EXCLUSIVE, "..<");
                    else lexer_add_token(lexer, TOKEN_RANGE_INCLUSIVE, "..");
                } else {
                    lexer_add_token(lexer, TOKEN_DOT, ".");
                }
            }
            break;
            
        case '|':
            lexer_advance(lexer);
            if (lexer_match(lexer, '>')) lexer_add_token(lexer, TOKEN_PIPELINE, "|>");
            else {
                char error[64];
                snprintf(error, sizeof(error), "Unexpected character: '%c'", c);
                lexer_add_error(lexer, error);
            }
            break;
            
        case '"':
            lexer_scan_string(lexer);
            break;
            
        case '\'':
            lexer_scan_char(lexer);
            break;
            
        default:
            if (isdigit(c)) {
                lexer_scan_number(lexer);
            } else if (isalpha(c) || c == '_') {
                lexer_scan_identifier(lexer);
            } else {
                char error[64];
                snprintf(error, sizeof(error), "Unexpected character: '%c'", c);
                lexer_add_error(lexer, error);
                lexer_advance(lexer);
            }
            break;
    }
    
    return true;
}

bool lexer_tokenize(Lexer* lexer) {
    if (!lexer) return false;
    
    while (lexer_scan_token(lexer)) {
        // Continue tokenizing
    }
    
    lexer_add_token(lexer, TOKEN_EOF, "");
    return !lexer->has_errors;
}

Token* lexer_get_tokens(Lexer* lexer, int* count) {
    if (!lexer || !count) return NULL;
    *count = lexer->token_count;
    return lexer->tokens;
}

char** lexer_get_errors(Lexer* lexer, int* count) {
    if (!lexer || !count) return NULL;
    *count = lexer->error_count;
    return lexer->errors;
}