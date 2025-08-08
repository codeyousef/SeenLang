// Zig lexer benchmark implementation for fair comparison with Seen
// Uses a realistic tokenizer similar to what Seen's lexer would do

const std = @import("std");
const print = std.debug.print;
const Allocator = std.mem.Allocator;

const TokenType = enum {
    // Keywords
    func, let, mut, @"if", @"else", @"while", @"for", loop, @"return", @"break", @"continue",
    @"struct", @"enum", impl, trait, pub, priv, mod, use, import, export,
    match, when, @"try", @"catch", finally, @"async", await, @"const", static,
    @"type", interface, class, extends, implements, abstract, @"override",
    virtual, final,
    
    // Types
    i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool, char, str,
    string, vec, hashmap, hashset, option, result, box, rc, arc,
    
    // Literals
    integer_literal,
    float_literal,
    string_literal,
    char_literal,
    bool_literal,
    
    // Identifiers
    identifier,
    
    // Operators
    plus, minus, star, slash, percent, equal, equal_equal, not_equal,
    less, less_equal, greater, greater_equal, and_and, or_or, not,
    and_op, or_op, xor, left_shift, right_shift, plus_equal, minus_equal,
    star_equal, slash_equal, percent_equal, and_equal, or_equal,
    xor_equal, left_shift_equal, right_shift_equal,
    
    // Punctuation
    left_paren, right_paren, left_brace, right_brace, left_bracket,
    right_bracket, semicolon, comma, dot, arrow, fat_arrow, colon,
    double_colon, question, at, dollar, hash,
    
    // Special
    newline,
    whitespace,
    comment,
    eof,
    invalid
};

const Token = struct {
    type: TokenType,
    lexeme: []const u8,
    line: usize,
    column: usize,
};

