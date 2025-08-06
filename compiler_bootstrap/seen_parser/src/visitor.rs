//! AST visitor pattern implementation for traversal and transformation

use crate::ast::*;

/// Trait for immutable AST visitors
pub trait Visitor<'a>: Sized {
    fn visit_program(&mut self, program: &Program<'a>) {
        walk_program(self, program);
    }

    fn visit_item(&mut self, item: &Item<'a>) {
        walk_item(self, item);
    }

    fn visit_function(&mut self, func: &Function<'a>) {
        walk_function(self, func);
    }

    fn visit_struct(&mut self, s: &Struct<'a>) {
        walk_struct(self, s);
    }

    fn visit_enum(&mut self, e: &Enum<'a>) {
        walk_enum(self, e);
    }

    fn visit_block(&mut self, block: &Block<'a>) {
        walk_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &Stmt<'a>) {
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &Expr<'a>) {
        walk_expr(self, expr);
    }

    fn visit_type(&mut self, ty: &Type<'a>) {
        walk_type(self, ty);
    }

    fn visit_pattern(&mut self, pat: &Pattern<'a>) {
        walk_pattern(self, pat);
    }
}

/// Trait for mutable AST visitors
pub trait MutVisitor<'a>: Sized {
    fn visit_program(&mut self, program: &mut Program<'a>) {
        walk_program_mut(self, program);
    }

    fn visit_item(&mut self, item: &mut Item<'a>) {
        walk_item_mut(self, item);
    }

    fn visit_function(&mut self, func: &mut Function<'a>) {
        walk_function_mut(self, func);
    }

    fn visit_struct(&mut self, s: &mut Struct<'a>) {
        walk_struct_mut(self, s);
    }

    fn visit_enum(&mut self, e: &mut Enum<'a>) {
        walk_enum_mut(self, e);
    }

    fn visit_block(&mut self, block: &mut Block<'a>) {
        walk_block_mut(self, block);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt<'a>) {
        walk_stmt_mut(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr<'a>) {
        walk_expr_mut(self, expr);
    }

    fn visit_type(&mut self, ty: &mut Type<'a>) {
        walk_type_mut(self, ty);
    }

    fn visit_pattern(&mut self, pat: &mut Pattern<'a>) {
        walk_pattern_mut(self, pat);
    }
}

// Walk functions for immutable visitors

pub fn walk_program<'a, V: Visitor<'a>>(visitor: &mut V, program: &Program<'a>) {
    for item in &program.items {
        visitor.visit_item(item);
    }
}

pub fn walk_item<'a, V: Visitor<'a>>(visitor: &mut V, item: &Item<'a>) {
    match &item.kind {
        ItemKind::Function(f) => visitor.visit_function(f),
        ItemKind::Struct(s) => visitor.visit_struct(s),
        ItemKind::Enum(e) => visitor.visit_enum(e),
        ItemKind::Impl(i) => {
            visitor.visit_type(&i.self_type);
            if let Some(ref trait_ref) = i.trait_ref {
                visitor.visit_type(trait_ref);
            }
            for impl_item in &i.items {
                match &impl_item.kind {
                    ImplItemKind::Function(f) => visitor.visit_function(f),
                    ImplItemKind::Const(c) => {
                        visitor.visit_type(&c.ty);
                        visitor.visit_expr(&c.value);
                    }
                    ImplItemKind::Type(ta) => visitor.visit_type(&ta.ty),
                }
            }
        }
        ItemKind::Trait(t) => {
            for supertrait in &t.supertraits {
                visitor.visit_type(supertrait);
            }
        }
        ItemKind::Module(m) => {
            for item in &m.items {
                visitor.visit_item(item);
            }
        }
        ItemKind::Import(_) => {}
        ItemKind::TypeAlias(ta) => visitor.visit_type(&ta.ty),
        ItemKind::Const(c) => {
            visitor.visit_type(&c.ty);
            visitor.visit_expr(&c.value);
        }
        ItemKind::Static(s) => {
            visitor.visit_type(&s.ty);
            visitor.visit_expr(&s.value);
        }
        // Kotlin-inspired features
        ItemKind::ExtensionFunction(ext_fn) => {
            visitor.visit_type(&ext_fn.receiver_type);
            visitor.visit_function(&ext_fn.function);
        }
        ItemKind::DataClass(data_class) => {
            for field in &data_class.fields {
                visitor.visit_type(&field.ty);
                if let Some(ref default_value) = field.default_value {
                    visitor.visit_expr(default_value);
                }
            }
        }
        ItemKind::SealedClass(sealed_class) => {
            for variant in &sealed_class.variants {
                for field in &variant.fields {
                    visitor.visit_type(&field.ty);
                    if let Some(ref default_value) = field.default_value {
                        visitor.visit_expr(default_value);
                    }
                }
            }
        }
    }
}

