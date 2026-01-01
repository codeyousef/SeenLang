use crate::{
    module::{IRModule, TypeAlias, TypeDefinition},
    value::IRType,
    IRResult,
};
use super::IRGenerator;
use seen_parser::{ClassField, EnumVariant, ImportSymbol, InterfaceMethod, Method, StructField, Type};

impl IRGenerator {
    // ==================== DRY Helpers ====================

    /// Register a type in both the context and the module.
    fn register_type(&mut self, module: &mut IRModule, name: &str, ir_type: IRType) {
        self.context
            .type_definitions
            .insert(name.to_string(), ir_type.clone());
        let type_def = TypeDefinition::new(name, ir_type);
        module.add_type(type_def);
    }

    /// Register a class type in both the context and the module.
    /// Classes are heap-allocated and represented as pointers (i64) at runtime.
    fn register_class_type_def(&mut self, module: &mut IRModule, name: &str, ir_type: IRType) {
        eprintln!("[DEBUG IR] register_class_type_def: '{}' as class", name);
        self.context
            .type_definitions
            .insert(name.to_string(), ir_type.clone());
        let type_def = TypeDefinition::new(name, ir_type).as_class();
        module.add_type(type_def);
    }

    /// Convert struct-like fields (from StructField or ClassField) to IR type tuples.
    fn convert_struct_fields(&self, fields: &[StructField]) -> Vec<(String, IRType)> {
        fields
            .iter()
            .map(|f| (f.name.clone(), self.convert_ast_type_to_ir(&f.field_type)))
            .collect()
    }

    /// Convert class fields to IR type tuples.
    fn convert_class_fields(&self, fields: &[ClassField]) -> Vec<(String, IRType)> {
        fields
            .iter()
            .map(|f| (f.name.clone(), self.convert_ast_type_to_ir(&f.field_type)))
            .collect()
    }

    // ==================== Type Conversion ====================

    pub(crate) fn convert_ast_type_to_ir(&self, ast_type: &Type) -> IRType {
        let base_type = match ast_type.name.as_str() {
            "Int" => IRType::Integer,
            "Float" => IRType::Float,
            "Bool" => IRType::Boolean,
            "String" => IRType::String,
            "Char" => IRType::Char,
            "()" => IRType::Void,
            // Array and List are treated as dynamic arrays
            // NOTE: Vec is NOT included here because Vec<T> is a custom class with fields
            "Array" | "List" => {
                if let Some(element_ast_type) = ast_type.generics.first() {
                    let element_type = self.convert_ast_type_to_ir(element_ast_type);
                    IRType::Array(Box::new(element_type))
                } else {
                    IRType::Array(Box::new(IRType::Integer))
                }
            }
            // Vec<T> is a class defined in seen_std, NOT a builtin array type.
            // It must be treated as a Struct so that method calls work correctly.
            "Vec" => {
                let _generic_types: Vec<IRType> = ast_type.generics.iter()
                    .map(|g| self.convert_ast_type_to_ir(g))
                    .collect();
                IRType::Struct {
                    name: "Vec".to_string(),
                    fields: Vec::new(),  // Fields will be filled in by class definition
                }
            }
            // Option<T> wraps T
            "Option" => {
                if let Some(inner_ast_type) = ast_type.generics.first() {
                    let inner_type = self.convert_ast_type_to_ir(inner_ast_type);
                    IRType::Optional(Box::new(inner_type))
                } else {
                    IRType::Struct {
                        name: "Option".to_string(),
                        fields: Vec::new(),
                    }
                }
            }
            "Map" | "Result" | "StringBuilder" => IRType::Struct {
                name: ast_type.name.clone(),
                fields: Vec::new(),
            },
            _ => {
                if let Some(ir_type) = self.context.type_definitions.get(&ast_type.name) {
                    ir_type.clone()
                } else if Some(&ast_type.name) == self.context.current_type_definition.as_ref() {
                    IRType::Struct {
                        name: ast_type.name.clone(),
                        fields: Vec::new(),
                    }
                } else {
                    IRType::Struct {
                        name: ast_type.name.clone(),
                        fields: Vec::new(),
                    }
                }
            }
        };

        if ast_type.is_nullable {
            IRType::Optional(Box::new(base_type))
        } else {
            base_type
        }
    }