const Lexer = struct {
    input: []const u8,
    position: usize,
    line: usize,
    column: usize,
    keywords: std.StringHashMap(TokenType),
    allocator: Allocator,
    
    const Self = @This();
    
    pub fn init(allocator: Allocator, input: []const u8) !Self {
        var keywords = std.StringHashMap(TokenType).init(allocator);
        
        // Keywords
        try keywords.put("func", .func);
        try keywords.put("let", .let);
        try keywords.put("mut", .mut);
        try keywords.put("if", .@"if");
        try keywords.put("else", .@"else");
        try keywords.put("while", .@"while");
        try keywords.put("for", .@"for");
        try keywords.put("loop", .loop);
        try keywords.put("return", .@"return");
        try keywords.put("break", .@"break");
        try keywords.put("continue", .@"continue");
        try keywords.put("struct", .@"struct");
        try keywords.put("enum", .@"enum");
        try keywords.put("impl", .impl);
        try keywords.put("trait", .trait);
        try keywords.put("pub", .pub);
        try keywords.put("priv", .priv);
        try keywords.put("mod", .mod);
        try keywords.put("use", .use);
        try keywords.put("import", .import);
        try keywords.put("export", .export);
        try keywords.put("match", .match);
        try keywords.put("when", .when);
        try keywords.put("try", .@"try");
        try keywords.put("catch", .@"catch");
        try keywords.put("finally", .finally);
        try keywords.put("async", .@"async");
        try keywords.put("await", .await);
        try keywords.put("const", .@"const");
        try keywords.put("static", .static);
        try keywords.put("type", .@"type");
        try keywords.put("interface", .interface);
        try keywords.put("class", .class);
        try keywords.put("extends", .extends);
        try keywords.put("implements", .implements);
        try keywords.put("abstract", .abstract);
        try keywords.put("override", .@"override");
        try keywords.put("virtual", .virtual);
        try keywords.put("final", .final);
        
        // Types
        try keywords.put("i8", .i8);
        try keywords.put("i16", .i16);
        try keywords.put("i32", .i32);
        try keywords.put("i64", .i64);
        try keywords.put("u8", .u8);
        try keywords.put("u16", .u16);
        try keywords.put("u32", .u32);
        try keywords.put("u64", .u64);
        try keywords.put("f32", .f32);
        try keywords.put("f64", .f64);
        try keywords.put("bool", .bool);
        try keywords.put("char", .char);
        try keywords.put("str", .str);
        try keywords.put("String", .string);
        try keywords.put("Vec", .vec);
        try keywords.put("HashMap", .hashmap);
        try keywords.put("HashSet", .hashset);
        try keywords.put("Option", .option);
        try keywords.put("Result", .result);
        try keywords.put("Box", .box);
        try keywords.put("Rc", .rc);
        try keywords.put("Arc", .arc);
        
        // Boolean literals
        try keywords.put("true", .bool_literal);
        try keywords.put("false", .bool_literal);
        
        return Self{
            .input = input,
            .position = 0,
            .line = 1,
            .column = 1,
            .keywords = keywords,
            .allocator = allocator,
        };
    }
    
    pub fn deinit(self: *Self) void {
        self.keywords.deinit();
    }
    
    fn isAtEnd(self: *const Self) bool {
        return self.position >= self.input.len;
    }
    
    fn currentChar(self: *const Self) u8 {
        if (self.isAtEnd()) return 0;
        return self.input[self.position];
    }
    
    fn peekChar(self: *const Self) u8 {
        if (self.position + 1 >= self.input.len) return 0;
        return self.input[self.position + 1];
    }
    
    fn advance(self: *Self) u8 {
        if (self.isAtEnd()) return 0;
        
        const ch = self.input[self.position];
        self.position += 1;
        
        if (ch == '\n') {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        
        return ch;
    }
    
    fn skipWhitespace(self: *Self) void {
        while (!self.isAtEnd()) {
            const ch = self.currentChar();
            if (ch == ' ' or ch == '\t' or ch == '\r' or ch == '\n') {
                _ = self.advance();
            } else {
                break;
            }
        }
    }
    
    fn scanLineComment(self: *Self, start_line: usize, start_column: usize) !Token {
        _ = self.advance(); // consume second '/'
        
        const start = self.position - 2; // Include the "//"
        
        while (!self.isAtEnd() and self.currentChar() != '\n') {
            _ = self.advance();
        }
        
        const lexeme = self.input[start..self.position];
        
        return Token{
            .type = .comment,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanBlockComment(self: *Self, start_line: usize, start_column: usize) !Token {
        _ = self.advance(); // consume '*'
        
        const start = self.position - 2; // Include the "/*"
        var depth: u32 = 1;
        
        while (depth > 0 and !self.isAtEnd()) {
            const ch = self.advance();
            
            if (ch == '*' and self.currentChar() == '/') {
                _ = self.advance();
                depth -= 1;
            } else if (ch == '/' and self.currentChar() == '*') {
                _ = self.advance();
                depth += 1;
            }
        }
        
        const lexeme = self.input[start..self.position];
        
        return Token{
            .type = .comment,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanStringLiteral(self: *Self, start_line: usize, start_column: usize) !Token {
        const start = self.position - 1; // Include opening quote
        
        while (!self.isAtEnd() and self.currentChar() != '"') {
            const ch = self.currentChar();
            if (ch == '\\') {
                _ = self.advance(); // Skip escape character
                if (!self.isAtEnd()) {
                    _ = self.advance(); // Skip escaped character
                }
            } else {
                _ = self.advance();
            }
        }
        
        if (!self.isAtEnd()) {
            _ = self.advance(); // closing quote
        }
        
        const lexeme = self.input[start..self.position];
        
        return Token{
            .type = .string_literal,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanCharLiteral(self: *Self, start_line: usize, start_column: usize) !Token {
        const start = self.position - 1; // Include opening quote
        
        if (!self.isAtEnd()) {
            const ch = self.currentChar();
            if (ch == '\\') {
                _ = self.advance(); // Skip escape character
                if (!self.isAtEnd()) {
                    _ = self.advance(); // Skip escaped character
                }
            } else {
                _ = self.advance();
            }
        }
        
        if (!self.isAtEnd() and self.currentChar() == '\'') {
            _ = self.advance(); // closing quote
        }
        
        const lexeme = self.input[start..self.position];
        
        return Token{
            .type = .char_literal,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanNumber(self: *Self, start_line: usize, start_column: usize) !Token {
        const start = self.position - 1; // Include first digit
        var is_float = false;
        
        while (!self.isAtEnd() and std.ascii.isDigit(self.currentChar())) {
            _ = self.advance();
        }
        
        if (!self.isAtEnd() and self.currentChar() == '.' and std.ascii.isDigit(self.peekChar())) {
            is_float = true;
            _ = self.advance(); // consume '.'
            
            while (!self.isAtEnd() and std.ascii.isDigit(self.currentChar())) {
                _ = self.advance();
            }
        }
        
        const lexeme = self.input[start..self.position];
        const token_type: TokenType = if (is_float) .float_literal else .integer_literal;
        
        return Token{
            .type = token_type,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanIdentifier(self: *Self, start_line: usize, start_column: usize) !Token {
        const start = self.position - 1; // Include first character
        
        while (!self.isAtEnd()) {
            const ch = self.currentChar();
            if (std.ascii.isAlphanumeric(ch) or ch == '_') {
                _ = self.advance();
            } else {
                break;
            }
        }
        
        const lexeme = self.input[start..self.position];
        const token_type = self.keywords.get(lexeme) orelse .identifier;
        
        return Token{
            .type = token_type,
            .lexeme = lexeme,
            .line = start_line,
            .column = start_column,
        };
    }
    
    fn scanToken(self: *Self) !Token {
        const start_line = self.line;
        const start_column = self.column;
        
        if (self.isAtEnd()) {
            return Token{
                .type = .eof,
                .lexeme = "",
                .line = start_line,
                .column = start_column,
            };
        }
        
        const ch = self.advance();
        
        return switch (ch) {
            // Single character tokens
            '(' => Token{ .type = .left_paren, .lexeme = "(", .line = start_line, .column = start_column },
            ')' => Token{ .type = .right_paren, .lexeme = ")", .line = start_line, .column = start_column },
            '{' => Token{ .type = .left_brace, .lexeme = "{", .line = start_line, .column = start_column },
            '}' => Token{ .type = .right_brace, .lexeme = "}", .line = start_line, .column = start_column },
            '[' => Token{ .type = .left_bracket, .lexeme = "[", .line = start_line, .column = start_column },
            ']' => Token{ .type = .right_bracket, .lexeme = "]", .line = start_line, .column = start_column },
            ';' => Token{ .type = .semicolon, .lexeme = ";", .line = start_line, .column = start_column },
            ',' => Token{ .type = .comma, .lexeme = ",", .line = start_line, .column = start_column },
            '.' => Token{ .type = .dot, .lexeme = ".", .line = start_line, .column = start_column },
            '?' => Token{ .type = .question, .lexeme = "?", .line = start_line, .column = start_column },
            '@' => Token{ .type = .at, .lexeme = "@", .line = start_line, .column = start_column },
            '$' => Token{ .type = .dollar, .lexeme = "$", .line = start_line, .column = start_column },
            '#' => Token{ .type = .hash, .lexeme = "#", .line = start_line, .column = start_column },
            
            // Potentially multi-character tokens
            ':' => {
                if (self.currentChar() == ':') {
                    _ = self.advance();
                    return Token{ .type = .double_colon, .lexeme = "::", .line = start_line, .column = start_column };
                }
                return Token{ .type = .colon, .lexeme = ":", .line = start_line, .column = start_column };
            },
            
            '+' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .plus_equal, .lexeme = "+=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .plus, .lexeme = "+", .line = start_line, .column = start_column };
            },
            
            '-' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .minus_equal, .lexeme = "-=", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '>') {
                    _ = self.advance();
                    return Token{ .type = .arrow, .lexeme = "->", .line = start_line, .column = start_column };
                }
                return Token{ .type = .minus, .lexeme = "-", .line = start_line, .column = start_column };
            },
            
            '*' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .star_equal, .lexeme = "*=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .star, .lexeme = "*", .line = start_line, .column = start_column };
            },
            
            '/' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .slash_equal, .lexeme = "/=", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '/') {
                    return self.scanLineComment(start_line, start_column);
                } else if (self.currentChar() == '*') {
                    return self.scanBlockComment(start_line, start_column);
                }
                return Token{ .type = .slash, .lexeme = "/", .line = start_line, .column = start_column };
            },
            
            '%' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .percent_equal, .lexeme = "%=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .percent, .lexeme = "%", .line = start_line, .column = start_column };
            },
            
            '=' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .equal_equal, .lexeme = "==", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '>') {
                    _ = self.advance();
                    return Token{ .type = .fat_arrow, .lexeme = "=>", .line = start_line, .column = start_column };
                }
                return Token{ .type = .equal, .lexeme = "=", .line = start_line, .column = start_column };
            },
            
            '!' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .not_equal, .lexeme = "!=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .not, .lexeme = "!", .line = start_line, .column = start_column };
            },
            
            '<' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .less_equal, .lexeme = "<=", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '<') {
                    _ = self.advance();
                    if (self.currentChar() == '=') {
                        _ = self.advance();
                        return Token{ .type = .left_shift_equal, .lexeme = "<<=", .line = start_line, .column = start_column };
                    }
                    return Token{ .type = .left_shift, .lexeme = "<<", .line = start_line, .column = start_column };
                }
                return Token{ .type = .less, .lexeme = "<", .line = start_line, .column = start_column };
            },
            
            '>' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .greater_equal, .lexeme = ">=", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '>') {
                    _ = self.advance();
                    if (self.currentChar() == '=') {
                        _ = self.advance();
                        return Token{ .type = .right_shift_equal, .lexeme = ">>=", .line = start_line, .column = start_column };
                    }
                    return Token{ .type = .right_shift, .lexeme = ">>", .line = start_line, .column = start_column };
                }
                return Token{ .type = .greater, .lexeme = ">", .line = start_line, .column = start_column };
            },
            
            '&' => {
                if (self.currentChar() == '&') {
                    _ = self.advance();
                    return Token{ .type = .and_and, .lexeme = "&&", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .and_equal, .lexeme = "&=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .and_op, .lexeme = "&", .line = start_line, .column = start_column };
            },
            
            '|' => {
                if (self.currentChar() == '|') {
                    _ = self.advance();
                    return Token{ .type = .or_or, .lexeme = "||", .line = start_line, .column = start_column };
                } else if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .or_equal, .lexeme = "|=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .or_op, .lexeme = "|", .line = start_line, .column = start_column };
            },
            
            '^' => {
                if (self.currentChar() == '=') {
                    _ = self.advance();
                    return Token{ .type = .xor_equal, .lexeme = "^=", .line = start_line, .column = start_column };
                }
                return Token{ .type = .xor, .lexeme = "^", .line = start_line, .column = start_column };
            },
            
            // String and character literals
            '"' => self.scanStringLiteral(start_line, start_column),
            '\'' => self.scanCharLiteral(start_line, start_column),
            
            // Numbers
            '0'...'9' => self.scanNumber(start_line, start_column),
            
            // Identifiers and keywords
            'a'...'z', 'A'...'Z', '_' => self.scanIdentifier(start_line, start_column),
            
            // Invalid character (including Unicode)
            else => Token{
                .type = .invalid,
                .lexeme = self.input[self.position - 1..self.position],
                .line = start_line,
                .column = start_column,
            },
        };
    }
    
    pub fn tokenize(self: *Self, allocator: Allocator) !std.ArrayList(Token) {
        var tokens = std.ArrayList(Token).init(allocator);
        
        while (!self.isAtEnd()) {
            self.skipWhitespace();
            if (self.isAtEnd()) break;
            
            const token = try self.scanToken();
            try tokens.append(token);
        }
        
        try tokens.append(Token{
            .type = .eof,
            .lexeme = "",
            .line = self.line,
            .column = self.column,
        });
        
        return tokens;
    }
};

