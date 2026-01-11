use seen_ir::instruction::{Instruction, ScopeKind};
use seen_ir::module::IRModule;
use seen_ir::{HardwareProfile, HardwareSchedulerHint, IRFunction, IRProgram, IRType};
use std::collections::BTreeMap;

const DIALECT_ATTR: &str =
    "#mlir.dialect_array<[\"arith\", \"cf\", \"func\", \"seen\", \"transform\"]>";
const DEFAULT_TRANSFORM_PIPELINE: &str = "builtin.pipeline(canonicalize,cse)";

fn module_attribute_block(profile: &HardwareProfile) -> String {
    let mut attrs = vec![format!("dialects = {}", DIALECT_ATTR)];
    if !profile.cpu_features.is_empty() {
        let quoted = profile
            .cpu_features
            .iter()
            .map(|feat| format!("\"{}\"", feat))
            .collect::<Vec<_>>()
            .join(", ");
        attrs.push(format!("seen.cpu_features = [{}]", quoted));
    }
    if let Some(bits) = profile.max_vector_bits {
        attrs.push(format!("seen.vector_width = {}", bits));
    }
    if profile.apx_enabled {
        attrs.push("seen.apx = true".to_string());
    }
    if profile.sve_enabled {
        attrs.push("seen.sve = true".to_string());
    }
    format!("module attributes {{ {} }} {{\n", attrs.join(", "))
}

/// Emit a minimal textual MLIR module for a full IR program.
pub fn program_to_mlir(program: &IRProgram) -> String {
    let mut modules: Vec<_> = program.modules.iter().collect();
    modules.sort_by(|a, b| a.name.cmp(&b.name));

    let mut module_sections = Vec::with_capacity(modules.len());
    for module in &modules {
        module_sections.push(indent_block(
            &module_to_mlir_with_profile(module, &program.hardware_profile),
            2,
        ));
    }

    let transform_section = indent_block(&transform_module_section(), 2);

    let mut out = String::new();
    out.push_str(&module_attribute_block(&program.hardware_profile));
    if !module_sections.is_empty() {
        out.push_str(&module_sections.join("\n"));
        out.push('\n');
    }
    out.push_str(&transform_section);
    out.push_str("\n}\n");
    out
}

/// Emit a textual MLIR module for a single IR module using a default (feature-less) profile.
pub fn module_to_mlir(module: &IRModule) -> String {
    module_to_mlir_with_profile(module, &HardwareProfile::default())
}

fn module_to_mlir_with_profile(module: &IRModule, profile: &HardwareProfile) -> String {
    let mut out = String::new();

    let module_name = sanitize_symbol(&module.name);
    out.push_str(&format!("module @{} {{\n", module_name));

    let mut functions: Vec<&IRFunction> = module.functions_iter().collect();
    functions.sort_by(|a, b| a.name.cmp(&b.name));

    for function in functions {
        let func_mlir = function_to_mlir(function, profile);
        out.push_str("  ");
        out.push_str(&func_mlir);
        out.push('\n');
    }

    out.push_str("}\n");
    out
}

fn transform_module_section() -> String {
    format!(
        "transform.module @seen_pipeline attributes {{ pipeline = \"{}\" }}",
        DEFAULT_TRANSFORM_PIPELINE
    )
}

