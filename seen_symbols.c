#include "seen_parser.h"
#include <string.h>
#include <stdio.h>

// Symbol table building from AST
static void build_symbols_recursive(SymbolTable* table, ASTNode* node, const char* module_name) {
    if (!table || !node) return;
    
    Symbol symbol = {0};
    bool add_symbol = false;
    
    switch (node->type) {
        case AST_FUNCTION:
            symbol.name = strdup(node->data.function.name);
            symbol.type = AST_FUNCTION;
            symbol.range = node->range;
            symbol.definition = node;
            symbol.documentation = node->data.function.documentation ? 
                strdup(node->data.function.documentation) : NULL;
            symbol.is_public = node->data.function.is_public;
            symbol.module = module_name ? strdup(module_name) : NULL;
            
            // Determine return type
            if (node->data.function.return_type) {
                if (node->data.function.return_type->type == AST_IDENTIFIER) {
                    symbol.type_name = strdup(node->data.function.return_type->data.identifier.name);
                } else {
                    symbol.type_name = strdup("Complex Type");
                }
            } else {
                symbol.type_name = strdup("Void");
            }
            
            add_symbol = true;
            break;
            
        case AST_VARIABLE_DECLARATION:
            symbol.name = strdup(node->data.variable.name);
            symbol.type = AST_VARIABLE_DECLARATION;
            symbol.range = node->range;
            symbol.definition = node;
            symbol.documentation = node->data.variable.documentation ? 
                strdup(node->data.variable.documentation) : NULL;
            symbol.is_public = node->data.variable.is_public;
            symbol.module = module_name ? strdup(module_name) : NULL;
            
            // Determine variable type
            if (node->data.variable.type) {
                if (node->data.variable.type->type == AST_IDENTIFIER) {
                    symbol.type_name = strdup(node->data.variable.type->data.identifier.name);
                } else if (node->data.variable.type->type == AST_NULLABLE_TYPE) {
                    ASTNode* base = node->data.variable.type->data.nullable_type.base_type;
                    if (base && base->type == AST_IDENTIFIER) {
                        char* nullable_type = malloc(strlen(base->data.identifier.name) + 2);
                        sprintf(nullable_type, "%s?", base->data.identifier.name);
                        symbol.type_name = nullable_type;
                    } else {
                        symbol.type_name = strdup("Unknown?");
                    }
                } else {
                    symbol.type_name = strdup("Inferred");
                }
            } else {
                symbol.type_name = strdup("Inferred");
            }
            
            add_symbol = true;
            break;
            
        case AST_CONSTANT_DECLARATION:
            symbol.name = strdup(node->data.constant.name);
            symbol.type = AST_CONSTANT_DECLARATION;
            symbol.range = node->range;
            symbol.definition = node;
            symbol.documentation = node->data.constant.documentation ? 
                strdup(node->data.constant.documentation) : NULL;
            symbol.is_public = node->data.constant.is_public;
            symbol.module = module_name ? strdup(module_name) : NULL;
            symbol.type_name = strdup("Const");
            add_symbol = true;
            break;
            
        case AST_STRUCT:
            symbol.name = strdup(node->data.struct_def.name);
            symbol.type = AST_STRUCT;
            symbol.range = node->range;
            symbol.definition = node;
            symbol.documentation = node->data.struct_def.documentation ? 
                strdup(node->data.struct_def.documentation) : NULL;
            symbol.is_public = node->data.struct_def.is_public;
            symbol.module = module_name ? strdup(module_name) : NULL;
            symbol.type_name = strdup("struct");
            add_symbol = true;
            break;
            
        case AST_PARAMETER:
            symbol.name = strdup(node->data.parameter.name);
            symbol.type = AST_PARAMETER;
            symbol.range = node->range;
            symbol.definition = node;
            symbol.is_public = false; // Parameters are local
            symbol.module = module_name ? strdup(module_name) : NULL;
            
            if (node->data.parameter.type && node->data.parameter.type->type == AST_IDENTIFIER) {
                symbol.type_name = strdup(node->data.parameter.type->data.identifier.name);
            } else {
                symbol.type_name = strdup("Unknown");
            }
            
            add_symbol = true;
            break;
            
        default:
            break;
    }
    
    if (add_symbol) {
        symbol_table_add_symbol(table, &symbol);
        
        // Free temporary allocations since symbol_table_add_symbol duplicates them
        free(symbol.name);
        free(symbol.documentation);
        free(symbol.type_name);
        free(symbol.module);
    }
    
    // Recursively process children
    for (int i = 0; i < node->child_count; i++) {
        build_symbols_recursive(table, node->children[i], module_name);
    }
    
    // Process node-specific children
    switch (node->type) {
        case AST_FUNCTION:
            if (node->data.function.parameters) {
                build_symbols_recursive(table, node->data.function.parameters, module_name);
            }
            if (node->data.function.body) {
                build_symbols_recursive(table, node->data.function.body, module_name);
            }
            break;
            
        case AST_VARIABLE_DECLARATION:
            if (node->data.variable.initializer) {
                build_symbols_recursive(table, node->data.variable.initializer, module_name);
            }
            break;
            
        case AST_IF_STATEMENT:
            if (node->data.if_stmt.condition) {
                build_symbols_recursive(table, node->data.if_stmt.condition, module_name);
            }
            if (node->data.if_stmt.then_block) {
                build_symbols_recursive(table, node->data.if_stmt.then_block, module_name);
            }
            if (node->data.if_stmt.else_block) {
                build_symbols_recursive(table, node->data.if_stmt.else_block, module_name);
            }
            break;
            
        case AST_BINARY_EXPRESSION:
            if (node->data.binary_expr.left) {
                build_symbols_recursive(table, node->data.binary_expr.left, module_name);
            }
            if (node->data.binary_expr.right) {
                build_symbols_recursive(table, node->data.binary_expr.right, module_name);
            }
            break;
            
        case AST_CALL_EXPRESSION:
            if (node->data.call_expr.function) {
                build_symbols_recursive(table, node->data.call_expr.function, module_name);
            }
            if (node->data.call_expr.arguments) {
                build_symbols_recursive(table, node->data.call_expr.arguments, module_name);
            }
            break;
            
        case AST_MEMBER_ACCESS:
            if (node->data.member_access.object) {
                build_symbols_recursive(table, node->data.member_access.object, module_name);
            }
            break;
            
        default:
            break;
    }
}

