#include "seen_parser.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// AST Node Implementation
ASTNode* ast_node_create(ASTNodeType type, Range range) {
    ASTNode* node = malloc(sizeof(ASTNode));
    if (!node) return NULL;
    
    node->type = type;
    node->range = range;
    node->text = NULL;
    node->children = NULL;
    node->child_count = 0;
    node->child_capacity = 0;
    
    // Initialize union data to null/zero
    memset(&node->data, 0, sizeof(node->data));
    
    return node;
}

void ast_node_destroy(ASTNode* node) {
    if (!node) return;
    
    // Free children
    for (int i = 0; i < node->child_count; i++) {
        ast_node_destroy(node->children[i]);
    }
    free(node->children);
    
    // Free node-specific data
    switch (node->type) {
        case AST_FUNCTION:
            free(node->data.function.name);
            free(node->data.function.documentation);
            break;
        case AST_STRUCT:
            free(node->data.struct_def.name);
            free(node->data.struct_def.documentation);
            break;
        case AST_VARIABLE_DECLARATION:
            free(node->data.variable.name);
            free(node->data.variable.documentation);
            break;
        case AST_CONSTANT_DECLARATION:
            free(node->data.constant.name);
            free(node->data.constant.documentation);
            break;
        case AST_PARAMETER:
            free(node->data.parameter.name);
            break;
        case AST_IDENTIFIER:
            free(node->data.identifier.name);
            break;
        case AST_LITERAL:
            free(node->data.literal.value);
            break;
        case AST_STRING_INTERPOLATION:
            free(node->data.string_interpolation.format);
            break;
        case AST_IMPORT:
            free(node->data.import_stmt.module_name);
            break;
        default:
            break;
    }
    
    free(node->text);
    free(node);
}

void ast_node_add_child(ASTNode* parent, ASTNode* child) {
    if (!parent || !child) return;
    
    if (parent->child_count >= parent->child_capacity) {
        parent->child_capacity = parent->child_capacity ? parent->child_capacity * 2 : 4;
        parent->children = realloc(parent->children, sizeof(ASTNode*) * parent->child_capacity);
    }
    
    parent->children[parent->child_count++] = child;
}

ASTNode* ast_node_get_child(ASTNode* node, int index) {
    if (!node || index < 0 || index >= node->child_count) return NULL;
    return node->children[index];
}

void ast_node_set_text(ASTNode* node, const char* text) {
    if (!node) return;
    free(node->text);
    node->text = text ? strdup(text) : NULL;
}

void ast_node_set_documentation(ASTNode* node, const char* doc) {
    if (!node) return;
    
    switch (node->type) {
        case AST_FUNCTION:
            free(node->data.function.documentation);
            node->data.function.documentation = doc ? strdup(doc) : NULL;
            break;
        case AST_STRUCT:
            free(node->data.struct_def.documentation);
            node->data.struct_def.documentation = doc ? strdup(doc) : NULL;
            break;
        case AST_VARIABLE_DECLARATION:
            free(node->data.variable.documentation);
            node->data.variable.documentation = doc ? strdup(doc) : NULL;
            break;
        case AST_CONSTANT_DECLARATION:
            free(node->data.constant.documentation);
            node->data.constant.documentation = doc ? strdup(doc) : NULL;
            break;
        default:
            break;
    }
}

// Symbol Table Implementation
SymbolTable* symbol_table_create(const char* scope_name, SymbolTable* parent) {
    SymbolTable* table = malloc(sizeof(SymbolTable));
    if (!table) return NULL;
    
    table->symbols = NULL;
    table->symbol_count = 0;
    table->symbol_capacity = 0;
    table->parent = parent;
    table->scope_name = scope_name ? strdup(scope_name) : NULL;
    
    return table;
}

void symbol_table_destroy(SymbolTable* table) {
    if (!table) return;
    
    for (int i = 0; i < table->symbol_count; i++) {
        free(table->symbols[i].name);
        free(table->symbols[i].documentation);
        free(table->symbols[i].type_name);
        free(table->symbols[i].module);
    }
    free(table->symbols);
    free(table->scope_name);
    free(table);
}