fn function_to_mlir(function: &IRFunction, profile: &HardwareProfile) -> String {
    let mut out = String::new();

    out.push_str("func.func ");
    if !function.is_public {
        out.push_str("private ");
    }

    let sym_name = sanitize_symbol(&function.name);
    out.push_str(&format!("@{}(", sym_name));

    let params: Vec<String> = function
        .parameters
        .iter()
        .enumerate()
        .map(|(idx, param)| {
            let mlir_ty = type_to_mlir(&param.param_type);
            format!("%arg{}: {}", idx, mlir_ty)
        })
        .collect();
    out.push_str(&params.join(", "));
    out.push(')');

    let ret_ty = type_to_mlir(&function.return_type);
    if ret_ty != "()" {
        out.push_str(&format!(" -> {}", ret_ty));
    }

    let attrs = function_attribute_list(profile);
    if !attrs.is_empty() {
        out.push_str(" attributes { ");
        out.push_str(&attrs.join(", "));
        out.push_str(" }");
    }

    out.push_str(" {\n");

    let mut ordered_blocks: Vec<String> = if !function.cfg.block_order.is_empty() {
        function.cfg.block_order.clone()
    } else {
        function
            .cfg
            .blocks_iter()
            .map(|block| block.label.0.clone())
            .collect()
    };

    // Ensure all blocks are represented even if they were not in block_order.
    for block in function.cfg.blocks_iter() {
        if !ordered_blocks.contains(&block.label.0) {
            ordered_blocks.push(block.label.0.clone());
        }
    }

    let mut emit_order: Vec<String> = Vec::new();
    if let Some(entry) = &function.cfg.entry_block {
        if !emit_order.contains(entry) {
            emit_order.push(entry.clone());
        }
    }
    for name in ordered_blocks {
        if Some(&name) != function.cfg.entry_block.as_ref() && !emit_order.contains(&name) {
            emit_order.push(name);
        }
    }
    emit_order = reorder_blocks_for_profile(profile, emit_order);

    let mut ctx = MlirContext::default();

    for (idx, block_name) in emit_order.iter().enumerate() {
        let block = function
            .cfg
            .get_block(block_name)
            .expect("CFG block must exist");
        let sanitized = sanitize_label(&block.label.0);
        out.push_str(&format!("  ^{}:\n", sanitized));

        for instr in &block.instructions {
            let line = instruction_to_mlir(instr, &mut ctx, None);
            out.push_str("    ");
            out.push_str(&line);
            out.push('\n');
        }

        if let Some(term) = &block.terminator {
            let fallthrough = emit_order.get(idx + 1).map(|s| s.as_str());
            let line = instruction_to_mlir(term, &mut ctx, fallthrough);
            out.push_str("    ");
            out.push_str(&line);
            out.push('\n');
        }
    }

    if ctx.needs_unreachable_block {
        out.push_str("  ^mlir_unreachable:\n    cf.unreachable\n");
    }

    out.push_str("}\n");
    out
}

fn function_attribute_list(profile: &HardwareProfile) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(bits) = profile.max_vector_bits {
        attrs.push(format!("seen.vector_width = {}", bits));
    }
    attrs.push(format!(
        "seen.register_budget = {}",
        profile.register_budget_hint()
    ));
    attrs.push(format!(
        "seen.scheduler = \"{}\"",
        profile.scheduler_hint().as_str()
    ));
    attrs
}

fn reorder_blocks_for_profile(
    profile: &HardwareProfile,
    mut emit_order: Vec<String>,
) -> Vec<String> {
    if emit_order.len() <= 1 {
        return emit_order;
    }
    let entry = emit_order.remove(0);
    let mut rest = emit_order;
    match profile.scheduler_hint() {
        HardwareSchedulerHint::Balanced => rest.sort(),
        HardwareSchedulerHint::Throughput => { /* keep declared order */ }
        HardwareSchedulerHint::Vector => {
            rest.sort_by(|a, b| match (is_loop_label(a), is_loop_label(b)) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            });
        }
    }
    let mut scheduled = vec![entry];
    for name in rest {
        if !scheduled.contains(&name) {
            scheduled.push(name);
        }
    }
    scheduled
}

fn is_loop_label(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    lower.contains("loop") || lower.contains("for")
}

fn indent_block(text: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Default)]
struct MlirContext {
    needs_unreachable_block: bool,
}