pub fn walk_function<'a, V: Visitor<'a>>(visitor: &mut V, func: &Function<'a>) {
    for param in &func.params {
        visitor.visit_type(&param.ty);
    }
    if let Some(ref ret_type) = func.return_type {
        visitor.visit_type(ret_type);
    }
    visitor.visit_block(&func.body);
}

pub fn walk_struct<'a, V: Visitor<'a>>(visitor: &mut V, s: &Struct<'a>) {
    for field in &s.fields {
        visitor.visit_type(&field.ty);
    }
}

pub fn walk_enum<'a, V: Visitor<'a>>(visitor: &mut V, e: &Enum<'a>) {
    for variant in &e.variants {
        match &variant.data {
            VariantData::Tuple(types) => {
                for ty in types {
                    visitor.visit_type(ty);
                }
            }
            VariantData::Struct(fields) => {
                for field in fields {
                    visitor.visit_type(&field.ty);
                }
            }
            VariantData::Unit => {}
        }
    }
}

pub fn walk_block<'a, V: Visitor<'a>>(visitor: &mut V, block: &Block<'a>) {
    for stmt in &block.statements {
        visitor.visit_stmt(stmt);
    }
}

pub fn walk_stmt<'a, V: Visitor<'a>>(visitor: &mut V, stmt: &Stmt<'a>) {
    match &stmt.kind {
        StmtKind::Let(let_stmt) => {
            visitor.visit_pattern(&let_stmt.pattern);
            if let Some(ref ty) = let_stmt.ty {
                visitor.visit_type(ty);
            }
            if let Some(ref init) = let_stmt.initializer {
                visitor.visit_expr(init);
            }
        }
        StmtKind::Expr(expr) => visitor.visit_expr(expr),
        StmtKind::Item(item) => visitor.visit_item(item),
        StmtKind::Empty => {}
    }
}