void symbol_table_add_symbol(SymbolTable* table, const Symbol* symbol) {
    if (!table || !symbol) return;
    
    if (table->symbol_count >= table->symbol_capacity) {
        table->symbol_capacity = table->symbol_capacity ? table->symbol_capacity * 2 : 16;
        table->symbols = realloc(table->symbols, sizeof(Symbol) * table->symbol_capacity);
    }
    
    Symbol* new_symbol = &table->symbols[table->symbol_count++];
    *new_symbol = *symbol;
    
    // Duplicate strings
    new_symbol->name = symbol->name ? strdup(symbol->name) : NULL;
    new_symbol->documentation = symbol->documentation ? strdup(symbol->documentation) : NULL;
    new_symbol->type_name = symbol->type_name ? strdup(symbol->type_name) : NULL;
    new_symbol->module = symbol->module ? strdup(symbol->module) : NULL;
}

Symbol* symbol_table_lookup(SymbolTable* table, const char* name) {
    if (!table || !name) return NULL;
    
    for (int i = 0; i < table->symbol_count; i++) {
        if (strcmp(table->symbols[i].name, name) == 0) {
            return &table->symbols[i];
        }
    }
    
    return NULL;
}

Symbol* symbol_table_lookup_global(SymbolTable* table, const char* name) {
    if (!table || !name) return NULL;
    
    Symbol* symbol = symbol_table_lookup(table, name);
    if (symbol) return symbol;
    
    if (table->parent) {
        return symbol_table_lookup_global(table->parent, name);
    }
    
    return NULL;
}

// Parser Implementation
Parser* parser_create(Token* tokens, int token_count) {
    Parser* parser = malloc(sizeof(Parser));
    if (!parser) return NULL;
    
    parser->tokens = tokens;
    parser->token_count = token_count;
    parser->current = 0;
    parser->has_errors = false;
    parser->errors = NULL;
    parser->error_count = 0;
    parser->root = NULL;
    
    return parser;
}

void parser_destroy(Parser* parser) {
    if (!parser) return;
    
    for (int i = 0; i < parser->error_count; i++) {
        free(parser->errors[i]);
    }
    free(parser->errors);
    
    ast_node_destroy(parser->root);
    free(parser);
}

static Token parser_current(Parser* parser) {
    if (parser->current >= parser->token_count) {
        Token eof = {TOKEN_EOF, "", {0, 0, 0, NULL}, {0, 0, 0, NULL}, 0};
        return eof;
    }
    return parser->tokens[parser->current];
}

static Token parser_peek(Parser* parser, int offset) {
    int pos = parser->current + offset;
    if (pos >= parser->token_count) {
        Token eof = {TOKEN_EOF, "", {0, 0, 0, NULL}, {0, 0, 0, NULL}, 0};
        return eof;
    }
    return parser->tokens[pos];
}

static bool parser_check(Parser* parser, TokenType type) {
    return parser_current(parser).type == type;
}

static bool parser_match(Parser* parser, TokenType type) {
    if (parser_check(parser, type)) {
        parser_advance(parser);
        return true;
    }
    return false;
}

static void parser_advance(Parser* parser) {
    if (parser->current < parser->token_count) {
        parser->current++;
    }
}

static void parser_add_error(Parser* parser, const char* message) {
    parser->has_errors = true;
    parser->errors = realloc(parser->errors, sizeof(char*) * (parser->error_count + 1));
    parser->errors[parser->error_count++] = strdup(message);
}

static bool parser_synchronize(Parser* parser) {
    // Skip tokens until we find a synchronization point
    while (!parser_check(parser, TOKEN_EOF)) {
        if (parser_check(parser, TOKEN_SEMICOLON) ||
            parser_check(parser, TOKEN_LEFT_BRACE) ||
            parser_check(parser, TOKEN_RIGHT_BRACE) ||
            parser_check(parser, TOKEN_FUN) ||
            parser_check(parser, TOKEN_STRUCT) ||
            parser_check(parser, TOKEN_CLASS) ||
            parser_check(parser, TOKEN_IF) ||
            parser_check(parser, TOKEN_WHILE) ||
            parser_check(parser, TOKEN_FOR) ||
            parser_check(parser, TOKEN_RETURN)) {
            return true;
        }
        parser_advance(parser);
    }
    return false;
}

// Parsing functions
static ASTNode* parse_identifier(Parser* parser) {
    Token token = parser_current(parser);
    if (!parser_match(parser, TOKEN_IDENTIFIER)) {
        parser_add_error(parser, "Expected identifier");
        return NULL;
    }
    
    Range range = {token.start, token.end};
    ASTNode* node = ast_node_create(AST_IDENTIFIER, range);
    node->data.identifier.name = strdup(token.value);
    return node;
}

