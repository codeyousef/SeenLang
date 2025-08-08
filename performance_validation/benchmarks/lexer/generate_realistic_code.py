#!/usr/bin/env python3
"""
Generate realistic code samples for lexer benchmarking.

This creates actual code that resembles real-world projects, not synthetic
patterns that could be optimized unfairly.
"""

import argparse
import random
import sys
from pathlib import Path
from typing import List, Dict, Any

class RealisticCodeGenerator:
    """Generates realistic code samples for performance testing."""
    
    def __init__(self, language: str = "seen"):
        self.language = language.lower()
        self.setup_language_specifics()
        
    def setup_language_specifics(self):
        """Configure language-specific keywords and patterns."""
        
        if self.language == "seen":
            self.keywords = [
                "func", "let", "mut", "if", "else", "while", "for", "loop",
                "return", "break", "continue", "struct", "enum", "impl",
                "trait", "pub", "priv", "mod", "use", "import", "export",
                "match", "when", "try", "catch", "finally", "async", "await",
                "const", "static", "type", "interface", "class", "extends",
                "implements", "abstract", "override", "virtual", "final"
            ]
            
            self.types = [
                "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
                "f32", "f64", "bool", "char", "str", "String", "Vec",
                "HashMap", "HashSet", "Option", "Result", "Box", "Rc", "Arc"
            ]
            
            self.operators = [
                "+", "-", "*", "/", "%", "=", "==", "!=", "<", "<=", ">", ">=",
                "&&", "||", "!", "&", "|", "^", "<<", ">>", "+=", "-=", "*=",
                "/=", "%=", "&=", "|=", "^=", "<<=", ">>="
            ]
            
            self.common_names = [
                "data", "result", "value", "item", "element", "node", "list",
                "map", "set", "queue", "stack", "tree", "graph", "buffer",
                "stream", "reader", "writer", "parser", "lexer", "token",
                "ast", "context", "state", "config", "options", "params"
            ]
    
    def generate_identifier(self) -> str:
        """Generate realistic identifier names."""
        prefixes = ["", "get_", "set_", "is_", "has_", "make_", "create_", "build_"]
        base_names = self.common_names + [f"var_{i}" for i in range(10)]
        suffixes = ["", "_impl", "_data", "_ptr", "_ref", "_mut", "_const"]
        
        prefix = random.choice(prefixes)
        base = random.choice(base_names)
        suffix = random.choice(suffixes) if random.random() < 0.3 else ""
        
        return f"{prefix}{base}{suffix}"
    
    def generate_string_literal(self) -> str:
        """Generate realistic string literals."""
        strings = [
            "Hello, World!", "Error: operation failed", "Warning: deprecated",
            "File not found", "Invalid input", "Success", "Loading...",
            "Processing data", "Initializing system", "Connecting to server",
            "Parsing configuration", "Building AST", "Optimizing code",
            "Generating output", "Cleaning up resources", "Done"
        ]
        
        # Sometimes generate longer strings with interpolation
        if random.random() < 0.2:
            return f'"Processing {{{self.generate_identifier()}}} with {{{random.randint(1, 1000)}}} items"'
        
        return f'"{random.choice(strings)}"'
    
    def generate_numeric_literal(self) -> str:
        """Generate realistic numeric literals."""
        if random.random() < 0.7:
            # Integer
            if random.random() < 0.3:
                return str(random.randint(0, 10))  # Small numbers
            elif random.random() < 0.6:
                return str(random.randint(0, 1000))  # Medium numbers
            else:
                return str(random.randint(0, 1000000))  # Large numbers
        else:
            # Float
            return f"{random.uniform(0, 1000):.2f}"
    
    def generate_comment(self) -> str:
        """Generate realistic comments."""
        single_line_comments = [
            "// TODO: Implement error handling",
            "// FIXME: This is a temporary workaround", 
            "// NOTE: This function assumes input is valid",
            "// WARNING: Performance critical section",
            "// Initialize the data structure",
            "// Clean up resources before exit",
            "// Check if the operation succeeded",
            "// Convert to the expected format",
            "// Validate input parameters",
            "// Update the internal state"
        ]
        
        multi_line_comments = [
            """/* 
 * This function implements the core algorithm
 * for processing the input data efficiently.
 * 
 * Parameters:
 *   data - the input data to process
 *   options - configuration options
 * 
 * Returns:
 *   The processed result or error
 */""",
            """/*
 * Performance critical section:
 * This code is optimized for speed and should not
 * be modified without careful benchmarking.
 */""",
            """/*
 * Memory management note:
 * All allocations in this function are tracked
 * and will be cleaned up automatically.
 */"""
        ]
        
        if random.random() < 0.8:
            return random.choice(single_line_comments)
        else:
            return random.choice(multi_line_comments)
    
    def generate_function(self, complexity: str = "medium") -> str:
        """Generate a realistic function."""
        func_name = self.generate_identifier()
        
        # Generate parameters
        param_count = {"small": 1, "medium": 3, "large": 5}[complexity]
        params = []
        for _ in range(random.randint(0, param_count)):
            param_name = self.generate_identifier()
            param_type = random.choice(self.types)
            params.append(f"{param_name}: {param_type}")
        
        param_str = ", ".join(params)
        return_type = random.choice(self.types + [""])
        return_annotation = f" -> {return_type}" if return_type else ""
        
        # Generate function body
        body_lines = []
        stmt_count = {"small": 5, "medium": 15, "large": 30}[complexity]
        
        for _ in range(random.randint(3, stmt_count)):
            body_lines.append("    " + self.generate_statement())
        
        if return_type:
            body_lines.append(f"    return {self.generate_expression()};")
        
        function_code = f"""func {func_name}({param_str}){return_annotation} {{
{chr(10).join(body_lines)}
}}"""
        
        return function_code
    
    def generate_statement(self) -> str:
        """Generate a realistic statement."""
        statement_types = [
            "variable_declaration",
            "assignment", 
            "if_statement",
            "for_loop",
            "while_loop",
            "function_call",
            "return_statement"
        ]
        
        stmt_type = random.choice(statement_types)
        
        if stmt_type == "variable_declaration":
            var_name = self.generate_identifier()
            var_type = random.choice(self.types)
            value = self.generate_expression()
            mutability = "mut " if random.random() < 0.3 else ""
            return f"let {mutability}{var_name}: {var_type} = {value};"
            
        elif stmt_type == "assignment":
            var_name = self.generate_identifier()
            op = random.choice(["=", "+=", "-=", "*=", "/="])
            value = self.generate_expression()
            return f"{var_name} {op} {value};"
            
        elif stmt_type == "if_statement":
            condition = self.generate_boolean_expression()
            then_stmt = self.generate_statement()
            if random.random() < 0.4:
                else_stmt = self.generate_statement()
                return f"if {condition} {{\n        {then_stmt}\n    }} else {{\n        {else_stmt}\n    }}"
            else:
                return f"if {condition} {{\n        {then_stmt}\n    }}"
                
        elif stmt_type == "for_loop":
            var_name = self.generate_identifier()
            iterable = self.generate_identifier()
            body_stmt = self.generate_statement()
            return f"for {var_name} in {iterable} {{\n        {body_stmt}\n    }}"
            
        elif stmt_type == "while_loop":
            condition = self.generate_boolean_expression()
            body_stmt = self.generate_statement()
            return f"while {condition} {{\n        {body_stmt}\n    }}"
            
        elif stmt_type == "function_call":
            func_name = self.generate_identifier()
            arg_count = random.randint(0, 4)
            args = [self.generate_expression() for _ in range(arg_count)]
            return f"{func_name}({', '.join(args)});"
            
        else:  # return_statement
            if random.random() < 0.3:
                return "return;"
            else:
                return f"return {self.generate_expression()};"
    
    def generate_expression(self) -> str:
        """Generate a realistic expression."""
        expr_types = ["literal", "identifier", "binary_op", "function_call"]
        expr_type = random.choice(expr_types)
        
        if expr_type == "literal":
            literal_types = ["number", "string", "boolean"]
            lit_type = random.choice(literal_types)
            
            if lit_type == "number":
                return self.generate_numeric_literal()
            elif lit_type == "string":
                return self.generate_string_literal()
            else:
                return random.choice(["true", "false"])
                
        elif expr_type == "identifier":
            return self.generate_identifier()
            
        elif expr_type == "binary_op":
            left = self.generate_identifier()
            op = random.choice(self.operators[:15])  # Avoid assignment ops
            right = self.generate_identifier()
            return f"{left} {op} {right}"
            
        else:  # function_call
            func_name = self.generate_identifier()
            arg_count = random.randint(0, 3)
            args = [self.generate_identifier() for _ in range(arg_count)]
            return f"{func_name}({', '.join(args)})"
    
    def generate_boolean_expression(self) -> str:
        """Generate a realistic boolean expression."""
        left = self.generate_identifier()
        op = random.choice(["==", "!=", "<", "<=", ">", ">="])
        right = self.generate_expression()
        
        if random.random() < 0.3:
            logical_op = random.choice(["&&", "||"])
            additional = self.generate_boolean_expression()
            return f"{left} {op} {right} {logical_op} {additional}"
        
        return f"{left} {op} {right}"
    
    def generate_struct(self) -> str:
        """Generate a realistic struct definition."""
        struct_name = self.generate_identifier().title()
        
        fields = []
        field_count = random.randint(2, 8)
        
        for _ in range(field_count):
            field_name = self.generate_identifier()
            field_type = random.choice(self.types)
            visibility = "pub " if random.random() < 0.4 else ""
            fields.append(f"    {visibility}{field_name}: {field_type},")
        
        return f"""struct {struct_name} {{
{chr(10).join(fields)}
}}"""
    
    def generate_enum(self) -> str:
        """Generate a realistic enum definition."""
        enum_name = self.generate_identifier().title()
        
        variants = []
        variant_count = random.randint(2, 6)
        
        for _ in range(variant_count):
            variant_name = self.generate_identifier().title()
            if random.random() < 0.4:
                # Tuple variant
                field_types = [random.choice(self.types) for _ in range(random.randint(1, 3))]
                variants.append(f"    {variant_name}({', '.join(field_types)}),")
            elif random.random() < 0.3:
                # Struct variant  
                variants.append(f"    {variant_name} {{ value: {random.choice(self.types)} }},")
            else:
                # Unit variant
                variants.append(f"    {variant_name},")
        
        return f"""enum {enum_name} {{
{chr(10).join(variants)}
}}"""
    
    def generate_impl_block(self) -> str:
        """Generate a realistic impl block."""
        type_name = self.generate_identifier().title()
        
        methods = []
        method_count = random.randint(1, 4)
        
        for _ in range(method_count):
            method = self.generate_function("small")
            # Add self parameter
            method = method.replace("func ", "func ").replace("(", "(&self, ", 1)
            methods.append("    " + method.replace("\n", "\n    "))
        
        return f"""impl {type_name} {{
{chr(10).join(methods)}
}}"""
    
    def generate_realistic_code(self, target_size: int) -> str:
        """Generate realistic code targeting approximately the given byte size."""
        
        code_parts = []
        current_size = 0
        
        # Add file header
        header = f"""// Generated realistic code for performance testing
// Target size: ~{target_size} bytes

use std::collections::HashMap;
use std::sync::Arc;

"""
        code_parts.append(header)
        current_size += len(header)
        
        # Generate various code constructs
        while current_size < target_size:
            remaining = target_size - current_size
            
            if remaining > 2000:
                # Generate large constructs
                construct_type = random.choice(["function_large", "impl_block", "struct", "enum"])
                
                if construct_type == "function_large":
                    new_code = self.generate_function("large")
                elif construct_type == "impl_block":
                    new_code = self.generate_impl_block()
                elif construct_type == "struct":
                    new_code = self.generate_struct()
                else:
                    new_code = self.generate_enum()
                    
            elif remaining > 500:
                # Generate medium constructs
                construct_type = random.choice(["function_medium", "struct", "comment"])
                
                if construct_type == "function_medium":
                    new_code = self.generate_function("medium")
                elif construct_type == "struct":
                    new_code = self.generate_struct()
                else:
                    new_code = self.generate_comment()
                    
            else:
                # Generate small constructs
                construct_type = random.choice(["function_small", "comment"])
                
                if construct_type == "function_small":
                    new_code = self.generate_function("small")
                else:
                    new_code = self.generate_comment()
            
            code_parts.append("\n\n" + new_code)
            current_size += len(new_code) + 2
            
            # Add some random whitespace and comments for realism
            if random.random() < 0.1:
                comment = "\n" + self.generate_comment()
                code_parts.append(comment)
                current_size += len(comment)
        
        return "".join(code_parts)