pub fn walk_expr<'a, V: Visitor<'a>>(visitor: &mut V, expr: &Expr<'a>) {
    match &*expr.kind {
        ExprKind::Literal(_) | ExprKind::Identifier(_) | ExprKind::Path(_) => {}
        ExprKind::Binary { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        ExprKind::Unary { operand, .. } => visitor.visit_expr(operand),
        ExprKind::Call { function, args } => {
            visitor.visit_expr(function);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visitor.visit_expr(receiver);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::FieldAccess { object, .. } => visitor.visit_expr(object),
        ExprKind::Index { array, index } => {
            visitor.visit_expr(array);
            visitor.visit_expr(index);
        }
        ExprKind::Tuple(elements) | ExprKind::Array(elements) => {
            for elem in elements {
                visitor.visit_expr(elem);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                visitor.visit_expr(&field.value);
            }
        }
        ExprKind::Block(block) => visitor.visit_block(block),
        ExprKind::If { condition, then_branch, else_branch } => {
            visitor.visit_expr(condition);
            visitor.visit_block(then_branch);
            if let Some(ref else_expr) = else_branch {
                visitor.visit_expr(else_expr);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            visitor.visit_expr(scrutinee);
            for arm in arms {
                visitor.visit_pattern(&arm.pattern);
                if let Some(ref guard) = arm.guard {
                    visitor.visit_expr(guard);
                }
                visitor.visit_expr(&arm.body);
            }
        }
        ExprKind::While { condition, body } => {
            visitor.visit_expr(condition);
            visitor.visit_block(body);
        }
        ExprKind::For { pattern, iterator, body } => {
            visitor.visit_pattern(pattern);
            visitor.visit_expr(iterator);
            visitor.visit_block(body);
        }
        ExprKind::Break(expr) | ExprKind::Return(expr) => {
            if let Some(ref expr) = expr {
                visitor.visit_expr(expr);
            }
        }
        ExprKind::Continue => {}
        ExprKind::Assign { target, value } | ExprKind::AssignOp { target, value, .. } => {
            visitor.visit_expr(target);
            visitor.visit_expr(value);
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(ref start) = start {
                visitor.visit_expr(start);
            }
            if let Some(ref end) = end {
                visitor.visit_expr(end);
            }
        }
        ExprKind::Cast { expr, ty } => {
            visitor.visit_expr(expr);
            visitor.visit_type(ty);
        }
        // Kotlin-inspired expressions
        ExprKind::Closure(closure) => {
            for param in &closure.params {
                if let Some(ref param_type) = param.ty {
                    visitor.visit_type(param_type);
                }
            }
            if let Some(ref return_type) = closure.return_type {
                visitor.visit_type(return_type);
            }
            match &closure.body {
                crate::ast::ClosureBody::Expression(expr) => visitor.visit_expr(expr),
                crate::ast::ClosureBody::Block(block) => visitor.visit_block(block),
            }
        }
        ExprKind::NamedArg { value, .. } => {
            visitor.visit_expr(value);
        }
        ExprKind::Null => {}
        ExprKind::SafeCall { receiver, args, .. } => {
            visitor.visit_expr(receiver);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::Elvis { expr, fallback } => {
            visitor.visit_expr(expr);
            visitor.visit_expr(fallback);
        }
    }
}

pub fn walk_type<'a, V: Visitor<'a>>(visitor: &mut V, ty: &Type<'a>) {
    match &*ty.kind {
        TypeKind::Primitive(_) | TypeKind::Infer => {}
        TypeKind::Named { generic_args, .. } => {
            for arg in generic_args {
                visitor.visit_type(arg);
            }
        }
        TypeKind::Tuple(types) => {
            for ty in types {
                visitor.visit_type(ty);
            }
        }
        TypeKind::Array { element_type, size } => {
            visitor.visit_type(element_type);
            if let Some(ref size_expr) = size {
                visitor.visit_expr(size_expr);
            }
        }
        TypeKind::Function { params, return_type } => {
            for param in params {
                visitor.visit_type(param);
            }
            visitor.visit_type(return_type);
        }
        TypeKind::Reference { inner, .. } => visitor.visit_type(inner),
        TypeKind::Nullable(inner) => visitor.visit_type(inner),
    }
}

pub fn walk_pattern<'a, V: Visitor<'a>>(visitor: &mut V, pat: &Pattern<'a>) {
    match &pat.kind {
        PatternKind::Identifier(_) | PatternKind::Wildcard | PatternKind::Literal(_) => {}
        PatternKind::Tuple(patterns) => {
            for pat in patterns {
                visitor.visit_pattern(pat);
            }
        }
        PatternKind::Struct { fields, .. } => {
            for field in fields {
                visitor.visit_pattern(&field.pattern);
            }
        }
        PatternKind::Enum { pattern, .. } => {
            if let Some(ref pat) = pattern {
                visitor.visit_pattern(pat);
            }
        }
    }
}

// Walk functions for mutable visitors

pub fn walk_program_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, program: &mut Program<'a>) {
    for item in &mut program.items {
        visitor.visit_item(item);
    }
}

pub fn walk_item_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, item: &mut Item<'a>) {
    match &mut item.kind {
        ItemKind::Function(f) => visitor.visit_function(f),
        ItemKind::Struct(s) => visitor.visit_struct(s),
        ItemKind::Enum(e) => visitor.visit_enum(e),
        ItemKind::Impl(i) => {
            visitor.visit_type(&mut i.self_type);
            if let Some(ref mut trait_ref) = i.trait_ref {
                visitor.visit_type(trait_ref);
            }
            for impl_item in &mut i.items {
                match &mut impl_item.kind {
                    ImplItemKind::Function(f) => visitor.visit_function(f),
                    ImplItemKind::Const(c) => {
                        visitor.visit_type(&mut c.ty);
                        visitor.visit_expr(&mut c.value);
                    }
                    ImplItemKind::Type(ta) => visitor.visit_type(&mut ta.ty),
                }
            }
        }
        ItemKind::Trait(t) => {
            for supertrait in &mut t.supertraits {
                visitor.visit_type(supertrait);
            }
        }
        ItemKind::Module(m) => {
            for item in &mut m.items {
                visitor.visit_item(item);
            }
        }
        ItemKind::Import(_) => {}
        ItemKind::TypeAlias(ta) => visitor.visit_type(&mut ta.ty),
        ItemKind::Const(c) => {
            visitor.visit_type(&mut c.ty);
            visitor.visit_expr(&mut c.value);
        }
        ItemKind::Static(s) => {
            visitor.visit_type(&mut s.ty);
            visitor.visit_expr(&mut s.value);
        }
        // Kotlin-inspired features
        ItemKind::ExtensionFunction(ext_fn) => {
            visitor.visit_type(&mut ext_fn.receiver_type);
            visitor.visit_function(&mut ext_fn.function);
        }
        ItemKind::DataClass(data_class) => {
            for field in &mut data_class.fields {
                visitor.visit_type(&mut field.ty);
                if let Some(ref mut default_value) = field.default_value {
                    visitor.visit_expr(default_value);
                }
            }
        }
        ItemKind::SealedClass(sealed_class) => {
            for variant in &mut sealed_class.variants {
                for field in &mut variant.fields {
                    visitor.visit_type(&mut field.ty);
                    if let Some(ref mut default_value) = field.default_value {
                        visitor.visit_expr(default_value);
                    }
                }
            }
        }
    }
}

pub fn walk_function_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, func: &mut Function<'a>) {
    for param in &mut func.params {
        visitor.visit_type(&mut param.ty);
    }
    if let Some(ref mut ret_type) = func.return_type {
        visitor.visit_type(ret_type);
    }
    visitor.visit_block(&mut func.body);
}

pub fn walk_struct_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, s: &mut Struct<'a>) {
    for field in &mut s.fields {
        visitor.visit_type(&mut field.ty);
    }
}