static ASTNode* parse_literal(Parser* parser) {
    Token token = parser_current(parser);
    TokenType type = token.type;
    
    if (type == TOKEN_INTEGER_LITERAL || type == TOKEN_FLOAT_LITERAL ||
        type == TOKEN_STRING_LITERAL || type == TOKEN_CHAR_LITERAL ||
        type == TOKEN_BOOL_LITERAL || type == TOKEN_NULL_LITERAL) {
        
        parser_advance(parser);
        Range range = {token.start, token.end};
        ASTNode* node = ast_node_create(AST_LITERAL, range);
        node->data.literal.literal_type = type;
        node->data.literal.value = strdup(token.value);
        return node;
    }
    
    parser_add_error(parser, "Expected literal");
    return NULL;
}

static ASTNode* parse_type(Parser* parser) {
    ASTNode* base_type = NULL;
    
    // Parse base type
    if (parser_check(parser, TOKEN_IDENTIFIER)) {
        base_type = parse_identifier(parser);
    } else {
        // Built-in types
        Token token = parser_current(parser);
        if (parser_match(parser, TOKEN_INT) || parser_match(parser, TOKEN_UINT) ||
            parser_match(parser, TOKEN_LONG) || parser_match(parser, TOKEN_ULONG) ||
            parser_match(parser, TOKEN_FLOAT) || parser_match(parser, TOKEN_DOUBLE) ||
            parser_match(parser, TOKEN_BOOL) || parser_match(parser, TOKEN_CHAR) ||
            parser_match(parser, TOKEN_STRING_TYPE) || parser_match(parser, TOKEN_VOID)) {
            
            Range range = {token.start, token.end};
            base_type = ast_node_create(AST_IDENTIFIER, range);
            base_type->data.identifier.name = strdup(token.value);
        }
    }
    
    if (!base_type) {
        parser_add_error(parser, "Expected type");
        return NULL;
    }
    
    // Handle nullable types
    if (parser_match(parser, TOKEN_QUESTION)) {
        Range range = {base_type->range.start, parser_peek(parser, -1).end};
        ASTNode* nullable = ast_node_create(AST_NULLABLE_TYPE, range);
        nullable->data.nullable_type.base_type = base_type;
        base_type = nullable;
    }
    
    // Handle generic types
    if (parser_match(parser, TOKEN_LESS)) {
        ASTNode* type_params = ast_node_create(AST_PROGRAM, base_type->range); // Temporary container
        
        do {
            ASTNode* param = parse_type(parser);
            if (param) {
                ast_node_add_child(type_params, param);
            }
        } while (parser_match(parser, TOKEN_COMMA));
        
        if (!parser_match(parser, TOKEN_GREATER)) {
            parser_add_error(parser, "Expected '>' after generic type parameters");
        }
        
        Range range = {base_type->range.start, parser_peek(parser, -1).end};
        ASTNode* generic = ast_node_create(AST_GENERIC_TYPE, range);
        generic->data.generic_type.base_type = base_type;
        generic->data.generic_type.type_parameters = type_params;
        base_type = generic;
    }
    
    return base_type;
}

static ASTNode* parse_parameter(Parser* parser) {
    Token name_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_IDENTIFIER)) {
        parser_add_error(parser, "Expected parameter name");
        return NULL;
    }
    
    if (!parser_match(parser, TOKEN_COLON)) {
        parser_add_error(parser, "Expected ':' after parameter name");
        return NULL;
    }
    
    ASTNode* type = parse_type(parser);
    if (!type) return NULL;
    
    Range range = {name_token.start, type->range.end};
    ASTNode* param = ast_node_create(AST_PARAMETER, range);
    param->data.parameter.name = strdup(name_token.value);
    param->data.parameter.type = type;
    
    // Handle default values
    if (parser_match(parser, TOKEN_ASSIGN)) {
        param->data.parameter.default_value = parse_expression(parser);
    }
    
    return param;
}

static ASTNode* parse_parameter_list(Parser* parser) {
    if (!parser_match(parser, TOKEN_LEFT_PAREN)) {
        parser_add_error(parser, "Expected '('");
        return NULL;
    }
    
    Token start_token = parser_peek(parser, -1);
    ASTNode* params = ast_node_create(AST_PROGRAM, (Range){start_token.start, start_token.end});
    
    if (!parser_check(parser, TOKEN_RIGHT_PAREN)) {
        do {
            ASTNode* param = parse_parameter(parser);
            if (param) {
                ast_node_add_child(params, param);
            }
        } while (parser_match(parser, TOKEN_COMMA));
    }
    
    if (!parser_match(parser, TOKEN_RIGHT_PAREN)) {
        parser_add_error(parser, "Expected ')' after parameters");
    }
    
    return params;
}