fn instruction_to_mlir(
    inst: &Instruction,
    ctx: &mut MlirContext,
    fallthrough: Option<&str>,
) -> String {
    match inst {
        Instruction::Binary {
            op,
            left,
            right,
            result,
        } => {
            let res = value_result(result);
            let lhs = value_operand(left, ctx);
            let rhs = value_operand(right, ctx);
            let lhs_ty = value_type(left, None);
            let rhs_ty = value_type(right, Some(&lhs_ty));
            let res_ty = value_type(result, Some(&lhs_ty));
            format!(
                "{} = \"seen.binary\"({}, {}) {{op = \"{}\"}} : ({}, {}) -> {}",
                res,
                lhs,
                rhs,
                binary_op_name(op),
                lhs_ty,
                rhs_ty,
                res_ty
            )
        }
        Instruction::Unary {
            op,
            operand,
            result,
        } => {
            let res = value_result(result);
            let opnd = value_operand(operand, ctx);
            let ty = value_type(operand, None);
            let res_ty = value_type(result, Some(&ty));
            format!(
                "{} = \"seen.unary\"({}) {{op = \"{}\"}} : ({}) -> {}",
                res, opnd, op, ty, res_ty
            )
        }
        Instruction::Call {
            target,
            args,
            result,
            ..
        } => render_call(target, args, result.as_ref(), ctx),
        Instruction::Return(Some(value)) => {
            let operand = value_operand(value, ctx);
            let ty = value_type(value, None);
            format!("return {} : {}", operand, ty)
        }
        Instruction::Return(None) => "return".to_string(),
        Instruction::Jump(label) => {
            let target = sanitize_label(&label.0);
            format!("cf.br ^{}", target)
        }
        Instruction::JumpIf { condition, target } => {
            let cond = value_operand(condition, ctx);
            let true_dest = sanitize_label(&target.0);
            let false_dest = fallthrough.map(sanitize_label).unwrap_or_else(|| {
                ctx.needs_unreachable_block = true;
                "mlir_unreachable".to_string()
            });
            format!("cf.cond_br {}, ^{}, ^{}", cond, true_dest, false_dest)
        }
        Instruction::JumpIfNot { condition, target } => {
            let cond = value_operand(condition, ctx);
            let false_dest = sanitize_label(&target.0);
            let true_dest = fallthrough.map(sanitize_label).unwrap_or_else(|| {
                ctx.needs_unreachable_block = true;
                "mlir_unreachable".to_string()
            });
            format!("cf.cond_br {}, ^{}, ^{}", cond, true_dest, false_dest)
        }
        Instruction::Load { source, dest } => {
            let res = value_result(dest);
            let src = value_operand(source, ctx);
            let ptr_ty = value_type(source, None);
            let res_ty = value_type(dest, None);
            format!(
                "{} = \"seen.load\"({}) : ({}) -> {}",
                res, src, ptr_ty, res_ty
            )
        }
        Instruction::Store { value, dest } => {
            let val = value_operand(value, ctx);
            let val_ty = value_type(value, None);
            let dst = value_operand(dest, ctx);
            let dst_ty = value_type(dest, None);
            format!(
                "\"seen.store\"({}, {}) : ({}, {}) -> ()",
                val, dst, val_ty, dst_ty
            )
        }
        Instruction::Move { source, dest } => {
            let res = value_result(dest);
            let src = value_operand(source, ctx);
            let src_ty = value_type(source, None);
            let dst_ty = value_type(dest, Some(&src_ty));
            format!(
                "{} = \"seen.move\"({}) : ({}) -> {}",
                res, src, src_ty, dst_ty
            )
        }
        Instruction::Allocate { size, result } => {
            let res = value_result(result);
            let size_op = value_operand(size, ctx);
            let size_ty = value_type(size, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.alloc\"({}) : ({}) -> {}",
                res, size_op, size_ty, res_ty
            )
        }
        Instruction::Deallocate { pointer } => {
            let ptr = value_operand(pointer, ctx);
            let ptr_ty = value_type(pointer, None);
            format!("\"seen.free\"({}) : ({}) -> ()", ptr, ptr_ty)
        }
        Instruction::ArrayAccess {
            array,
            index,
            result,
            ..
        } => {
            let res = value_result(result);
            let arr = value_operand(array, ctx);
            let idx = value_operand(index, ctx);
            let arr_ty = value_type(array, None);
            let idx_ty = value_type(index, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.array.get\"({}, {}) : ({}, {}) -> {}",
                res, arr, idx, arr_ty, idx_ty, res_ty
            )
        }
        Instruction::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            let arr = value_operand(array, ctx);
            let idx = value_operand(index, ctx);
            let val = value_operand(value, ctx);
            let arr_ty = value_type(array, None);
            let idx_ty = value_type(index, None);
            let val_ty = value_type(value, None);
            format!(
                "\"seen.array.set\"({}, {}, {}) : ({}, {}, {}) -> ()",
                arr, idx, val, arr_ty, idx_ty, val_ty
            )
        }
        Instruction::ArrayLength { array, result } => {
            let res = value_result(result);
            let arr = value_operand(array, ctx);
            let arr_ty = value_type(array, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.array.length\"({}) : ({}) -> {}",
                res, arr, arr_ty, res_ty
            )
        }
        Instruction::FieldAccess {
            struct_val,
            field,
            result,
            ..
        } => {
            let res = value_result(result);
            let obj = value_operand(struct_val, ctx);
            let obj_ty = value_type(struct_val, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.field.get\"({}) {{field = \"{}\"}} : ({}) -> {}",
                res,
                obj,
                sanitize_symbol(field),
                obj_ty,
                res_ty
            )
        }
        Instruction::FieldSet {
            struct_val,
            field,
            value,
            ..
        } => {
            let obj = value_operand(struct_val, ctx);
            let val = value_operand(value, ctx);
            let obj_ty = value_type(struct_val, None);
            let val_ty = value_type(value, None);
            format!(
                "\"seen.field.set\"({}, {}) {{field = \"{}\"}} : ({}, {}) -> ()",
                obj,
                val,
                sanitize_symbol(field),
                obj_ty,
                val_ty
            )
        }
        Instruction::GetEnumTag { enum_value, result } => {
            let res = value_result(result);
            let val = value_operand(enum_value, ctx);
            let val_ty = value_type(enum_value, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.enum.tag\"({}) : ({}) -> {}",
                res, val, val_ty, res_ty
            )
        }
        Instruction::GetEnumField {
            enum_value,
            field_index,
            result,
        } => {
            let res = value_result(result);
            let val = value_operand(enum_value, ctx);
            let val_ty = value_type(enum_value, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.enum.field\"({}) {{field_index = {}}} : ({}) -> {}",
                res, val, field_index, val_ty, res_ty
            )
        }
        Instruction::Cast {
            value,
            target_type,
            result,
        } => {
            let res = value_result(result);
            let val = value_operand(value, ctx);
            let val_ty = value_type(value, None);
            let tgt_ty = type_to_mlir(target_type);
            format!(
                "{} = \"seen.cast\"({}) : ({}) -> {}",
                res, val, val_ty, tgt_ty
            )
        }
        Instruction::TypeCheck {
            value,
            target_type,
            result,
        } => {
            let res = value_result(result);
            let val = value_operand(value, ctx);
            let val_ty = value_type(value, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.typecheck\"({}) {{type = \"{}\"}} : ({}) -> {}",
                res,
                val,
                type_to_mlir(target_type),
                val_ty,
                res_ty
            )
        }
        Instruction::StringConcat {
            left,
            right,
            result,
        } => {
            let res = value_result(result);
            let lhs = value_operand(left, ctx);
            let rhs = value_operand(right, ctx);
            let lhs_ty = value_type(left, None);
            let rhs_ty = value_type(right, Some(&lhs_ty));
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.string.concat\"({}, {}) : ({}, {}) -> {}",
                res, lhs, rhs, lhs_ty, rhs_ty, res_ty
            )
        }
        Instruction::StringLength { string, result } => {
            let res = value_result(result);
            let s = value_operand(string, ctx);
            let s_ty = value_type(string, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.string.length\"({}) : ({}) -> {}",
                res, s, s_ty, res_ty
            )
        }
        Instruction::SimdSplat {
            scalar,
            lane_type,
            lanes,
            result,
        } => {
            let res = value_result(result);
            let operand = value_operand(scalar, ctx);
            let operand_ty = value_type(scalar, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.simd.splat\"({}) {{ lanes = {}, element_type = \"{}\" }} : ({}) -> {}",
                res,
                operand,
                lanes,
                type_to_mlir(lane_type),
                operand_ty,
                res_ty
            )
        }
        Instruction::SimdReduceAdd {
            vector,
            lane_type,
            result,
        } => {
            let res = value_result(result);
            let operand = value_operand(vector, ctx);
            let operand_ty = value_type(vector, None);
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.simd.reduce_add\"({}) {{ element_type = \"{}\" }} : ({}) -> {}",
                res,
                operand,
                type_to_mlir(lane_type),
                operand_ty,
                res_ty
            )
        }
        Instruction::PushFrame => "\"seen.push_frame\"() : () -> ()".to_string(),
        Instruction::PopFrame => "\"seen.pop_frame\"() : () -> ()".to_string(),
        Instruction::VirtualCall {
            receiver,
            method_name,
            args,
            result,
            ..
        } => render_dispatch_call(
            "seen.virtual_call",
            Some(receiver),
            method_name,
            args,
            result.as_ref(),
            ctx,
        ),
        Instruction::StaticCall {
            class_name,
            method_name,
            args,
            result,
            ..
        } => render_static_call(class_name, method_name, args, result.as_ref(), ctx),
        Instruction::ConstructObject {
            class_name,
            args,
            result,
            ..
        } => {
            let res = value_result(result);
            let operands: Vec<String> = args.iter().map(|a| value_operand(a, ctx)).collect();
            let types: Vec<String> = args.iter().map(|a| value_type(a, None)).collect();
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.construct.object\"({}) {{class = \"{}\"}} : ({}) -> {}",
                res,
                operands.join(", "),
                sanitize_symbol(class_name),
                types.join(", "),
                res_ty
            )
        }
        Instruction::ConstructEnum {
            enum_name,
            variant_name,
            fields,
            result,
            ..
        } => {
            let res = value_result(result);
            let operands: Vec<String> = fields.iter().map(|f| value_operand(f, ctx)).collect();
            let types: Vec<String> = fields.iter().map(|f| value_type(f, None)).collect();
            let res_ty = value_type(result, None);
            format!(
                "{} = \"seen.construct.enum\"({}) {{enum = \"{}\", variant = \"{}\"}} : ({}) -> {}",
                res,
                operands.join(", "),
                sanitize_symbol(enum_name),
                sanitize_symbol(variant_name),
                types.join(", "),
                res_ty
            )
        }
        Instruction::Print(value) => {
            let val = value_operand(value, ctx);
            let ty = value_type(value, None);
            format!("\"seen.print\"({}) : ({}) -> ()", val, ty)
        }
        Instruction::Debug { message, value } => {
            let escaped = escape_string_literal(message);
            if let Some(val) = value {
                let opnd = value_operand(val, ctx);
                let ty = value_type(val, None);
                format!(
                    "\"seen.debug\"({}) {{message = {}}} : ({}) -> ()",
                    opnd, escaped, ty
                )
            } else {
                format!("\"seen.debug\"() {{message = {}}} : () -> ()", escaped)
            }
        }
        Instruction::Scoped { kind, result, .. } => {
            let res_name = value_result(result);
            let res_ty = value_type(result, None);
            let kind_str = match kind {
                ScopeKind::Task => "task",
                ScopeKind::Jobs => "jobs",
            };
            format!(
                "{} = \"seen.scope\"() {{kind = \"{}\"}} : () -> {}",
                res_name, kind_str, res_ty
            )
        }
        Instruction::Spawn {
            detached, result, ..
        } => {
            let res_name = value_result(result);
            let res_ty = value_type(result, None);
            let mode = if *detached { "detached" } else { "scoped" };
            format!(
                "{} = \"seen.spawn\"() {{mode = \"{}\"}} : () -> {}",
                res_name, mode, res_ty
            )
        }
        Instruction::ChannelSelect {
            cases,
            payload_result,
            ..
        } => {
            let res_name = value_result(payload_result);
            let res_ty = value_type(payload_result, None);
            format!(
                "{} = \"seen.select\"() {{cases = {}}} : () -> {}",
                res_name,
                cases.len(),
                res_ty
            )
        }
        Instruction::Label(label) => format!("// label {}", sanitize_label(&label.0)),
        Instruction::Nop => "\"seen.nop\"() : () -> ()".to_string(),
    }
}

