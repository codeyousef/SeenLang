use seen_ir::instruction::{BinaryOp, Instruction};
use seen_ir::module::IRModule;
use seen_ir::{HardwareProfile, HardwareSchedulerHint, IRFunction, IRProgram, IRType, IRValue};

/// Emit a textual Cranelift IR representation for the entire program.
pub fn program_to_clif(program: &IRProgram) -> String {
    let mut modules: Vec<_> = program.modules.iter().collect();
    modules.sort_by(|a, b| a.name.cmp(&b.name));

    let mut sections = Vec::new();
    if let Some(header) = clif_profile_header(&program.hardware_profile) {
        sections.push(header);
    }
    sections.extend(
        modules
            .into_iter()
            .map(|m| module_to_clif_with_profile(m, &program.hardware_profile)),
    );
    sections.join("\n")
}

fn clif_profile_header(profile: &HardwareProfile) -> Option<String> {
    if profile.cpu_features.is_empty()
        && profile.max_vector_bits.is_none()
        && !profile.apx_enabled
        && !profile.sve_enabled
    {
        return None;
    }
    let mut lines = Vec::new();
    lines.push("; === hardware profile ===".to_string());
    if !profile.cpu_features.is_empty() {
        lines.push(format!(
            "; cpu-features: {}",
            profile.cpu_features.join(",")
        ));
    }
    if let Some(bits) = profile.max_vector_bits {
        lines.push(format!("; max-vector-bits: {}", bits));
    }
    if profile.apx_enabled {
        lines.push("; apx-enabled".to_string());
    }
    if profile.sve_enabled {
        lines.push("; sve-enabled".to_string());
    }
    Some(lines.join("\n"))
}

fn module_to_clif_with_profile(module: &IRModule, profile: &HardwareProfile) -> String {
    let mut out = String::new();
    out.push_str(&format!("; module {}\n", sanitize_symbol(&module.name)));
    let mut functions: Vec<&IRFunction> = module.functions_iter().collect();
    functions.sort_by(|a, b| a.name.cmp(&b.name));

    for function in functions {
        out.push_str(&function_to_clif(function, profile));
        out.push('\n');
    }

    out
}

#[allow(dead_code)]
fn module_to_clif(module: &IRModule) -> String {
    module_to_clif_with_profile(module, &HardwareProfile::default())
}