def main():
    parser = argparse.ArgumentParser(description="Generate realistic code for lexer benchmarking")
    parser.add_argument("--size", choices=["small", "medium", "large"], default="medium",
                      help="Target code size")
    parser.add_argument("--output", "-o", type=Path, required=True,
                      help="Output file path")
    parser.add_argument("--language", default="seen",
                      help="Target language (default: seen)")
    parser.add_argument("--bytes", type=int, help="Target size in bytes (overrides --size)")
    
    args = parser.parse_args()
    
    # Determine target size
    size_mapping = {
        "small": 10_000,    # 10KB
        "medium": 100_000,  # 100KB  
        "large": 1_000_000  # 1MB
    }
    
    target_size = args.bytes if args.bytes else size_mapping[args.size]
    
    print(f"Generating {args.language} code (~{target_size:,} bytes) -> {args.output}")
    
    # Generate code
    generator = RealisticCodeGenerator(args.language)
    code = generator.generate_realistic_code(target_size)
    
    # Write to output file
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, 'w', encoding='utf-8') as f:
        f.write(code)
    
    actual_size = len(code)
    print(f"Generated {actual_size:,} bytes ({actual_size/target_size*100:.1f}% of target)")
    print(f"Saved to: {args.output}")

if __name__ == "__main__":
    main()