fn binary_op_name(op: &seen_ir::instruction::BinaryOp) -> &'static str {
    use seen_ir::instruction::BinaryOp;
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Subtract => "sub",
        BinaryOp::Multiply => "mul",
        BinaryOp::Divide => "div",
        BinaryOp::Modulo => "mod",
        BinaryOp::Equal => "eq",
        BinaryOp::NotEqual => "ne",
        BinaryOp::LessThan => "lt",
        BinaryOp::LessEqual => "le",
        BinaryOp::GreaterThan => "gt",
        BinaryOp::GreaterEqual => "ge",
        BinaryOp::And => "and",
        BinaryOp::Or => "or",
        BinaryOp::BitwiseAnd => "band",
        BinaryOp::BitwiseOr => "bor",
        BinaryOp::BitwiseXor => "bxor",
        BinaryOp::LeftShift => "shl",
        BinaryOp::RightShift => "shr",
    }
}

fn value_operand(value: &seen_ir::IRValue, ctx: &mut MlirContext) -> String {
    use seen_ir::IRValue;
    match value {
        IRValue::Register(idx) => format!("%r{}", idx),
        IRValue::Variable(name) => format!("%{}", sanitize_symbol(name)),
        IRValue::Function { name, .. } => format!("@{}", sanitize_symbol(name)),
        IRValue::GlobalVariable(name) => format!("@{}", sanitize_symbol(name)),
        IRValue::Label(label) => format!("^{}", sanitize_label(label)),
        IRValue::Integer(i) => format!("#seen.int<{}>", i),
        IRValue::Float(f) => format!("#seen.float<{}>", f),
        IRValue::Boolean(b) => format!("#seen.bool<{}>", if *b { "true" } else { "false" }),
        IRValue::Char(c) => format!("#seen.char<'{}'>", escape_char_literal(*c)),
        IRValue::StringConstant(idx) => format!("#seen.strref<{}>", idx),
        IRValue::SizeOf(ty) => format!("#seen.sizeof<{:?}>", ty),
        IRValue::Null => "#seen.null".to_string(),
        IRValue::Undefined => "#seen.undef".to_string(),
        IRValue::Void => "#seen.void".to_string(),
        IRValue::String(s) => format!("#seen.str<{}>", escape_string_literal(s)),
        IRValue::ByteArray(bytes) => format!("#seen.bytes<{}>", hex_literal(bytes)),
        IRValue::Array(elements) => {
            let items: Vec<String> = elements.iter().map(|e| value_operand(e, ctx)).collect();
            format!("#seen.array<[{}]>", items.join(", "))
        }
        IRValue::Struct { type_name, fields } => {
            let mut ordered = BTreeMap::new();
            for (field, val) in fields {
                ordered.insert(field, val);
            }
            let mut parts = Vec::new();
            for (field, val) in ordered {
                parts.push(format!(
                    "{} = {}",
                    sanitize_symbol(field),
                    value_operand(val, ctx)
                ));
            }
            format!(
                "#seen.struct<{}{{{}}}>",
                sanitize_symbol(type_name),
                parts.join(", ")
            )
        }
        IRValue::AddressOf(inner) => {
            let inner = value_operand(inner, ctx);
            format!("#seen.addrof<{}>", inner)
        }
    }
}