static ASTNode* parse_block(Parser* parser) {
    Token start_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_LEFT_BRACE)) {
        parser_add_error(parser, "Expected '{'");
        return NULL;
    }
    
    ASTNode* block = ast_node_create(AST_BLOCK, (Range){start_token.start, start_token.end});
    
    while (!parser_check(parser, TOKEN_RIGHT_BRACE) && !parser_check(parser, TOKEN_EOF)) {
        ASTNode* stmt = parse_statement(parser);
        if (stmt) {
            ast_node_add_child(block, stmt);
        } else {
            parser_synchronize(parser);
        }
    }
    
    if (!parser_match(parser, TOKEN_RIGHT_BRACE)) {
        parser_add_error(parser, "Expected '}' after block");
    }
    
    block->range.end = parser_peek(parser, -1).end;
    return block;
}

static ASTNode* parse_function(Parser* parser) {
    Token start_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_FUN)) {
        parser_add_error(parser, "Expected 'fun'");
        return NULL;
    }
    
    // Handle method syntax: fun (receiver: Type) methodName()
    ASTNode* receiver = NULL;
    if (parser_check(parser, TOKEN_LEFT_PAREN)) {
        Token paren_token = parser_current(parser);
        parser_advance(parser); // Skip '('
        
        // Check if this is a receiver (parameter with type)
        if (parser_check(parser, TOKEN_IDENTIFIER)) {
            Token receiver_name = parser_current(parser);
            parser_advance(parser);
            
            if (parser_match(parser, TOKEN_COLON)) {
                ASTNode* receiver_type = parse_type(parser);
                if (receiver_type && parser_match(parser, TOKEN_RIGHT_PAREN)) {
                    // This is a method receiver
                    receiver = ast_node_create(AST_PARAMETER, (Range){receiver_name.start, receiver_type->range.end});
                    receiver->data.parameter.name = strdup(receiver_name.value);
                    receiver->data.parameter.type = receiver_type;
                } else {
                    parser_add_error(parser, "Invalid method receiver syntax");
                    return NULL;
                }
            } else {
                // Not a receiver, backtrack
                parser->current = paren_token.start.offset; // Crude backtrack
                parser_add_error(parser, "Invalid function syntax");
                return NULL;
            }
        } else {
            parser_add_error(parser, "Expected receiver parameter");
            return NULL;
        }
    }
    
    // Function name
    Token name_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_IDENTIFIER)) {
        parser_add_error(parser, "Expected function name");
        return NULL;
    }
    
    // Parameters
    ASTNode* params = parse_parameter_list(parser);
    if (!params) return NULL;
    
    // Return type
    ASTNode* return_type = NULL;
    if (parser_match(parser, TOKEN_ARROW)) {
        return_type = parse_type(parser);
    }
    
    // Body
    ASTNode* body = parse_block(parser);
    if (!body) return NULL;
    
    Range range = {start_token.start, body->range.end};
    ASTNode* function = ast_node_create(AST_FUNCTION, range);
    function->data.function.name = strdup(name_token.value);
    function->data.function.parameters = params;
    function->data.function.return_type = return_type;
    function->data.function.body = body;
    function->data.function.is_public = isupper(name_token.value[0]);
    function->data.function.is_async = false; // TODO: Handle async
    
    // Add receiver as first parameter if it's a method
    if (receiver) {
        // Insert receiver at the beginning of parameters
        ast_node_add_child(params, NULL); // Make space
        for (int i = params->child_count - 1; i > 0; i--) {
            params->children[i] = params->children[i-1];
        }
        params->children[0] = receiver;
    }
    
    return function;
}

