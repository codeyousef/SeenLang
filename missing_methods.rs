impl TypeChecker {
    fn predeclare_struct_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Struct {
            name: name.to_string(),
            fields: HashMap::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn predeclare_enum_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Enum {
            name: name.to_string(),
            variants: Vec::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn predeclare_enum_type_with_variants(
        &mut self,
        name: &str,
        generics: &[String],
        variants: &[seen_parser::ast::EnumVariant],
    ) {
        if self.env.get_type(name).is_some() {
            return;
        }
        // Extract variant names immediately during predeclaration
        let variant_names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
        let enum_type = Type::Enum {
            name: name.to_string(),
            variants: variant_names,
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
        };
        self.env.define_type(name.to_string(), enum_type);
    }

    fn predeclare_interface_type(&mut self, name: &str, generics: &[String]) {
        if self.env.get_type(name).is_some() {
            return;
        }
        let placeholder = Type::Interface {
            name: name.to_string(),
            methods: Vec::new(),
            generics: generics.iter().map(|g| Type::Generic(g.clone())).collect(),
            is_sealed: false,
        };
        self.env.define_type(name.to_string(), placeholder);
    }

    fn handle_import(
        &mut self,
        module_path: &[String],
        symbols: &[seen_parser::ast::ImportSymbol],
        _pos: Position,
    ) {
        let module_name = module_path.join(".");

        match module_name.as_str() {
            "bootstrap.frontend" => {
                for symbol in symbols {
                    match symbol.name.as_str() {
                        "FrontendResult" => {
                            let mut fields = HashMap::new();
                            fields.insert("success".to_string(), Type::Bool);
                            fields.insert(
                                "diagnostics".to_string(),
                                Type::Array(Box::new(Type::Unknown)),
                            );
                            self.env.define_type(
                                "FrontendResult".to_string(),
                                Type::Struct {
                                    name: "FrontendResult".to_string(),
                                    fields,
                                    generics: vec![],
                                },
                            );
                        }
                        "FrontendDiagnostic" => {
                            let mut fields = HashMap::new();
                            fields.insert("file".to_string(), Type::String);
                            fields.insert("line".to_string(), Type::Int);
                            fields.insert("column".to_string(), Type::Int);
                            fields.insert("severity".to_string(), Type::String);
                            fields.insert("message".to_string(), Type::String);
                            self.env.define_type(
                                "FrontendDiagnostic".to_string(),
                                Type::Struct {
                                    name: "FrontendDiagnostic".to_string(),
                                    fields,
                                    generics: vec![],
                                },
                            );
                        }
                        "run_frontend" => {
                            let mut result_fields = HashMap::new();
                            result_fields.insert("success".to_string(), Type::Bool);
                            result_fields.insert(
                                "diagnostics".to_string(),
                                Type::Array(Box::new(Type::Unknown)),
                            );

                            self.env.define_function(
                                "run_frontend".to_string(),
                                FunctionSignature {
                                    name: "run_frontend".to_string(),
                                    parameters: vec![
                                        Parameter {
                                            name: "source".to_string(),
                                            param_type: Type::String,
                                        },
                                        Parameter {
                                            name: "fileLabel".to_string(),
                                            param_type: Type::String,
                                        },
                                        Parameter {
                                            name: "language".to_string(),
                                            param_type: Type::String,
                                        },
                                    ],
                                    return_type: Some(Type::Struct {
                                        name: "FrontendResult".to_string(),
                                        fields: result_fields,
                                        generics: vec![],
                                    }),
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn with_generics<F, R>(&mut self, generics: &[String], f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        if !generics.is_empty() {
            self.generic_stack.push(generics.to_vec());
            let result = f(self);
            self.generic_stack.pop();
            result
        } else {
            f(self)
        }
    }

    fn is_generic_name(&self, name: &str) -> bool {
        self.generic_stack
            .iter()
            .rev()
            .any(|scope| scope.iter().any(|g| g == name))
    }

    fn resolve_ast_type(&mut self, ast_type: &seen_parser::Type, pos: Position) -> Type {
        if self.is_generic_name(&ast_type.name) && ast_type.generics.is_empty() {
            let base = Type::Generic(ast_type.name.clone());
            return if ast_type.is_nullable {
                Type::Nullable(Box::new(base))
            } else {
                base
            };
        }

        let resolved_args: Vec<Type> = ast_type
            .generics
            .iter()
            .map(|g| self.resolve_ast_type(g, pos))
            .collect();

        let mut base = match ast_type.name.as_str() {
            "Int" => Type::Int,
            "UInt" => Type::UInt,
            "Float" => Type::Float,
            "Bool" => Type::Bool,
            "String" => Type::String,
            "Char" => Type::Char,
            "Void" | "()" | "Unit" => Type::Unit,
            "Array" | "List" | "Vec" => {
                if resolved_args.len() == 1 {
                    Type::Array(Box::new(resolved_args[0].clone()))
                } else {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: ast_type.name.clone(),
                        expected: 1,
                        actual: resolved_args.len(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            "Map" | "HashMap" | "Dict" => {
                if resolved_args.len() == 2 {
                    Type::Map {
                        key_type: Box::new(resolved_args[0].clone()),
                        value_type: Box::new(resolved_args[1].clone()),
                    }
                } else {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: ast_type.name.clone(),
                        expected: 2,
                        actual: resolved_args.len(),
                        position: pos,
                    });
                    Type::Unknown
                }
            }
            _ => {
                if let Some(mut def) = self.env.get_type(&ast_type.name).cloned() {
                    if let Type::Struct { name, fields, .. } = &def {
                        if fields.is_empty() {
                            if let Some(fresh) = self.env.get_type(name) {
                                if let Type::Struct {
                                    fields: fresh_fields,
                                    ..
                                } = fresh
                                {
                                    if !fresh_fields.is_empty() {
                                        def = fresh.clone();
                                    }
                                }
                            }
                        }
                    }
                    return self.instantiate_type(def, &resolved_args, pos);
                }

                Type::Struct {
                    name: ast_type.name.clone(),
                    fields: HashMap::new(),
                    generics: resolved_args.clone(),
                }
            }
        };

        if ast_type.is_nullable {
            base = Type::Nullable(Box::new(base));
        }

        base
    }

    fn instantiate_type(&mut self, definition: Type, args: &[Type], pos: Position) -> Type {
        match definition {
            Type::Struct {
                name,
                fields,
                generics,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                let mut mapping = HashMap::new();
                for (param, arg) in generics.iter().zip(args.iter()) {
                    if let Type::Generic(param_name) = param {
                        mapping.insert(param_name.clone(), arg.clone());
                    }
                }

                let substituted_fields = fields
                    .into_iter()
                    .map(|(field, ty)| (field, self.substitute_generics(&ty, &mapping)))
                    .collect();

                Type::Struct {
                    name,
                    fields: substituted_fields,
                    generics: args.to_vec(),
                }
            }
            Type::Enum {
                name,
                variants,
                generics,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                let actual_variants = if variants.is_empty() {
                    if let Some(fresh_type) = self.env.get_type(&name) {
                        if let Type::Enum {
                            variants: fresh_variants,
                            ..
                        } = fresh_type
                        {
                            if !fresh_variants.is_empty() {
                                fresh_variants.clone()
                            } else {
                                variants
                            }
                        } else {
                            variants
                        }
                    } else {
                        variants
                    }
                } else {
                    variants
                };

                Type::Enum {
                    name,
                    variants: actual_variants,
                    generics: args.to_vec(),
                }
            }
            Type::Interface {
                name,
                methods,
                generics,
                is_sealed,
            } => {
                if generics.len() != args.len() {
                    self.result.add_error(TypeError::GenericArityMismatch {
                        type_name: name.clone(),
                        expected: generics.len(),
                        actual: args.len(),
                        position: pos,
                    });
                    return Type::Unknown;
                }

                Type::Interface {
                    name,
                    methods,
                    generics: args.to_vec(),
                    is_sealed,
                }
            }
            other => other,
        }
    }

    fn placeholder_generic_args(&self, definition: &Type) -> Vec<Type> {
        match definition {
            Type::Struct { generics, .. }
            | Type::Enum { generics, .. }
            | Type::Interface { generics, .. } => generics.iter().map(|_| Type::Unknown).collect(),
            _ => Vec::new(),
        }
    }

    fn type_from_identifier(&mut self, name: &str, pos: Position) -> Option<Type> {
        if let Some(definition) = self.env.get_type(name).cloned() {
            let placeholders = self.placeholder_generic_args(&definition);
            return Some(self.instantiate_type(definition, &placeholders, pos));
        }

        match name {
            "Int" => Some(Type::Int),
            "UInt" => Some(Type::UInt),
            "Float" => Some(Type::Float),
            "Bool" => Some(Type::Bool),
            "String" => Some(Type::String),
            "Char" => Some(Type::Char),
            "Unit" | "Void" | "()" => Some(Type::Unit),
            "Never" => Some(Type::Never),
            "Array" | "List" => Some(Type::Array(Box::new(Type::Unknown))),
            _ => None,
        }
    }

    fn lookup_this_field_type(&self, field: &str) -> Option<Type> {
        if let Some(this_type) = self.env.get_variable("this") {
            match this_type.non_nullable() {
                Type::Struct { fields, .. } => fields.get(field).cloned(),
                _ => None,
            }
        } else {
            None
        }
    }

    fn substitute_generics(&self, ty: &Type, mapping: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Generic(name) => mapping.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Array(inner) => Type::Array(Box::new(self.substitute_generics(inner, mapping))),
            Type::Map {
                key_type,
                value_type,
            } => Type::Map {
                key_type: Box::new(self.substitute_generics(key_type, mapping)),
                value_type: Box::new(self.substitute_generics(value_type, mapping)),
            },
            Type::Nullable(inner) => {
                Type::Nullable(Box::new(self.substitute_generics(inner, mapping)))
            }
            Type::Struct {
                name,
                fields,
                generics,
            } => {
                let new_fields = fields
                    .iter()
                    .map(|(field, ty)| (field.clone(), self.substitute_generics(ty, mapping)))
                    .collect();
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Struct {
                    name: name.clone(),
                    fields: new_fields,
                    generics: new_generics,
                }
            }
            Type::Enum {
                name,
                variants,
                generics,
            } => {
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Enum {
                    name: name.clone(),
                    variants: variants.clone(),
                    generics: new_generics,
                }
            }
            Type::Interface {
                name,
                methods,
                generics,
                is_sealed,
            } => {
                let new_generics = generics
                    .iter()
                    .map(|g| self.substitute_generics(g, mapping))
                    .collect();
                Type::Interface {
                    name: name.clone(),
                    methods: methods.clone(),
                    generics: new_generics,
                    is_sealed: *is_sealed,
                }
            }
            Type::Function {
                params,
                return_type,
                is_async,
            } => {
                let new_params: Vec<Type> = params
                    .iter()
                    .map(|p| self.substitute_generics(p, mapping))
                    .collect();
                let new_return = self.substitute_generics(return_type, mapping);
                Type::Function {
                    params: new_params,
                    return_type: Box::new(new_return),
                    is_async: *is_async,
                }
            }
            _ => ty.clone(),
        }
    }
}