fn value_result(value: &seen_ir::IRValue) -> String {
    use seen_ir::IRValue;
    match value {
        IRValue::Register(idx) => format!("%r{}", idx),
        IRValue::Variable(name) => format!("%{}", sanitize_symbol(name)),
        _ => "%res".to_string(),
    }
}

fn value_type(value: &seen_ir::IRValue, fallback: Option<&String>) -> String {
    let ty = value.get_type();
    if let IRType::Generic(name) = &ty {
        if name == "Unresolved" {
            if let Some(fb) = fallback {
                return fb.clone();
            }
            return "!seen.unknown".to_string();
        }
    }
    type_to_mlir(&ty)
}

fn render_call(
    target: &seen_ir::IRValue,
    args: &[seen_ir::IRValue],
    result: Option<&seen_ir::IRValue>,
    ctx: &mut MlirContext,
) -> String {
    let arg_operands: Vec<String> = args.iter().map(|arg| value_operand(arg, ctx)).collect();
    let arg_types: Vec<String> = args.iter().map(|arg| value_type(arg, None)).collect();

    let callee_symbol = match target {
        seen_ir::IRValue::Function { name, .. } => Some(sanitize_symbol(name)),
        seen_ir::IRValue::GlobalVariable(name) => Some(sanitize_symbol(name)),
        _ => None,
    };

    if let Some(symbol) = callee_symbol {
        if let Some(res) = result {
            let res_name = value_result(res);
            let res_ty = value_type(res, None);
            format!(
                "{} = func.call @{}({}) : ({}) -> {}",
                res_name,
                symbol,
                arg_operands.join(", "),
                arg_types.join(", "),
                res_ty
            )
        } else {
            format!(
                "func.call @{}({}) : ({}) -> ()",
                symbol,
                arg_operands.join(", "),
                arg_types.join(", ")
            )
        }
    } else {
        let mut operands = Vec::with_capacity(args.len() + 1);
        let mut types = Vec::with_capacity(args.len() + 1);
        operands.push(value_operand(target, ctx));
        types.push(value_type(target, None));
        operands.extend(arg_operands);
        types.extend(arg_types);

        if let Some(res) = result {
            let res_name = value_result(res);
            let res_ty = value_type(res, None);
            format!(
                "{} = \"seen.indirect_call\"({}) : ({}) -> {}",
                res_name,
                operands.join(", "),
                types.join(", "),
                res_ty
            )
        } else {
            format!(
                "\"seen.indirect_call\"({}) : ({}) -> ()",
                operands.join(", "),
                types.join(", ")
            )
        }
    }
}

