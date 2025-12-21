            FunctionSignature {
                name: "sqrt".to_string(),
                parameters: vec![Parameter {
                    name: "x".to_string(),
                    param_type: Type::Float,
                }],
                return_type: Some(Type::Float),
            },
        );
        env.define_function(
            "__Abs".to_string(),
            FunctionSignature {
                name: "__Abs".to_string(),
                parameters: vec![Parameter {
                    name: "x".to_string(),
                    param_type: Type::Float,
                }],
                return_type: Some(Type::Float),
            },
        );
        env.define_function(
            "__IntToString".to_string(),
            FunctionSignature {
                name: "__IntToString".to_string(),
                parameters: vec![Parameter {
                    name: "value".to_string(),
                    param_type: Type::Int,
                }],
                return_type: Some(Type::String),
            },
        );

        Self {
            env,
            result: TypeCheckResult::new(),
            current_function_return_type: None,
            generic_stack: Vec::new(),
            scope_depth: 0,
            prelude: HashMap::new(),
        }
    }
}