    pub(crate) fn register_import_types(
        &mut self,
        module: &mut IRModule,
        module_path: &str,
        _symbols: &Vec<ImportSymbol>,
    ) -> IRResult<()> {
        match module_path {
            "str.string" | "seen_std.str.string" => {
                let sb_type = IRType::Struct {
                    name: "StringBuilder".to_string(),
                    fields: vec![
                        ("parts".to_string(), IRType::Array(Box::new(IRType::String))),
                        ("totalLength".to_string(), IRType::Integer),
                    ],
                };
                self.context
                    .type_definitions
                    .insert("StringBuilder".to_string(), sb_type.clone());
                // StringBuilder is a class (heap-allocated), not a value type
                let type_def = TypeDefinition::new("StringBuilder", sb_type).as_class();
                module.add_type(type_def);
            }
            "seen_std.collections.string_hash_map" => {
                let shm_type = IRType::Struct {
                    name: "StringHashMap".to_string(),
                    fields: vec![("length".to_string(), IRType::Integer)],
                };
                self.context
                    .type_definitions
                    .insert("StringHashMap".to_string(), shm_type.clone());
                let type_def = TypeDefinition::new("StringHashMap", shm_type);
                module.add_type(type_def);
            }
            "collections.vec" | "seen_std.collections.vec" => {}
            "core.option" | "seen_std.core.option" => {
                let option_type = IRType::Struct {
                    name: "Option".to_string(),
                    fields: vec![
                        ("hasValue".to_string(), IRType::Boolean),
                        ("value".to_string(), IRType::Integer),
                    ],
                };
                self.context
                    .type_definitions
                    .insert("Option".to_string(), option_type.clone());
                let type_def = TypeDefinition::new("Option", option_type);
                module.add_type(type_def);
            }
            "core.result" | "seen_std.core.result" => {
                let result_type = IRType::Struct {
                    name: "Result".to_string(),
                    fields: vec![
                        ("isOk".to_string(), IRType::Boolean),
                        ("okStorage".to_string(), IRType::Array(Box::new(IRType::Integer))),
                        ("errStorage".to_string(), IRType::Array(Box::new(IRType::String))),
                    ],
                };
                self.context
                    .type_definitions
                    .insert("Result".to_string(), result_type.clone());
                let type_def = TypeDefinition::new("Result", result_type);
                module.add_type(type_def);
            }
            "ffi.cinterop" | "seen_std.ffi.cinterop" => {}
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn register_struct_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        fields: &[StructField],
    ) -> IRResult<()> {
        self.context.current_type_definition = Some(name.to_string());
        let ir_fields = self.convert_struct_fields(fields);
        self.context.current_type_definition = None;

        let struct_type = IRType::Struct {
            name: name.to_string(),
            fields: ir_fields,
        };
        self.register_type(module, name, struct_type);
        Ok(())
    }

    pub(crate) fn register_enum_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        variants: &[EnumVariant],
    ) -> IRResult<()> {
        let ir_variants: Vec<_> = variants
            .iter()
            .map(|v| {
                let variant_fields = v.fields.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|f| self.convert_ast_type_to_ir(&f.type_annotation))
                        .collect()
                });
                (v.name.clone(), variant_fields)
            })
            .collect();

        let enum_type = IRType::Enum {
            name: name.to_string(),
            variants: ir_variants,
        };
        self.register_type(module, name, enum_type);
        
        // Register implicit toString() method for enums that returns String
        let to_string_underscore = format!("{}_toString", name);
        let to_string_dot = format!("{}.toString", name);
        self.context.function_return_types.insert(to_string_underscore, IRType::String);
        self.context.function_return_types.insert(to_string_dot, IRType::String);
        
        Ok(())
    }

    pub(crate) fn register_class_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        fields: &[ClassField],
        methods: &[Method],
    ) -> IRResult<()> {
        self.context.current_type_definition = Some(name.to_string());
        let class_fields = self.convert_class_fields(fields);
        self.context.current_type_definition = None;

        let class_type = IRType::Struct {
            name: name.to_string(),
            fields: class_fields,
        };
        // Use register_class_type_def to mark this as a class (heap-allocated)
        self.register_class_type_def(module, name, class_type);

        for method in methods {
            let mangled_name = format!("{}_{}", name, method.name);
            let ir_return_type = if let Some(ret_type) = &method.return_type {
                self.convert_ast_type_to_ir(ret_type)
            } else {
                IRType::Void
            };
            self.context
                .function_return_types
                .insert(mangled_name, ir_return_type);
        }
        Ok(())
    }

    pub(crate) fn generate_class_methods(
        &mut self,
        module: &mut IRModule,
        name: &str,
        methods: &[Method],
    ) -> IRResult<()> {
        // Set the current type definition so method bodies can resolve bare method calls
        let old_type_definition = self.context.current_type_definition.clone();
        self.context.current_type_definition = Some(name.to_string());
        
        for method in methods {
            let is_static = method.is_static;
            // In Seen, 'new' methods are factory methods that allocate internally
            // They should NOT receive a 'this' parameter - they create and return a new instance
            let is_factory_constructor = method.name == "new";
            let mut effective_params: Vec<seen_parser::Parameter> = Vec::new();
            let mut receiver_name = "self".to_string();
            
            if is_static || is_factory_constructor {
                // Static methods and factory constructors: NO receiver parameter at all
                // Just use the method's own parameters
            } else {
                // Instance methods: add receiver as first parameter
                let recv_type = Type {
                    name: name.to_string(),
                    is_nullable: false,
                    generics: vec![],
                };
                receiver_name = method
                    .receiver
                    .as_ref()
                    .map(|r| if r.name == "self" { "this".to_string() } else { r.name.clone() })
                    .unwrap_or_else(|| "this".to_string());
                let recv = seen_parser::Parameter {
                    name: receiver_name.clone(),
                    type_annotation: Some(recv_type),
                    default_value: None,
                    memory_modifier: None,
                };
                effective_params.push(recv);
            }
            effective_params.extend(method.parameters.clone());

            // Set receiver name in context
            let old_receiver_name = self.context._current_receiver_name.clone();
            self.context._current_receiver_name = Some(receiver_name.clone());
            
            // For factory constructors, set up 'this' as a local variable that will be allocated
            // This allows constructors that use 'this.field = ...' syntax to work
            if is_factory_constructor {
                // Add 'this' to the context as a variable of the current type
                let this_type = IRType::Struct {
                    name: name.to_string(),
                    fields: vec![], // Fields will be filled in elsewhere
                };
                self.context.set_variable_type("this".to_string(), this_type.clone());
                self.context.set_variable_type("self".to_string(), this_type);
            }

            let mangled_name = format!("{}_{}", name, method.name);
            let mut function = self.generate_method_function(
                &mangled_name,
                &effective_params,
                &method.return_type,
                &method.body,
            )?;
            
            // For factory constructors, add 'this' as a local variable
            if is_factory_constructor {
                let this_type = IRType::Struct {
                    name: name.to_string(),
                    fields: vec![], // Fields will be filled in by LLVM backend from type definitions
                };
                function.add_local(crate::function::LocalVariable::new("this", this_type));
            }

            // Restore receiver name
            self.context._current_receiver_name = old_receiver_name;

            module.add_function(function);
        }
        
        // Restore the old type definition
        self.context.current_type_definition = old_type_definition;
        
        Ok(())
    }

    pub(crate) fn register_type_alias(
        &mut self,
        module: &mut IRModule,
        name: &str,
        target_type: &Type,
    ) -> IRResult<()> {
        let ir_target_type = self.convert_ast_type_to_ir(target_type);
        let alias_entry = TypeAlias {
            name: name.to_string(),
            target: ir_target_type.clone(),
            is_public: name.chars().next().unwrap().is_uppercase(),
        };

        module.add_type_alias(alias_entry);
        let type_def = TypeDefinition::new(name, ir_target_type);
        module.add_type(type_def);
        Ok(())
    }

    pub(crate) fn register_interface_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        methods: &[InterfaceMethod],
    ) -> IRResult<()> {
        let mut vtable_fields = Vec::new();
        let mut method_signatures = Vec::new();

        for method in methods {
            let param_types: Vec<_> = method
                .params
                .iter()
                .map(|p| {
                    p.type_annotation
                        .as_ref()
                        .map(|t| self.convert_ast_type_to_ir(t))
                        .unwrap_or_else(|| IRType::Generic("T".to_string()))
                })
                .collect();

            let return_type = method
                .return_type
                .as_ref()
                .map(|t| self.convert_ast_type_to_ir(t))
                .unwrap_or(IRType::Void);

            let method_func_type = IRType::Function {
                parameters: param_types,
                return_type: Box::new(return_type.clone()),
            };

            vtable_fields.push((method.name.clone(), method_func_type.clone()));
            method_signatures.push((method.name.clone(), method_func_type));
        }

        // Register vtable struct
        let vtable_struct_name = format!("{}__vtable", name);
        let vtable_struct_type = IRType::Struct {
            name: vtable_struct_name.clone(),
            fields: vtable_fields,
        };
        self.register_type(module, &vtable_struct_name, vtable_struct_type);

        // Register interface struct
        let interface_struct = IRType::Struct {
            name: name.to_string(),
            fields: vec![(
                "vtable".to_string(),
                IRType::Pointer(Box::new(IRType::Generic(vtable_struct_name))),
            )],
        };
        self.register_type(module, name, interface_struct);

        // Register method return types
        for (method_name, method_signature) in method_signatures {
            let function_name = format!("{}_{}", name, method_name);
            if let IRType::Function { return_type, .. } = method_signature {
                self.context
                    .function_return_types
                    .insert(function_name, *return_type);
            }
        }

        Ok(())
    }
}