fn render_dispatch_call(
    op_name: &str,
    receiver: Option<&seen_ir::IRValue>,
    method_name: &str,
    args: &[seen_ir::IRValue],
    result: Option<&seen_ir::IRValue>,
    ctx: &mut MlirContext,
) -> String {
    let mut operands = Vec::new();
    let mut types = Vec::new();

    if let Some(rcv) = receiver {
        operands.push(value_operand(rcv, ctx));
        types.push(value_type(rcv, None));
    }

    operands.extend(args.iter().map(|arg| value_operand(arg, ctx)));
    types.extend(args.iter().map(|arg| value_type(arg, None)));

    let method = sanitize_symbol(method_name);

    if let Some(res) = result {
        let res_name = value_result(res);
        let res_ty = value_type(res, None);
        format!(
            "{} = \"{}\"({}) {{method = \"{}\"}} : ({}) -> {}",
            res_name,
            op_name,
            operands.join(", "),
            method,
            types.join(", "),
            res_ty
        )
    } else {
        format!(
            "\"{}\"({}) {{method = \"{}\"}} : ({}) -> ()",
            op_name,
            operands.join(", "),
            method,
            types.join(", ")
        )
    }
}

fn render_static_call(
    class_name: &str,
    method_name: &str,
    args: &[seen_ir::IRValue],
    result: Option<&seen_ir::IRValue>,
    ctx: &mut MlirContext,
) -> String {
    let operands: Vec<String> = args.iter().map(|arg| value_operand(arg, ctx)).collect();
    let types: Vec<String> = args.iter().map(|arg| value_type(arg, None)).collect();
    let class = sanitize_symbol(class_name);
    let method = sanitize_symbol(method_name);

    if let Some(res) = result {
        let res_name = value_result(res);
        let res_ty = value_type(res, None);
        format!(
            "{} = \"seen.static_call\"({}) {{class = \"{}\", method = \"{}\"}} : ({}) -> {}",
            res_name,
            operands.join(", "),
            class,
            method,
            types.join(", "),
            res_ty
        )
    } else {
        format!(
            "\"seen.static_call\"({}) {{class = \"{}\", method = \"{}\"}} : ({}) -> ()",
            operands.join(", "),
            class,
            method,
            types.join(", ")
        )
    }
}