void symbol_table_build_from_ast(SymbolTable* table, ASTNode* root) {
    if (!table || !root) return;
    
    const char* module_name = table->scope_name ? table->scope_name : "main";
    build_symbols_recursive(table, root, module_name);
}

// LSP helper functions
Symbol* find_symbol_at_position(SymbolTable* table, Position position) {
    if (!table) return NULL;
    
    for (int i = 0; i < table->symbol_count; i++) {
        Symbol* symbol = &table->symbols[i];
        
        // Check if position is within symbol's range
        if (position.line >= symbol->range.start.line && 
            position.line <= symbol->range.end.line) {
            
            if (position.line == symbol->range.start.line && 
                position.column < symbol->range.start.column) {
                continue;
            }
            
            if (position.line == symbol->range.end.line && 
                position.column > symbol->range.end.column) {
                continue;
            }
            
            return symbol;
        }
    }
    
    // Check parent scope
    if (table->parent) {
        return find_symbol_at_position(table->parent, position);
    }
    
    return NULL;
}

Symbol** find_references(SymbolTable* table, const char* name, int* count) {
    if (!table || !name || !count) return NULL;
    
    *count = 0;
    Symbol** references = NULL;
    int capacity = 0;
    
    // Helper function to add reference
    void add_reference(Symbol* symbol) {
        if (*count >= capacity) {
            capacity = capacity ? capacity * 2 : 8;
            references = realloc(references, sizeof(Symbol*) * capacity);
        }
        references[(*count)++] = symbol;
    }
    
    // Find references in current scope
    for (int i = 0; i < table->symbol_count; i++) {
        if (strcmp(table->symbols[i].name, name) == 0) {
            add_reference(&table->symbols[i]);
        }
    }
    
    // TODO: Add more sophisticated reference finding by walking the AST
    // This would find all identifier nodes that reference the symbol
    
    return references;
}