fn readFile(allocator: Allocator, filename: []const u8) ![]u8 {
    const file = std.fs.cwd().openFile(filename, .{}) catch |err| switch (err) {
        error.FileNotFound => {
            print("Error: File not found: {s}\n", .{filename});
            return err;
        },
        else => return err,
    };
    defer file.close();
    
    const file_size = try file.getEndPos();
    const contents = try allocator.alloc(u8, file_size);
    _ = try file.readAll(contents);
    
    return contents;
}

fn benchmarkLexerRealWorld(allocator: Allocator) !void {
    const test_files = [_][]const u8{
        "../../test_data/large_codebases/large_codebase.seen",
        "../../test_data/large_codebases/minified_code.seen",
        "../../test_data/large_codebases/sparse_code.seen",
        "../../test_data/large_codebases/unicode_heavy.seen",
    };
    
    var total_tokens: u64 = 0;
    var total_time: f64 = 0.0;
    
    for (test_files) |file_path| {
        // Check if file exists
        const file_exists = std.fs.cwd().access(file_path, .{}) catch |err| switch (err) {
            error.FileNotFound => false,
            else => return err,
        };
        
        if (!file_exists) {
            print("Warning: Test file {s} not found, skipping...\n", .{file_path});
            continue;
        }
        
        const content = readFile(allocator, file_path) catch |err| {
            print("Error reading {s}: {}\n", .{ file_path, err });
            continue;
        };
        defer allocator.free(content);
        
        const file_size = content.len;
        print("Testing Zig lexer performance on {s} ({} bytes)\n", .{ file_path, file_size });
        
        // Run multiple iterations for statistical accuracy
        const iterations = 10;
        var file_tokens: usize = 0;
        var file_time: f64 = 0.0;
        
        var i: u32 = 0;
        while (i < iterations) : (i += 1) {
            var lexer = try Lexer.init(allocator, content);
            defer lexer.deinit();
            
            const start_time = std.time.nanoTimestamp();
            var tokens = try lexer.tokenize(allocator);
            const end_time = std.time.nanoTimestamp();
            defer tokens.deinit();
            
            file_tokens = tokens.items.len;
            const elapsed = @as(f64, @floatFromInt(end_time - start_time)) / 1_000_000_000.0; // Convert to seconds
            file_time += elapsed;
        }
        
        const avg_time = file_time / @as(f64, @floatFromInt(iterations));
        const tokens_per_second = if (avg_time > 0.0) @as(f64, @floatFromInt(file_tokens)) / avg_time else 0.0;
        
        print("  Tokens: {}, Avg Time: {d:.6}s, Tokens/sec: {d:.0}\n", .{ file_tokens, avg_time, tokens_per_second });
        
        total_tokens += file_tokens;
        total_time += avg_time;
    }
    
    const overall_tokens_per_sec = if (total_time > 0.0) @as(f64, @floatFromInt(total_tokens)) / total_time else 0.0;
    
    print("\nZig Lexer Overall Performance:\n");
    print("  Total tokens: {}\n", .{total_tokens});
    print("  Total time: {d:.6}s\n", .{total_time});
    print("  Average tokens/second: {d:.0}\n", .{overall_tokens_per_sec});
    
    // Check if it meets the 14M tokens/sec claim
    if (overall_tokens_per_sec >= 14_000_000.0) {
        print("✅ ZIG BASELINE: Achieved {d:.1}M tokens/sec\n", .{overall_tokens_per_sec / 1_000_000.0});
    } else {
        print("❌ ZIG BASELINE: Achieved {d:.1}M tokens/sec (target: 14M)\n", .{overall_tokens_per_sec / 1_000_000.0});
    }
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();
    
    benchmarkLexerRealWorld(allocator) catch |err| {
        print("Error running Zig lexer benchmark: {}\n", .{err});
        std.process.exit(1);
    };
}