static ASTNode* parse_variable_declaration(Parser* parser) {
    Token start_token = parser_current(parser);
    bool is_mutable = false;
    
    if (parser_match(parser, TOKEN_VAR)) {
        is_mutable = true;
    } else if (!parser_match(parser, TOKEN_LET)) {
        parser_add_error(parser, "Expected 'let' or 'var'");
        return NULL;
    }
    
    Token name_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_IDENTIFIER)) {
        parser_add_error(parser, "Expected variable name");
        return NULL;
    }
    
    ASTNode* type = NULL;
    if (parser_match(parser, TOKEN_COLON)) {
        type = parse_type(parser);
    }
    
    ASTNode* initializer = NULL;
    if (parser_match(parser, TOKEN_ASSIGN)) {
        initializer = parse_expression(parser);
    }
    
    Range range = {start_token.start, parser_peek(parser, -1).end};
    ASTNode* var_decl = ast_node_create(AST_VARIABLE_DECLARATION, range);
    var_decl->data.variable.name = strdup(name_token.value);
    var_decl->data.variable.type = type;
    var_decl->data.variable.initializer = initializer;
    var_decl->data.variable.is_mutable = is_mutable;
    var_decl->data.variable.is_public = isupper(name_token.value[0]);
    
    return var_decl;
}

// Forward declarations for expression parsing
static ASTNode* parse_expression(Parser* parser);
static ASTNode* parse_assignment(Parser* parser);
static ASTNode* parse_logical_or(Parser* parser);
static ASTNode* parse_logical_and(Parser* parser);
static ASTNode* parse_equality(Parser* parser);
static ASTNode* parse_comparison(Parser* parser);
static ASTNode* parse_addition(Parser* parser);
static ASTNode* parse_multiplication(Parser* parser);
static ASTNode* parse_unary(Parser* parser);
static ASTNode* parse_postfix(Parser* parser);
static ASTNode* parse_primary(Parser* parser);

static ASTNode* parse_primary(Parser* parser) {
    if (parser_check(parser, TOKEN_IDENTIFIER)) {
        return parse_identifier(parser);
    }
    
    if (parser_check(parser, TOKEN_INTEGER_LITERAL) ||
        parser_check(parser, TOKEN_FLOAT_LITERAL) ||
        parser_check(parser, TOKEN_STRING_LITERAL) ||
        parser_check(parser, TOKEN_CHAR_LITERAL) ||
        parser_check(parser, TOKEN_BOOL_LITERAL) ||
        parser_check(parser, TOKEN_NULL_LITERAL)) {
        return parse_literal(parser);
    }
    
    if (parser_match(parser, TOKEN_LEFT_PAREN)) {
        ASTNode* expr = parse_expression(parser);
        if (!parser_match(parser, TOKEN_RIGHT_PAREN)) {
            parser_add_error(parser, "Expected ')' after expression");
        }
        return expr;
    }
    
    parser_add_error(parser, "Expected expression");
    return NULL;
}

static ASTNode* parse_unary(Parser* parser) {
    Token op_token = parser_current(parser);
    
    if (parser_match(parser, TOKEN_NOT) || parser_match(parser, TOKEN_MINUS) ||
        parser_match(parser, TOKEN_PLUS) || parser_match(parser, TOKEN_EXCLAMATION)) {
        
        ASTNode* operand = parse_unary(parser);
        if (!operand) return NULL;
        
        Range range = {op_token.start, operand->range.end};
        ASTNode* unary = ast_node_create(AST_UNARY_EXPRESSION, range);
        unary->data.unary_expr.operator = op_token.type;
        unary->data.unary_expr.operand = operand;
        return unary;
    }
    
    return parse_postfix(parser);
}

