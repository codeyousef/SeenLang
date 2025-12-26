use crate::{
    module::{IRModule, TypeAlias, TypeDefinition},
    value::IRType,
    IRResult,
};
use super::IRGenerator;
use seen_parser::{ClassField, EnumVariant, ImportSymbol, InterfaceMethod, Method, StructField, Type};

impl IRGenerator {
    pub(crate) fn convert_ast_type_to_ir(&self, ast_type: &Type) -> IRType {
        let base_type = match ast_type.name.as_str() {
            "Int" => IRType::Integer,
            "Float" => IRType::Float,
            "Bool" => IRType::Boolean,
            "String" => IRType::String,
            "Char" => IRType::Char,
            "()" => IRType::Void,
            "Array" => {
                if let Some(element_ast_type) = ast_type.generics.first() {
                    let element_type = self.convert_ast_type_to_ir(element_ast_type);
                    IRType::Array(Box::new(element_type))
                } else {
                    IRType::Array(Box::new(IRType::Integer))
                }
            }
            "Map" | "Vec" | "Result" | "Option" | "StringBuilder" => IRType::Struct {
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
                let type_def = TypeDefinition::new("StringBuilder", sb_type);
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
        let mut ir_fields = Vec::new();
        for field in fields {
            let field_type = self.convert_ast_type_to_ir(&field.field_type);
            ir_fields.push((field.name.clone(), field_type));
        }
        self.context.current_type_definition = None;

        let struct_type = IRType::Struct {
            name: name.to_string(),
            fields: ir_fields,
        };
        self.context
            .type_definitions
            .insert(name.to_string(), struct_type.clone());
        let type_def = TypeDefinition::new(name, struct_type);
        module.add_type(type_def);
        Ok(())
    }

    pub(crate) fn register_enum_type(
        &mut self,
        module: &mut IRModule,
        name: &str,
        variants: &[EnumVariant],
    ) -> IRResult<()> {
        let mut ir_variants = Vec::new();
        for variant in variants {
            let variant_name = variant.name.clone();
            let variant_fields = if let Some(fields) = &variant.fields {
                let field_types: Vec<IRType> = fields
                    .iter()
                    .map(|field| self.convert_ast_type_to_ir(&field.type_annotation))
                    .collect();
                Some(field_types)
            } else {
                None
            };
            ir_variants.push((variant_name, variant_fields));
        }

        let enum_type = IRType::Enum {
            name: name.to_string(),
            variants: ir_variants,
        };
        self.context
            .type_definitions
            .insert(name.to_string(), enum_type.clone());
        let type_def = TypeDefinition::new(name, enum_type);
        module.add_type(type_def);
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
        let mut ir_fields = Vec::new();
        for field in fields {
            let field_type = self.convert_ast_type_to_ir(&field.field_type);
            ir_fields.push((field.name.clone(), field_type));
        }
        self.context.current_type_definition = None;

        let mut class_fields = Vec::new();
        class_fields.extend(ir_fields);

        let class_type = IRType::Struct {
            name: name.to_string(),
            fields: class_fields,
        };
        self.context
            .type_definitions
            .insert(name.to_string(), class_type.clone());
        let type_def = TypeDefinition::new(name, class_type);
        module.add_type(type_def);

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
        for method in methods {
            let is_constructor = method.is_static || method.name == "new";
            let mut effective_params: Vec<seen_parser::Parameter> = Vec::new();
            if !is_constructor {
                let recv_type = Type {
                    name: name.to_string(),
                    is_nullable: false,
                    generics: vec![],
                };
                let recv = seen_parser::Parameter {
                    name: method
                        .receiver
                        .as_ref()
                        .map(|r| r.name.clone())
                        .unwrap_or_else(|| "self".to_string()),
                    type_annotation: Some(recv_type),
                    default_value: None,
                    memory_modifier: None,
                };
                effective_params.push(recv);
            }
            effective_params.extend(method.parameters.clone());

            let mangled_name = format!("{}_{}", name, method.name);
            let function = self.generate_method_function(
                &mangled_name,
                &effective_params,
                &method.return_type,
                &method.body,
            )?;
            module.add_function(function);
        }
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
            let mut param_types = Vec::new();
            for param in &method.params {
                let param_type = if let Some(type_ann) = &param.type_annotation {
                    self.convert_ast_type_to_ir(type_ann)
                } else {
                    IRType::Generic("T".to_string())
                };
                param_types.push(param_type);
            }

            let return_type = if let Some(ret_type) = &method.return_type {
                self.convert_ast_type_to_ir(ret_type)
            } else {
                IRType::Void
            };

            let method_func_type = IRType::Function {
                parameters: param_types,
                return_type: Box::new(return_type.clone()),
            };

            vtable_fields.push((method.name.clone(), method_func_type.clone()));
            method_signatures.push((method.name.clone(), method_func_type));
        }

        let vtable_struct_name = format!("{}__vtable", name);
        let vtable_struct_type = IRType::Struct {
            name: vtable_struct_name.clone(),
            fields: vtable_fields,
        };
        self.context
            .type_definitions
            .insert(vtable_struct_name.clone(), vtable_struct_type.clone());
        let vtable_def = TypeDefinition::new(&vtable_struct_name, vtable_struct_type);
        module.add_type(vtable_def);

        let interface_struct = IRType::Struct {
            name: name.to_string(),
            fields: vec![
                ("vtable".to_string(), IRType::Pointer(Box::new(IRType::Generic(vtable_struct_name)))),
            ],
        };
        self.context
            .type_definitions
            .insert(name.to_string(), interface_struct.clone());
        let interface_def = TypeDefinition::new(name, interface_struct.clone());
        module.add_type(interface_def);

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