pub fn walk_enum_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, e: &mut Enum<'a>) {
    for variant in &mut e.variants {
        match &mut variant.data {
            VariantData::Tuple(types) => {
                for ty in types {
                    visitor.visit_type(ty);
                }
            }
            VariantData::Struct(fields) => {
                for field in fields {
                    visitor.visit_type(&mut field.ty);
                }
            }
            VariantData::Unit => {}
        }
    }
}

pub fn walk_block_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, block: &mut Block<'a>) {
    for stmt in &mut block.statements {
        visitor.visit_stmt(stmt);
    }
}

pub fn walk_stmt_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, stmt: &mut Stmt<'a>) {
    match &mut stmt.kind {
        StmtKind::Let(let_stmt) => {
            visitor.visit_pattern(&mut let_stmt.pattern);
            if let Some(ref mut ty) = let_stmt.ty {
                visitor.visit_type(ty);
            }
            if let Some(ref mut init) = let_stmt.initializer {
                visitor.visit_expr(init);
            }
        }
        StmtKind::Expr(expr) => visitor.visit_expr(expr),
        StmtKind::Item(item) => visitor.visit_item(item),
        StmtKind::Empty => {}
    }
}

pub fn walk_expr_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, expr: &mut Expr<'a>) {
    match &mut *expr.kind {
        ExprKind::Literal(_) | ExprKind::Identifier(_) | ExprKind::Path(_) => {}
        ExprKind::Binary { left, right, .. } => {
            visitor.visit_expr(left);
            visitor.visit_expr(right);
        }
        ExprKind::Unary { operand, .. } => visitor.visit_expr(operand),
        ExprKind::Call { function, args } => {
            visitor.visit_expr(function);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            visitor.visit_expr(receiver);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::FieldAccess { object, .. } => visitor.visit_expr(object),
        ExprKind::Index { array, index } => {
            visitor.visit_expr(array);
            visitor.visit_expr(index);
        }
        ExprKind::Tuple(elements) | ExprKind::Array(elements) => {
            for elem in elements {
                visitor.visit_expr(elem);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                visitor.visit_expr(&mut field.value);
            }
        }
        ExprKind::Block(block) => visitor.visit_block(block),
        ExprKind::If { condition, then_branch, else_branch } => {
            visitor.visit_expr(condition);
            visitor.visit_block(then_branch);
            if let Some(ref mut else_expr) = else_branch {
                visitor.visit_expr(else_expr);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            visitor.visit_expr(scrutinee);
            for arm in arms {
                visitor.visit_pattern(&mut arm.pattern);
                if let Some(ref mut guard) = arm.guard {
                    visitor.visit_expr(guard);
                }
                visitor.visit_expr(&mut arm.body);
            }
        }
        ExprKind::While { condition, body } => {
            visitor.visit_expr(condition);
            visitor.visit_block(body);
        }
        ExprKind::For { pattern, iterator, body } => {
            visitor.visit_pattern(pattern);
            visitor.visit_expr(iterator);
            visitor.visit_block(body);
        }
        ExprKind::Break(expr) | ExprKind::Return(expr) => {
            if let Some(ref mut expr) = expr {
                visitor.visit_expr(expr);
            }
        }
        ExprKind::Continue => {}
        ExprKind::Assign { target, value } | ExprKind::AssignOp { target, value, .. } => {
            visitor.visit_expr(target);
            visitor.visit_expr(value);
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(ref mut start) = start {
                visitor.visit_expr(start);
            }
            if let Some(ref mut end) = end {
                visitor.visit_expr(end);
            }
        }
        ExprKind::Cast { expr, ty } => {
            visitor.visit_expr(expr);
            visitor.visit_type(ty);
        }
        // Kotlin-inspired expressions  
        ExprKind::Closure(closure) => {
            for param in &mut closure.params {
                if let Some(ref mut param_type) = param.ty {
                    visitor.visit_type(param_type);
                }
            }
            if let Some(ref mut return_type) = closure.return_type {
                visitor.visit_type(return_type);
            }
            match &mut closure.body {
                crate::ast::ClosureBody::Expression(expr) => visitor.visit_expr(expr),
                crate::ast::ClosureBody::Block(block) => visitor.visit_block(block),
            }
        }
        ExprKind::NamedArg { value, .. } => {
            visitor.visit_expr(value);
        }
        ExprKind::Null => {}
        ExprKind::SafeCall { receiver, args, .. } => {
            visitor.visit_expr(receiver);
            for arg in args {
                visitor.visit_expr(arg);
            }
        }
        ExprKind::Elvis { expr, fallback } => {
            visitor.visit_expr(expr);
            visitor.visit_expr(fallback);
        }
    }
}

pub fn walk_type_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, ty: &mut Type<'a>) {
    match &mut *ty.kind {
        TypeKind::Primitive(_) | TypeKind::Infer => {}
        TypeKind::Named { generic_args, .. } => {
            for arg in generic_args {
                visitor.visit_type(arg);
            }
        }
        TypeKind::Tuple(types) => {
            for ty in types {
                visitor.visit_type(ty);
            }
        }
        TypeKind::Array { element_type, size } => {
            visitor.visit_type(element_type);
            if let Some(ref mut size_expr) = size {
                visitor.visit_expr(size_expr);
            }
        }
        TypeKind::Function { params, return_type } => {
            for param in params {
                visitor.visit_type(param);
            }
            visitor.visit_type(return_type);
        }
        TypeKind::Reference { inner, .. } => visitor.visit_type(inner),
        TypeKind::Nullable(inner) => visitor.visit_type(inner),
    }
}

pub fn walk_pattern_mut<'a, V: MutVisitor<'a>>(visitor: &mut V, pat: &mut Pattern<'a>) {
    match &mut pat.kind {
        PatternKind::Identifier(_) | PatternKind::Wildcard | PatternKind::Literal(_) => {}
        PatternKind::Tuple(patterns) => {
            for pat in patterns {
                visitor.visit_pattern(pat);
            }
        }
        PatternKind::Struct { fields, .. } => {
            for field in fields {
                visitor.visit_pattern(&mut field.pattern);
            }
        }
        PatternKind::Enum { pattern, .. } => {
            if let Some(ref mut pat) = pattern {
                visitor.visit_pattern(pat);
            }
        }
    }
}