#ifndef SEEN_PARSER_H
#define SEEN_PARSER_H

#include "seen_lexer.h"
#include <stdbool.h>

// AST Node Types
typedef enum {
    AST_PROGRAM,
    AST_MODULE,
    AST_IMPORT,
    AST_EXPORT,
    AST_FUNCTION,
    AST_STRUCT,
    AST_CLASS,
    AST_INTERFACE,
    AST_ENUM,
    AST_VARIABLE_DECLARATION,
    AST_CONSTANT_DECLARATION,
    AST_PARAMETER,
    AST_TYPE_ANNOTATION,
    AST_BLOCK,
    AST_IF_STATEMENT,
    AST_WHILE_LOOP,
    AST_FOR_LOOP,
    AST_MATCH_STATEMENT,
    AST_MATCH_ARM,
    AST_RETURN_STATEMENT,
    AST_BREAK_STATEMENT,
    AST_CONTINUE_STATEMENT,
    AST_EXPRESSION_STATEMENT,
    AST_BINARY_EXPRESSION,
    AST_UNARY_EXPRESSION,
    AST_CALL_EXPRESSION,
    AST_MEMBER_ACCESS,
    AST_INDEX_ACCESS,
    AST_LAMBDA_EXPRESSION,
    AST_ASSIGNMENT,
    AST_IDENTIFIER,
    AST_LITERAL,
    AST_ARRAY_LITERAL,
    AST_MAP_LITERAL,
    AST_SET_LITERAL,
    AST_STRING_INTERPOLATION,
    AST_RANGE_EXPRESSION,
    AST_NULLABLE_TYPE,
    AST_GENERIC_TYPE,
    AST_PATTERN,
    AST_PATTERN_DESTRUCTURE,
    AST_TRY_CATCH,
    AST_ASYNC_BLOCK,
    AST_AWAIT_EXPRESSION,
    AST_REACTIVE_EXPRESSION,
    AST_ERROR
} ASTNodeType;

// Forward declarations
typedef struct ASTNode ASTNode;
typedef struct Parser Parser;

// Position range for AST nodes
typedef struct {
    Position start;
    Position end;
} Range;

// Generic AST node structure
struct ASTNode {
    ASTNodeType type;
    Range range;
    char* text;
    ASTNode** children;
    int child_count;
    int child_capacity;
    
    // Node-specific data
    union {
        struct {
            char* name;
            ASTNode* parameters;
            ASTNode* return_type;
            ASTNode* body;
            bool is_public;
            bool is_async;
            char* documentation;
        } function;
        
        struct {
            char* name;
            ASTNode* fields;
            bool is_public;
            char* documentation;
        } struct_def;
        
        struct {
            char* name;
            ASTNode* type;
            ASTNode* initializer;
            bool is_mutable;
            bool is_public;
            char* documentation;
        } variable;
        
        struct {
            char* name;
            ASTNode* value;
            bool is_public;
            char* documentation;
        } constant;
        
        struct {
            char* name;
            ASTNode* type;
            ASTNode* default_value;
        } parameter;
        
        struct {
            ASTNode* condition;
            ASTNode* then_block;
            ASTNode* else_block;
        } if_stmt;
        
        struct {
            ASTNode* condition;
            ASTNode* body;
        } while_loop;
        
        struct {
            char* variable;
            ASTNode* iterable;
            ASTNode* body;
        } for_loop;
        
        struct {
            ASTNode* expression;
            ASTNode* arms;
        } match_stmt;
        
        struct {
            ASTNode* pattern;
            ASTNode* guard;
            ASTNode* body;
        } match_arm;
        
        struct {
            ASTNode* expression;
        } return_stmt;
        
        struct {
            ASTNode* left;
            TokenType operator;
            ASTNode* right;
        } binary_expr;
        
        struct {
            TokenType operator;
            ASTNode* operand;
        } unary_expr;
        
        struct {
            ASTNode* function;
            ASTNode* arguments;
        } call_expr;
        
        struct {
            ASTNode* object;
            char* member;
            bool is_safe_navigation;
        } member_access;
        
        struct {
            ASTNode* object;
            ASTNode* index;
        } index_access;
        
        struct {
            ASTNode* parameters;
            ASTNode* body;
            ASTNode* return_type;
        } lambda;
        
        struct {
            char* name;
        } identifier;
        
        struct {
            TokenType literal_type;
            char* value;
        } literal;
        
        struct {
            ASTNode* elements;
        } array_literal;
        
        struct {
            ASTNode* pairs;
        } map_literal;
        
        struct {
            ASTNode* elements;
        } set_literal;
        
        struct {
            char* format;
            ASTNode* expressions;
        } string_interpolation;
        
        struct {
            ASTNode* start;
            ASTNode* end;
            bool is_inclusive;
        } range;
        
        struct {
            ASTNode* base_type;
        } nullable_type;
        
        struct {
            ASTNode* base_type;
            ASTNode* type_parameters;
        } generic_type;
        
        struct {
            char* module_name;
            ASTNode* items;
        } import_stmt;
        
        struct {
            ASTNode* try_block;
            ASTNode* catch_clauses;
            ASTNode* finally_block;
        } try_catch;
        
        struct {
            ASTNode* body;
        } async_block;
        
        struct {
            ASTNode* expression;
        } await_expr;
    } data;
};

// Parser state
struct Parser {
    Token* tokens;
    int token_count;
    int current;
    bool has_errors;
    char** errors;
    int error_count;
    ASTNode* root;
};

// Symbol information for LSP features
typedef struct {
    char* name;
    ASTNodeType type;
    Range range;
    ASTNode* definition;
    char* documentation;
    char* type_name;
    bool is_public;
    char* module;
} Symbol;

// Symbol table for tracking definitions and scopes
typedef struct SymbolTable {
    Symbol* symbols;
    int symbol_count;
    int symbol_capacity;
    struct SymbolTable* parent;
    char* scope_name;
} SymbolTable;

// Function declarations
Parser* parser_create(Token* tokens, int token_count);
void parser_destroy(Parser* parser);
bool parser_parse(Parser* parser);
ASTNode* parser_get_ast(Parser* parser);
char** parser_get_errors(Parser* parser, int* count);

// AST node functions
ASTNode* ast_node_create(ASTNodeType type, Range range);
void ast_node_destroy(ASTNode* node);
void ast_node_add_child(ASTNode* parent, ASTNode* child);
ASTNode* ast_node_get_child(ASTNode* node, int index);
void ast_node_set_text(ASTNode* node, const char* text);
void ast_node_set_documentation(ASTNode* node, const char* doc);

// Symbol table functions
SymbolTable* symbol_table_create(const char* scope_name, SymbolTable* parent);
void symbol_table_destroy(SymbolTable* table);
void symbol_table_add_symbol(SymbolTable* table, const Symbol* symbol);
Symbol* symbol_table_lookup(SymbolTable* table, const char* name);
Symbol* symbol_table_lookup_global(SymbolTable* table, const char* name);
void symbol_table_build_from_ast(SymbolTable* table, ASTNode* root);

// LSP helper functions
Symbol* find_symbol_at_position(SymbolTable* table, Position position);
Symbol** find_references(SymbolTable* table, const char* name, int* count);
Symbol** get_completions(SymbolTable* table, Position position, int* count);
char* get_hover_info(Symbol* symbol);

// Internal parsing functions (implemented in .c file)

#endif // SEEN_PARSER_H