static ASTNode* parse_postfix(Parser* parser) {
    ASTNode* expr = parse_primary(parser);
    if (!expr) return NULL;
    
    while (true) {
        if (parser_match(parser, TOKEN_DOT) || parser_match(parser, TOKEN_SAFE_NAV)) {
            bool is_safe = parser_peek(parser, -1).type == TOKEN_SAFE_NAV;
            
            Token member_token = parser_current(parser);
            if (!parser_match(parser, TOKEN_IDENTIFIER)) {
                parser_add_error(parser, "Expected member name after '.'");
                break;
            }
            
            Range range = {expr->range.start, member_token.end};
            ASTNode* member = ast_node_create(AST_MEMBER_ACCESS, range);
            member->data.member_access.object = expr;
            member->data.member_access.member = strdup(member_token.value);
            member->data.member_access.is_safe_navigation = is_safe;
            expr = member;
        } else if (parser_match(parser, TOKEN_LEFT_BRACKET)) {
            ASTNode* index = parse_expression(parser);
            if (!parser_match(parser, TOKEN_RIGHT_BRACKET)) {
                parser_add_error(parser, "Expected ']' after index");
            }
            
            Range range = {expr->range.start, parser_peek(parser, -1).end};
            ASTNode* index_access = ast_node_create(AST_INDEX_ACCESS, range);
            index_access->data.index_access.object = expr;
            index_access->data.index_access.index = index;
            expr = index_access;
        } else if (parser_match(parser, TOKEN_LEFT_PAREN)) {
            // Function call
            ASTNode* args = ast_node_create(AST_PROGRAM, expr->range); // Temporary container
            
            if (!parser_check(parser, TOKEN_RIGHT_PAREN)) {
                do {
                    ASTNode* arg = parse_expression(parser);
                    if (arg) {
                        ast_node_add_child(args, arg);
                    }
                } while (parser_match(parser, TOKEN_COMMA));
            }
            
            if (!parser_match(parser, TOKEN_RIGHT_PAREN)) {
                parser_add_error(parser, "Expected ')' after arguments");
            }
            
            Range range = {expr->range.start, parser_peek(parser, -1).end};
            ASTNode* call = ast_node_create(AST_CALL_EXPRESSION, range);
            call->data.call_expr.function = expr;
            call->data.call_expr.arguments = args;
            expr = call;
        } else {
            break;
        }
    }
    
    return expr;
}

