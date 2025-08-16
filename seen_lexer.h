#ifndef SEEN_LEXER_H
#define SEEN_LEXER_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <stdbool.h>

// Token types for the Seen language
typedef enum {
    // Keywords
    TOKEN_LET,
    TOKEN_VAR,
    TOKEN_CONST,
    TOKEN_FUN,
    TOKEN_STRUCT,
    TOKEN_CLASS,
    TOKEN_INTERFACE,
    TOKEN_ENUM,
    TOKEN_IF,
    TOKEN_ELSE,
    TOKEN_MATCH,
    TOKEN_FOR,
    TOKEN_WHILE,
    TOKEN_LOOP,
    TOKEN_BREAK,
    TOKEN_CONTINUE,
    TOKEN_RETURN,
    TOKEN_AND,
    TOKEN_OR,
    TOKEN_NOT,
    TOKEN_IS,
    TOKEN_IN,
    TOKEN_AS,
    TOKEN_TRY,
    TOKEN_CATCH,
    TOKEN_THROW,
    TOKEN_ASYNC,
    TOKEN_AWAIT,
    TOKEN_YIELD,
    TOKEN_IMPORT,
    TOKEN_EXPORT,
    TOKEN_MODULE,
    TOKEN_USE,
    TOKEN_MOVE,
    TOKEN_BORROW,
    TOKEN_MUT,
    TOKEN_COMPTIME,
    TOKEN_EFFECT,
    TOKEN_REACT,
    TOKEN_OBSERVE,
    TOKEN_SUBSCRIBE,
    
    // Basic types
    TOKEN_INT,
    TOKEN_UINT,
    TOKEN_LONG,
    TOKEN_ULONG,
    TOKEN_FLOAT,
    TOKEN_DOUBLE,
    TOKEN_BOOL,
    TOKEN_CHAR,
    TOKEN_STRING_TYPE,
    TOKEN_VOID,
    
    // Literals
    TOKEN_INTEGER_LITERAL,
    TOKEN_FLOAT_LITERAL,
    TOKEN_STRING_LITERAL,
    TOKEN_CHAR_LITERAL,
    TOKEN_BOOL_LITERAL,
    TOKEN_NULL_LITERAL,
    
    // Identifiers
    TOKEN_IDENTIFIER,
    
    // Operators
    TOKEN_PLUS,              // +
    TOKEN_MINUS,             // -
    TOKEN_MULTIPLY,          // *
    TOKEN_DIVIDE,            // /
    TOKEN_MODULO,            // %
    TOKEN_POWER,             // **
    TOKEN_ASSIGN,            // =
    TOKEN_PLUS_ASSIGN,       // +=
    TOKEN_MINUS_ASSIGN,      // -=
    TOKEN_MULTIPLY_ASSIGN,   // *=
    TOKEN_DIVIDE_ASSIGN,     // /=
    TOKEN_MODULO_ASSIGN,     // %=
    TOKEN_INCREMENT,         // ++
    TOKEN_DECREMENT,         // --
    
    // Comparison
    TOKEN_EQUAL,             // ==
    TOKEN_NOT_EQUAL,         // !=
    TOKEN_LESS,              // <
    TOKEN_LESS_EQUAL,        // <=
    TOKEN_GREATER,           // >
    TOKEN_GREATER_EQUAL,     // >=
    TOKEN_SPACESHIP,         // <=>
    
    // Special operators
    TOKEN_SAFE_NAV,          // ?.
    TOKEN_ELVIS,             // ?:
    TOKEN_FORCE_UNWRAP,      // !!
    TOKEN_RANGE_INCLUSIVE,   // ..
    TOKEN_RANGE_EXCLUSIVE,   // ..<
    TOKEN_PIPELINE,          // |>
    TOKEN_ARROW,             // ->
    TOKEN_FAT_ARROW,         // =>
    TOKEN_LAMBDA,            // =>
    
    // Punctuation
    TOKEN_DOT,               // .
    TOKEN_COMMA,             // ,
    TOKEN_SEMICOLON,         // ;
    TOKEN_COLON,             // :
    TOKEN_DOUBLE_COLON,      // ::
    TOKEN_QUESTION,          // ?
    TOKEN_EXCLAMATION,       // !
    TOKEN_AT,                // @
    TOKEN_HASH,              // #
    TOKEN_DOLLAR,            // $
    
    // Brackets and Braces
    TOKEN_LEFT_PAREN,        // (
    TOKEN_RIGHT_PAREN,       // )
    TOKEN_LEFT_BRACKET,      // [
    TOKEN_RIGHT_BRACKET,     // ]
    TOKEN_LEFT_BRACE,        // {
    TOKEN_RIGHT_BRACE,       // }
    TOKEN_LEFT_ANGLE,        // <
    TOKEN_RIGHT_ANGLE,       // >
    
    // Comments and documentation
    TOKEN_COMMENT,           // // or /* */
    TOKEN_DOC_COMMENT,       // /** */ or ///
    
    // Special tokens
    TOKEN_NEWLINE,
    TOKEN_WHITESPACE,
    TOKEN_EOF,
    TOKEN_ERROR
} TokenType;

// Position information for each token
typedef struct {
    int line;
    int column;
    int offset;
    char* filename;
} Position;

// Token structure
typedef struct {
    TokenType type;
    char* value;
    Position start;
    Position end;
    int length;
} Token;

// Keyword mapping for multilingual support
typedef struct {
    char* keyword;
    TokenType token_type;
} KeywordMapping;

// Multilingual keyword manager
typedef struct {
    KeywordMapping* mappings;
    int mapping_count;
    int mapping_capacity;
    char* language;
} KeywordManager;

// Lexer state
typedef struct {
    char* source;
    int length;
    int position;
    int line;
    int column;
    char* filename;
    Token* tokens;
    int token_count;
    int token_capacity;
    bool has_errors;
    char** errors;
    int error_count;
    KeywordManager* keyword_manager;
} Lexer;

// Function declarations
KeywordManager* keyword_manager_create(const char* language);
void keyword_manager_destroy(KeywordManager* manager);
bool keyword_manager_load_from_toml(KeywordManager* manager, const char* toml_path);
TokenType keyword_manager_get_token_type(KeywordManager* manager, const char* word);

Lexer* lexer_create(const char* source, const char* filename, const char* language);
void lexer_destroy(Lexer* lexer);
bool lexer_tokenize(Lexer* lexer);
Token* lexer_get_tokens(Lexer* lexer, int* count);
char** lexer_get_errors(Lexer* lexer, int* count);

// Internal functions (implemented in .c file)

#endif // SEEN_LEXER_H