fn type_to_mlir(ty: &IRType) -> String {
    match ty {
        IRType::Void => "()".to_string(),
        IRType::Integer => "i64".to_string(),
        IRType::Float => "f64".to_string(),
        IRType::Boolean => "i1".to_string(),
        IRType::Char => "i8".to_string(),
        IRType::String => "!seen.str".to_string(),
        IRType::Pointer(inner) | IRType::Reference(inner) => {
            format!("{}*", type_to_mlir(inner))
        }
        IRType::Array(inner) => format!("!seen.array<{}>", type_to_mlir(inner)),
        IRType::Function {
            parameters,
            return_type,
        } => {
            let params: Vec<String> = parameters.iter().map(type_to_mlir).collect();
            format!("({}) -> {}", params.join(", "), type_to_mlir(return_type))
        }
        IRType::Struct { name, .. } => format!("!seen.struct<{}>", sanitize_symbol(name)),
        IRType::Enum { name, .. } => format!("!seen.enum<{}>", sanitize_symbol(name)),
        IRType::Optional(inner) => format!("!seen.optional<{}>", type_to_mlir(inner)),
        IRType::Generic(name) => format!("!seen.generic<{}>", sanitize_symbol(name)),
        IRType::Vector { lanes, lane_type } => {
            format!("vector<{}x{}>", lanes, type_to_mlir(lane_type))
        }
    }
}

fn sanitize_symbol(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "anonymous".to_string()
    } else {
        out
    }
}

fn sanitize_label(label: &str) -> String {
    sanitize_symbol(label)
}

fn escape_string_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        for esc in ch.escape_default() {
            escaped.push(esc);
        }
    }
    escaped.push('"');
    escaped
}

fn escape_char_literal(value: char) -> String {
    value.escape_default().collect()
}