fn function_to_clif(function: &IRFunction, profile: &HardwareProfile) -> String {
    let mut out = String::new();
    let params: Vec<String> = function
        .parameters
        .iter()
        .enumerate()
        .map(|(idx, param)| format!("v{}: {}", idx, clif_type(&param.param_type)))
        .collect();
    let return_ty = clif_type(&function.return_type);
    out.push_str(&format!(
        "function %{}({}) -> {} {{\n",
        sanitize_symbol(&function.name),
        params.join(", "),
        return_ty
    ));

    if let Some(bits) = profile.max_vector_bits {
        out.push_str(&format!("  ; seen.vector_width = {}\n", bits));
    }
    out.push_str(&format!(
        "  ; seen.register_budget = {}\n",
        profile.register_budget_hint()
    ));
    out.push_str(&format!(
        "  ; seen.scheduler = {}\n",
        profile.scheduler_hint().as_str()
    ));

    let mut ordered_blocks: Vec<String> = if !function.cfg.block_order.is_empty() {
        function.cfg.block_order.clone()
    } else {
        function
            .cfg
            .blocks_iter()
            .map(|block| block.label.0.clone())
            .collect()
    };
    for block in function.cfg.blocks_iter() {
        if !ordered_blocks.contains(&block.label.0) {
            ordered_blocks.push(block.label.0.clone());
        }
    }

    let mut emit_order: Vec<String> = Vec::new();
    if let Some(entry) = &function.cfg.entry_block {
        emit_order.push(entry.clone());
    }
    for name in ordered_blocks {
        if Some(&name) != function.cfg.entry_block.as_ref() && !emit_order.contains(&name) {
            emit_order.push(name);
        }
    }
    emit_order = reorder_blocks_for_profile(profile, emit_order);

    for block_name in emit_order {
        let block = function
            .cfg
            .get_block(&block_name)
            .expect("block must exist");
        out.push_str(&format!("  {}:\n", sanitize_block(&block.label.0)));
        for inst in &block.instructions {
            out.push_str("    ");
            out.push_str(&instruction_to_clif(inst));
            out.push('\n');
        }
        if let Some(term) = &block.terminator {
            out.push_str("    ");
            out.push_str(&instruction_to_clif(term));
            out.push('\n');
        }
    }

    out.push_str("}\n");
    out
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
        HardwareSchedulerHint::Throughput => { /* keep insertion order */ }
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

fn instruction_to_clif(inst: &Instruction) -> String {
    match inst {
        Instruction::Binary {
            op,
            left,
            right,
            result,
        } => {
            let dst = value_result(result);
            let lhs = value_operand(left);
            let rhs = value_operand(right);
            match clif_binary_op(op) {
                Some((name, ty)) => format!("{dst} = {name}.{ty} {lhs}, {rhs}"),
                None => format!("; unsupported binary op {op:?} {lhs} {rhs}"),
            }
        }
        Instruction::Unary {
            operand, result, ..
        } => {
            let dst = value_result(result);
            let src = value_operand(operand);
            format!("{dst} = copy {src}")
        }
        Instruction::Move { source, dest } => {
            let dst = value_result(dest);
            let src = value_operand(source);
            format!("{dst} = copy {src}")
        }
        Instruction::Jump(label) => format!("br {}", sanitize_block(&label.0)),
        Instruction::JumpIf { condition, target } => {
            let cond = value_operand(condition);
            format!(
                "brif {cond}, {}, {}",
                sanitize_block(&target.0),
                "fallthrough"
            )
        }
        Instruction::JumpIfNot { condition, target } => {
            let cond = value_operand(condition);
            format!("brif {}, fallthrough, {}", cond, sanitize_block(&target.0))
        }
        Instruction::Return(Some(value)) => {
            let opnd = value_operand(value);
            format!("return {opnd}")
        }
        Instruction::Return(None) => "return".to_string(),
        Instruction::Call {
            target,
            args,
            result,
            ..
        } => {
            let callee = value_operand(target);
            let arg_list: Vec<String> = args.iter().map(value_operand).collect();
            if let Some(dest) = result {
                format!(
                    "{} = call {}({})",
                    value_result(dest),
                    callee,
                    arg_list.join(", ")
                )
            } else {
                format!("call {}({})", callee, arg_list.join(", "))
            }
        }
        Instruction::Label(_) => String::new(),
        other => format!("; unsupported {:?}", other),
    }
}

fn clif_binary_op(op: &BinaryOp) -> Option<(&'static str, &'static str)> {
    match op {
        BinaryOp::Add => Some(("iadd", "i64")),
        BinaryOp::Subtract => Some(("isub", "i64")),
        BinaryOp::Multiply => Some(("imul", "i64")),
        BinaryOp::Divide => Some(("sdiv", "i64")),
        BinaryOp::Modulo => Some(("srem", "i64")),
        BinaryOp::Equal => Some(("icmp", "eq.i64")),
        BinaryOp::NotEqual => Some(("icmp", "ne.i64")),
        BinaryOp::GreaterThan => Some(("icmp", "gt.i64")),
        BinaryOp::GreaterEqual => Some(("icmp", "ge.i64")),
        BinaryOp::LessThan => Some(("icmp", "lt.i64")),
        BinaryOp::LessEqual => Some(("icmp", "le.i64")),
        _ => None,
    }
}

fn value_operand(value: &IRValue) -> String {
    match value {
        IRValue::Register(idx) => format!("v{}", idx),
        IRValue::Integer(num) => format!("{}", num),
        IRValue::Boolean(true) => "true".to_string(),
        IRValue::Boolean(false) => "false".to_string(),
        IRValue::String(s) => format!("\"{s}\""),
        IRValue::Variable(name) => format!("%{}", sanitize_symbol(name)),
        IRValue::Function { name, .. } => format!("@{}", sanitize_symbol(name)),
        other => format!("; unsupported operand {:?}", other),
    }
}

fn value_result(value: &IRValue) -> String {
    match value {
        IRValue::Register(idx) => format!("v{}", idx),
        IRValue::Variable(name) => format!("%{}", sanitize_symbol(name)),
        _ => "_res".to_string(),
    }
}

fn clif_type(ty: &IRType) -> &'static str {
    match ty {
        IRType::Void => "void",
        IRType::Integer | IRType::Float | IRType::Boolean => "i64",
        _ => "i64",
    }
}

fn sanitize_symbol(symbol: &str) -> String {
    let mut out = String::new();
    for ch in symbol.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "anon".to_string()
    } else {
        out
    }
}

fn sanitize_block(label: &str) -> String {
    format!("block{}", sanitize_symbol(label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_ir::function::Parameter;
    use seen_ir::instruction::{BasicBlock, Instruction, Label};
    use seen_ir::module::IRModule;

    #[test]
    fn clif_emits_function_add() {
        let mut module = IRModule::new("math");
        let mut func = IRFunction::new("Add", IRType::Integer);
        func.add_parameter(Parameter::new("lhs", IRType::Integer));
        func.add_parameter(Parameter::new("rhs", IRType::Integer));

        let mut block = BasicBlock::new(Label::new("entry"));
        block.instructions.push(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(0),
            right: IRValue::Register(1),
            result: IRValue::Register(2),
        });
        block.terminator = Some(Instruction::Return(Some(IRValue::Register(2))));
        func.cfg.add_block(block);
        module.add_function(func);

        let text = module_to_clif(&module);
        assert!(text.contains("function %Add"));
        assert!(text.contains("v2 = iadd.i64 v0, v1"));
        assert!(text.contains("return v2"));
    }

    #[test]
    fn program_orders_modules() {
        let mut program = IRProgram::new();
        let mut a = IRModule::new("zeta");
        let mut f = IRFunction::new("Main", IRType::Void);
        let mut block = BasicBlock::new(Label::new("entry"));
        block.terminator = Some(Instruction::Return(None));
        f.cfg.add_block(block);
        a.add_function(f);
        program.add_module(a);

        let mut b = IRModule::new("alpha");
        let mut g = IRFunction::new("Main", IRType::Void);
        let mut block = BasicBlock::new(Label::new("entry"));
        block.terminator = Some(Instruction::Return(None));
        g.cfg.add_block(block);
        b.add_function(g);
        program.add_module(b);

        let text = program_to_clif(&program);
        assert!(text.contains("; module alpha"));
        assert!(text.contains("; module zeta"));
        assert!(text.find("; module alpha").unwrap() < text.find("; module zeta").unwrap());
    }
}
