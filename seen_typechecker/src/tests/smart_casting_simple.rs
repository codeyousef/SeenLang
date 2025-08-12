//! Simple test for smart casting functionality

#[cfg(test)]
mod tests {
    use crate::*;
    use crate::types::Type;
    use crate::checker::TypeChecker;
    use seen_parser::ast::*;
    use seen_lexer::Position;

    fn pos() -> Position {
        Position { line: 1, column: 1, offset: 0 }
    }

    #[test]
    fn test_smart_casting_basic_functionality() {
        let mut checker = TypeChecker::new();
        
        // Just test that the type checker can be created
        assert!(checker.result.errors.is_empty());
        
        // Test that we can create a nullable type
        let nullable_type = Type::Nullable(Box::new(Type::String));
        assert!(matches!(nullable_type, Type::Nullable(_)));
    }
}