fn hex_literal(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "0x".to_string();
    }
    let hex_bytes: Vec<String> = bytes.iter().map(|b| format!("{:02X}", b)).collect();
    format!("0x{}", hex_bytes.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_ir::instruction::{BasicBlock, BinaryOp, Instruction, Label};
    use seen_ir::{module::IRModule, IRFunction, IRProgram, IRType, IRValue, Parameter};

    #[test]
    fn emits_basic_module() {
        let mut module = IRModule::new("test.mod");
        let mut func = IRFunction::new("add", IRType::Integer).public();
        func.add_parameter(Parameter::new("lhs", IRType::Integer));
        func.add_parameter(Parameter::new("rhs", IRType::Integer));

        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.instructions.push(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(0),
            right: IRValue::Register(1),
            result: IRValue::Register(2),
        });
        entry.terminator = Some(Instruction::Return(Some(IRValue::Register(2))));
        func.cfg.add_block(entry);

        module.add_function(func);
        let mlir = module_to_mlir(&module);
        let expected = "module @test_mod {\n  func.func @add(%arg0: i64, %arg1: i64) -> i64 {\n  ^entry:\n    %r2 = \"seen.binary\"(%r0, %r1) {op = \"add\"} : (!seen.unknown, !seen.unknown) -> !seen.unknown\n    return %r2 : !seen.unknown\n}\n\n}\n";
        assert_eq!(mlir, expected);
    }

    #[test]
    fn lowers_branch_and_direct_call() {
        let mut module = IRModule::new("control");
        let mut func = IRFunction::new("branch", IRType::Integer).public();
        func.add_parameter(Parameter::new("cond", IRType::Boolean));
        func.add_parameter(Parameter::new("input", IRType::Integer));

        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.terminator = Some(Instruction::JumpIf {
            condition: IRValue::Variable("cond".into()),
            target: Label::new("then"),
        });
        func.cfg.add_block(entry);

        let mut else_block = BasicBlock::new(Label::new("else"));
        else_block.terminator = Some(Instruction::Return(Some(IRValue::Integer(0))));
        func.cfg.add_block(else_block);

        let mut then_block = BasicBlock::new(Label::new("then"));
        let call_result = IRValue::Register(0);
        then_block.instructions.push(Instruction::Call {
            target: IRValue::Function {
                name: "helper".into(),
                parameters: vec!["input".into()],
            },
            args: vec![IRValue::Variable("input".into())],
            result: Some(call_result.clone()),
            arg_types: None,
            return_type: None,
        });
        then_block.terminator = Some(Instruction::Return(Some(call_result)));
        func.cfg.add_block(then_block);

        module.add_function(func);
        let mlir = module_to_mlir(&module);
        assert!(mlir.contains("cf.cond_br %cond, ^then, ^else"));
        assert!(mlir.contains("%r0 = func.call @helper(%input) : (!seen.unknown) -> !seen.unknown"));
    }

    #[test]
    fn lowers_jump_if_not_and_side_effects() {
        let mut module = IRModule::new("effects");
        let mut func = IRFunction::new("maybe_print", IRType::Void).public();
        func.add_parameter(Parameter::new("flag", IRType::Boolean));

        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.terminator = Some(Instruction::JumpIfNot {
            condition: IRValue::Variable("flag".into()),
            target: Label::new("exit"),
        });
        func.cfg.add_block(entry);

        let mut body = BasicBlock::new(Label::new("body"));
        body.instructions
            .push(Instruction::Print(IRValue::String("hit".into())));
        body.terminator = Some(Instruction::Return(None));
        func.cfg.add_block(body);

        let mut exit = BasicBlock::new(Label::new("exit"));
        exit.terminator = Some(Instruction::Return(None));
        func.cfg.add_block(exit);

        module.add_function(func);
        let mlir = module_to_mlir(&module);
        assert!(mlir.contains("cf.cond_br %flag, ^body, ^exit"));
        assert!(mlir.contains("\"seen.print\"(#seen.str<\"hit\">) : (!seen.str) -> ()"));
        assert!(!mlir.contains("mlir_unreachable"));
    }

    #[test]
    fn program_emits_modules_sorted() {
        let mut program = IRProgram::new();

        let mut east = IRModule::new("east");
        let mut foo = IRFunction::new("foo", IRType::Void);
        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.terminator = Some(Instruction::Return(None));
        foo.cfg.add_block(entry);
        east.add_function(foo);
        program.add_module(east);

        let mut west = IRModule::new("west");
        let mut bar = IRFunction::new("bar", IRType::Boolean);
        let mut entry = BasicBlock::new(Label::new("entry"));
        entry.terminator = Some(Instruction::Return(Some(IRValue::Boolean(true))));
        bar.cfg.add_block(entry);
        west.add_function(bar);
        program.add_module(west);

        let mlir = program_to_mlir(&program);
        assert!(mlir.contains("module @east"));
        assert!(mlir.contains("module @west"));
        assert!(mlir.find("module @east").unwrap() < mlir.find("module @west").unwrap());
    }
}