static ASTNode* parse_multiplication(Parser* parser) {
    ASTNode* expr = parse_unary(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_MULTIPLY) || parser_check(parser, TOKEN_DIVIDE) ||
           parser_check(parser, TOKEN_MODULO) || parser_check(parser, TOKEN_POWER)) {
        
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_unary(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_addition(Parser* parser) {
    ASTNode* expr = parse_multiplication(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_PLUS) || parser_check(parser, TOKEN_MINUS)) {
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_multiplication(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_comparison(Parser* parser) {
    ASTNode* expr = parse_addition(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_GREATER) || parser_check(parser, TOKEN_GREATER_EQUAL) ||
           parser_check(parser, TOKEN_LESS) || parser_check(parser, TOKEN_LESS_EQUAL)) {
        
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_addition(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_equality(Parser* parser) {
    ASTNode* expr = parse_comparison(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_EQUAL) || parser_check(parser, TOKEN_NOT_EQUAL)) {
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_comparison(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_logical_and(Parser* parser) {
    ASTNode* expr = parse_equality(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_AND)) {
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_equality(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_logical_or(Parser* parser) {
    ASTNode* expr = parse_logical_and(parser);
    if (!expr) return NULL;
    
    while (parser_check(parser, TOKEN_OR)) {
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_logical_and(parser);
        if (!right) break;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* binary = ast_node_create(AST_BINARY_EXPRESSION, range);
        binary->data.binary_expr.left = expr;
        binary->data.binary_expr.operator = op_token.type;
        binary->data.binary_expr.right = right;
        expr = binary;
    }
    
    return expr;
}

static ASTNode* parse_assignment(Parser* parser) {
    ASTNode* expr = parse_logical_or(parser);
    if (!expr) return NULL;
    
    if (parser_check(parser, TOKEN_ASSIGN) || parser_check(parser, TOKEN_PLUS_ASSIGN) ||
        parser_check(parser, TOKEN_MINUS_ASSIGN) || parser_check(parser, TOKEN_MULTIPLY_ASSIGN) ||
        parser_check(parser, TOKEN_DIVIDE_ASSIGN) || parser_check(parser, TOKEN_MODULO_ASSIGN)) {
        
        Token op_token = parser_current(parser);
        parser_advance(parser);
        
        ASTNode* right = parse_assignment(parser);
        if (!right) return expr;
        
        Range range = {expr->range.start, right->range.end};
        ASTNode* assignment = ast_node_create(AST_ASSIGNMENT, range);
        assignment->data.binary_expr.left = expr;
        assignment->data.binary_expr.operator = op_token.type;
        assignment->data.binary_expr.right = right;
        return assignment;
    }
    
    return expr;
}

static ASTNode* parse_expression(Parser* parser) {
    return parse_assignment(parser);
}

static ASTNode* parse_return_statement(Parser* parser) {
    Token start_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_RETURN)) {
        parser_add_error(parser, "Expected 'return'");
        return NULL;
    }
    
    ASTNode* expr = NULL;
    if (!parser_check(parser, TOKEN_SEMICOLON) && !parser_check(parser, TOKEN_NEWLINE) &&
        !parser_check(parser, TOKEN_RIGHT_BRACE)) {
        expr = parse_expression(parser);
    }
    
    Range range = {start_token.start, expr ? expr->range.end : start_token.end};
    ASTNode* return_stmt = ast_node_create(AST_RETURN_STATEMENT, range);
    return_stmt->data.return_stmt.expression = expr;
    
    return return_stmt;
}

static ASTNode* parse_if_statement(Parser* parser) {
    Token start_token = parser_current(parser);
    if (!parser_match(parser, TOKEN_IF)) {
        parser_add_error(parser, "Expected 'if'");
        return NULL;
    }
    
    ASTNode* condition = parse_expression(parser);
    if (!condition) return NULL;
    
    ASTNode* then_block = parse_block(parser);
    if (!then_block) return NULL;
    
    ASTNode* else_block = NULL;
    if (parser_match(parser, TOKEN_ELSE)) {
        if (parser_check(parser, TOKEN_IF)) {
            else_block = parse_if_statement(parser);
        } else {
            else_block = parse_block(parser);
        }
    }
    
    Range range = {start_token.start, else_block ? else_block->range.end : then_block->range.end};
    ASTNode* if_stmt = ast_node_create(AST_IF_STATEMENT, range);
    if_stmt->data.if_stmt.condition = condition;
    if_stmt->data.if_stmt.then_block = then_block;
    if_stmt->data.if_stmt.else_block = else_block;
    
    return if_stmt;
}

static ASTNode* parse_statement(Parser* parser) {
    // Skip whitespace and comments
    while (parser_check(parser, TOKEN_WHITESPACE) || parser_check(parser, TOKEN_NEWLINE) ||
           parser_check(parser, TOKEN_COMMENT)) {
        parser_advance(parser);
    }
    
    if (parser_check(parser, TOKEN_LET) || parser_check(parser, TOKEN_VAR)) {
        return parse_variable_declaration(parser);
    }
    
    if (parser_check(parser, TOKEN_FUN)) {
        return parse_function(parser);
    }
    
    if (parser_check(parser, TOKEN_IF)) {
        return parse_if_statement(parser);
    }
    
    if (parser_check(parser, TOKEN_RETURN)) {
        return parse_return_statement(parser);
    }
    
    if (parser_check(parser, TOKEN_LEFT_BRACE)) {
        return parse_block(parser);
    }
    
    // Expression statement
    ASTNode* expr = parse_expression(parser);
    if (!expr) return NULL;
    
    ASTNode* stmt = ast_node_create(AST_EXPRESSION_STATEMENT, expr->range);
    ast_node_add_child(stmt, expr);
    
    return stmt;
}

static ASTNode* parse_program(Parser* parser) {
    ASTNode* program = ast_node_create(AST_PROGRAM, (Range){{0, 0, 0, NULL}, {0, 0, 0, NULL}});
    
    while (!parser_check(parser, TOKEN_EOF)) {
        // Skip whitespace and comments at top level
        while (parser_check(parser, TOKEN_WHITESPACE) || parser_check(parser, TOKEN_NEWLINE) ||
               parser_check(parser, TOKEN_COMMENT) || parser_check(parser, TOKEN_DOC_COMMENT)) {
            
            // Capture documentation comments
            if (parser_check(parser, TOKEN_DOC_COMMENT)) {
                Token doc_token = parser_current(parser);
                parser_advance(parser);
                
                // Associate with next declaration
                ASTNode* next_stmt = parse_statement(parser);
                if (next_stmt) {
                    ast_node_set_documentation(next_stmt, doc_token.value);
                    ast_node_add_child(program, next_stmt);
                }
                continue;
            }
            
            parser_advance(parser);
        }
        
        if (parser_check(parser, TOKEN_EOF)) break;
        
        ASTNode* stmt = parse_statement(parser);
        if (stmt) {
            ast_node_add_child(program, stmt);
        } else {
            if (!parser_synchronize(parser)) break;
        }
    }
    
    return program;
}

bool parser_parse(Parser* parser) {
    if (!parser) return false;
    
    parser->root = parse_program(parser);
    return parser->root != NULL && !parser->has_errors;
}

ASTNode* parser_get_ast(Parser* parser) {
    return parser ? parser->root : NULL;
}

char** parser_get_errors(Parser* parser, int* count) {
    if (!parser || !count) return NULL;
    *count = parser->error_count;
    return parser->errors;
}