Symbol** get_completions(SymbolTable* table, Position position, int* count) {
    if (!table || !count) return NULL;
    
    *count = 0;
    Symbol** completions = NULL;
    int capacity = 0;
    
    // Helper function to add completion
    void add_completion(Symbol* symbol) {
        if (*count >= capacity) {
            capacity = capacity ? capacity * 2 : 16;
            completions = realloc(completions, sizeof(Symbol*) * capacity);
        }
        completions[(*count)++] = symbol;
    }
    
    // Add all visible symbols as completions
    SymbolTable* current = table;
    while (current) {
        for (int i = 0; i < current->symbol_count; i++) {
            Symbol* symbol = &current->symbols[i];
            
            // Only include public symbols from other modules, or all symbols from current module
            if (current == table || symbol->is_public) {
                add_completion(symbol);
            }
        }
        current = current->parent;
    }
    
    // Add built-in types and functions
    static Symbol builtin_symbols[] = {
        {"Int", AST_IDENTIFIER, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Built-in integer type", "type", true, "builtin"},
        {"String", AST_IDENTIFIER, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Built-in string type", "type", true, "builtin"},
        {"Bool", AST_IDENTIFIER, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Built-in boolean type", "type", true, "builtin"},
        {"Float", AST_IDENTIFIER, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Built-in floating point type", "type", true, "builtin"},
        {"print", AST_FUNCTION, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Print to standard output", "Void", true, "builtin"},
        {"println", AST_FUNCTION, {{0,0,0,NULL},{0,0,0,NULL}}, NULL, "Print line to standard output", "Void", true, "builtin"},
    };
    
    for (int i = 0; i < sizeof(builtin_symbols) / sizeof(builtin_symbols[0]); i++) {
        add_completion(&builtin_symbols[i]);
    }
    
    return completions;
}

char* get_hover_info(Symbol* symbol) {
    if (!symbol) return NULL;
    
    char* info = malloc(1024);
    info[0] = '\0';
    
    // Format symbol information
    switch (symbol->type) {
        case AST_FUNCTION:
            snprintf(info, 1024, "**%s** %s\n\n```seen\nfun %s(): %s\n```\n\n%s",
                symbol->is_public ? "Public Function" : "Private Function",
                symbol->name,
                symbol->name,
                symbol->type_name ? symbol->type_name : "Void",
                symbol->documentation ? symbol->documentation : "No documentation available.");
            break;
            
        case AST_VARIABLE_DECLARATION:
            snprintf(info, 1024, "**%s Variable** %s\n\n```seen\nlet %s: %s\n```\n\n%s",
                symbol->is_public ? "Public" : "Private",
                symbol->name,
                symbol->name,
                symbol->type_name ? symbol->type_name : "Unknown",
                symbol->documentation ? symbol->documentation : "No documentation available.");
            break;
            
        case AST_CONSTANT_DECLARATION:
            snprintf(info, 1024, "**%s Constant** %s\n\n```seen\nconst %s\n```\n\n%s",
                symbol->is_public ? "Public" : "Private",
                symbol->name,
                symbol->name,
                symbol->documentation ? symbol->documentation : "No documentation available.");
            break;
            
        case AST_STRUCT:
            snprintf(info, 1024, "**%s Struct** %s\n\n```seen\nstruct %s\n```\n\n%s",
                symbol->is_public ? "Public" : "Private",
                symbol->name,
                symbol->name,
                symbol->documentation ? symbol->documentation : "No documentation available.");
            break;
            
        case AST_PARAMETER:
            snprintf(info, 1024, "**Parameter** %s\n\n```seen\n%s: %s\n```",
                symbol->name,
                symbol->name,
                symbol->type_name ? symbol->type_name : "Unknown");
            break;
            
        default:
            snprintf(info, 1024, "**%s**\n\nType: %s",
                symbol->name,
                symbol->type_name ? symbol->type_name : "Unknown");
            break;
    }
    
    return info;
}

// Helper function to find symbol definition by name
Symbol* find_symbol_definition(SymbolTable* table, const char* name) {
    if (!table || !name) return NULL;
    
    // Look in current scope first
    Symbol* symbol = symbol_table_lookup(table, name);
    if (symbol) return symbol;
    
    // Look in parent scopes
    return symbol_table_lookup_global(table, name);
}

// Helper function to check if a position is within a range
bool position_in_range(Position position, Range range) {
    if (position.line < range.start.line || position.line > range.end.line) {
        return false;
    }
    
    if (position.line == range.start.line && position.column < range.start.column) {
        return false;
    }
    
    if (position.line == range.end.line && position.column > range.end.column) {
        return false;
    }
    
    return true;
}

// Find identifier at specific position for go-to-definition
ASTNode* find_identifier_at_position(ASTNode* node, Position position) {
    if (!node) return NULL;
    
    // Check if this node contains the position
    if (!position_in_range(position, node->range)) {
        return NULL;
    }
    
    // If this is an identifier at the exact position, return it
    if (node->type == AST_IDENTIFIER && position_in_range(position, node->range)) {
        return node;
    }
    
    // Check member access
    if (node->type == AST_MEMBER_ACCESS) {
        // Check if position is on the member name
        int member_start_col = node->range.end.column - strlen(node->data.member_access.member);
        Position member_start = {node->range.end.line, member_start_col, 0, node->range.end.filename};
        Range member_range = {member_start, node->range.end};
        
        if (position_in_range(position, member_range)) {
            return node; // Return the member access node
        }
        
        // Check the object part
        return find_identifier_at_position(node->data.member_access.object, position);
    }
    
    // Recursively check children
    for (int i = 0; i < node->child_count; i++) {
        ASTNode* result = find_identifier_at_position(node->children[i], position);
        if (result) return result;
    }
    
    // Check node-specific children
    switch (node->type) {
        case AST_BINARY_EXPRESSION:
            {
                ASTNode* result = find_identifier_at_position(node->data.binary_expr.left, position);
                if (result) return result;
                return find_identifier_at_position(node->data.binary_expr.right, position);
            }
        case AST_CALL_EXPRESSION:
            {
                ASTNode* result = find_identifier_at_position(node->data.call_expr.function, position);
                if (result) return result;
                return find_identifier_at_position(node->data.call_expr.arguments, position);
            }
        case AST_UNARY_EXPRESSION:
            return find_identifier_at_position(node->data.unary_expr.operand, position);
        default:
            break;
    }
    
    return NULL;
}