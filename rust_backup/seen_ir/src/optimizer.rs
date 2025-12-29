//! IR optimization passes for the Seen programming language

use crate::{
    function::{IRFunction, InlineHint, SimdDecisionReason, SimdMode},
    instruction::{BasicBlock, BinaryOp, Instruction, UnaryOp},
    module::IRModule,
    value::IRValue,
    HardwareProfile, IRProgram, IRResult, SimdPolicy,
};
mod egraph;
mod ml;
use egraph::{try_simplify_binary, SimplifiedExpr};
use ml::{
    DecisionLogger, DecisionReplay, FunctionFeatures, MlDecision, MlHeuristicModel,
    OptimizationProfileWriter, RegisterPressurePlan,
};
use indexmap::{IndexMap, IndexSet};
// Deterministic maps/sets to stabilize optimizer passes
type HashMap<K, V> = IndexMap<K, V>;
type HashSet<T> = IndexSet<T>;

#[cfg(test)]
use crate::value::IRType;

const MIN_SIMD_ARITHMETIC_OPS: usize = 2;

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    None,       // O0 - No optimizations
    Basic,      // O1 - Basic optimizations
    Standard,   // O2 - Standard optimizations
    Aggressive, // O3 - Aggressive optimizations
}

impl OptimizationLevel {
    pub fn from_level(level: u8) -> Self {
        match level {
            0 => OptimizationLevel::None,
            1 => OptimizationLevel::Basic,
            2 => OptimizationLevel::Standard,
            3 | _ => OptimizationLevel::Aggressive,
        }
    }

    pub fn should_run_pass(&self, pass_level: OptimizationLevel) -> bool {
        match (self, pass_level) {
            (OptimizationLevel::None, _) => false,
            (OptimizationLevel::Basic, OptimizationLevel::Basic) => true,
            (
                OptimizationLevel::Standard,
                OptimizationLevel::Basic | OptimizationLevel::Standard,
            ) => true,
            (OptimizationLevel::Aggressive, _) => true,
            _ => false,
        }
    }
}

/// Statistics about optimization passes
#[derive(Debug, Clone, Default)]
pub struct OptimizationStats {
    pub instructions_eliminated: usize,
    pub constant_folding_operations: usize,
    pub dead_code_blocks_removed: usize,
    pub redundant_moves_eliminated: usize,
    pub equality_saturation_applied: usize,
    pub inline_hints_applied: usize,
    pub register_pressure_adjustments: usize,
    pub registers_reused: usize,
    pub lens_superoptimizer_rewrites: usize,
    pub passes_run: Vec<String>,
}

/// IR optimizer that applies various optimization passes
pub struct IROptimizer {
    level: OptimizationLevel,
    stats: OptimizationStats,
    ml_model: Option<MlHeuristicModel>,
    profile_writer: Option<OptimizationProfileWriter>,
    decision_logger: Option<DecisionLogger>,
    decision_replay: Option<DecisionReplay>,
}

impl IROptimizer {
    pub fn new(level: OptimizationLevel) -> Self {
        Self {
            level,
            stats: OptimizationStats::default(),
            ml_model: MlHeuristicModel::from_env(),
            profile_writer: OptimizationProfileWriter::from_env(),
            decision_logger: DecisionLogger::from_env(),
            decision_replay: DecisionReplay::from_env(),
        }
    }

    pub fn get_stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// Optimize an entire IR program
    pub fn optimize_program(&mut self, program: &mut IRProgram) -> IRResult<()> {
        let hardware_profile = program.hardware_profile.clone();
        for module in &mut program.modules {
            self.optimize_module(module, &hardware_profile)?;
        }
        Ok(())
    }

    /// Optimize a single module
    pub fn optimize_module(
        &mut self,
        module: &mut IRModule,
        profile: &HardwareProfile,
    ) -> IRResult<()> {
        for function in module.functions_iter_mut() {
            self.optimize_function_with_profile(function, profile)?;
        }
        Ok(())
    }

    /// Optimize a single function using the default hardware profile.
    pub fn optimize_function(&mut self, function: &mut IRFunction) -> IRResult<()> {
        let profile = HardwareProfile::default();
        self.optimize_function_with_profile(function, &profile)
    }

    /// Optimize a single function with an explicit hardware profile.
    pub fn optimize_function_with_profile(
        &mut self,
        function: &mut IRFunction,
        profile: &HardwareProfile,
    ) -> IRResult<()> {
        let features = FunctionFeatures::from_function(function);
        if let Some(writer) = self.profile_writer.as_mut() {
            writer.record(&function.name, &features);
        }
        let mut decision = if let Some(model) = &self.ml_model {
            model.decide(&features)
        } else {
            MlDecision::from_level(self.level, &features)
        };

        if let Some(replay) = self.decision_replay.as_ref() {
            replay.apply(&function.name, &mut decision);
        }

        function.inline_hint = decision.inline_hint;
        function.register_pressure = decision.register_class;
        if !matches!(function.inline_hint, InlineHint::Auto) {
            self.stats.inline_hints_applied += 1;
        }
        if let Some(plan) = decision.register_plan.clone() {
            if let Some(outcome) = self.apply_register_pressure_plan(function, &plan)? {
                self.stats.register_pressure_adjustments += 1;
                self.stats.registers_reused += outcome.reused;
            }
        }

        let run_aggressive = decision.aggressive_allowed;

        // Run optimization passes in order

        if self.level.should_run_pass(OptimizationLevel::Basic) {
            self.constant_folding(function)?;
            self.stats.passes_run.push("constant_folding".to_string());

            self.eliminate_redundant_moves(function)?;
            self.stats
                .passes_run
                .push("eliminate_redundant_moves".to_string());

            self.eliminate_nops(function)?;
            self.stats.passes_run.push("eliminate_nops".to_string());
        }

        if self.level.should_run_pass(OptimizationLevel::Standard) {
            self.dead_code_elimination(function)?;
            self.stats
                .passes_run
                .push("dead_code_elimination".to_string());

            self.strength_reduction(function)?;
            self.stats.passes_run.push("strength_reduction".to_string());

            self.equality_saturation(function)?;
            self.stats
                .passes_run
                .push("equality_saturation".to_string());

            if features.loop_blocks > 0 {
                self.superoptimize_loop_chains(function)?;
            }
        }

        if run_aggressive {
            self.common_subexpression_elimination(function)?;
            self.stats
                .passes_run
                .push("common_subexpression_elimination".to_string());

            self.loop_optimization(function)?;
            self.stats.passes_run.push("loop_optimization".to_string());
        }

        if let Some(logger) = self.decision_logger.as_mut() {
            let snapshot = FunctionFeatures::from_function(function);
            logger.record(&function.name, &snapshot, &decision);
        }

        self.plan_simd(function, profile, &features);

        Ok(())
    }

    fn plan_simd(
        &self,
        function: &mut IRFunction,
        profile: &HardwareProfile,
        features: &FunctionFeatures,
    ) {
        let summary = Self::analyze_simd_opportunities(function);
        let mut metadata = function.simd_metadata.clone();
        metadata.policy = profile.simd_policy;
        metadata.hot_loops = features.loop_blocks;
        metadata.arithmetic_ops = summary.arithmetic_ops;
        metadata.memory_ops = summary.memory_ops;
        metadata.existing_vector_ops = summary.vector_ops;
        metadata.vector_width_bits = Self::resolve_vector_width_bits(profile);
        metadata.mode = SimdMode::Scalar;
        metadata.reason = SimdDecisionReason::Unknown;
        metadata.estimated_speedup = 1.0;

        let register_budget = profile.register_budget_hint();
        let register_usage = features.register_count.min(u32::MAX as usize) as u32;
        let memory_bound = summary.memory_ops > summary.arithmetic_ops.saturating_mul(2);
        let resolved_vector_bits = metadata.vector_width_bits.unwrap_or(128);

        match profile.simd_policy {
            SimdPolicy::Off => {
                metadata.mode = SimdMode::Scalar;
                metadata.reason = SimdDecisionReason::PolicyOff;
            }
            SimdPolicy::Max => {
                if summary.arithmetic_ops == 0 {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::NoOpportunities;
                } else {
                    metadata.mode = SimdMode::Vectorized;
                    metadata.reason = SimdDecisionReason::ForcedMax;
                    metadata.estimated_speedup =
                        Self::estimate_speedup(summary.arithmetic_ops, resolved_vector_bits);
                }
            }
            SimdPolicy::Auto => {
                if summary.arithmetic_ops == 0 {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::NoOpportunities;
                } else if features.loop_blocks == 0 {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::AutoNoHotLoops;
                } else if summary.arithmetic_ops < MIN_SIMD_ARITHMETIC_OPS {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::AutoLowArithmeticIntensity;
                } else if memory_bound {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::AutoMemoryBound;
                } else if register_usage >= register_budget.saturating_sub(4) {
                    metadata.mode = SimdMode::Scalar;
                    metadata.reason = SimdDecisionReason::AutoHighRegisterPressure;
                } else {
                    metadata.mode = SimdMode::Vectorized;
                    metadata.reason = SimdDecisionReason::AutoHotLoop;
                    metadata.estimated_speedup =
                        Self::estimate_speedup(summary.arithmetic_ops, resolved_vector_bits);
                }
            }
        }

        function.simd_metadata = metadata;
    }

    fn analyze_simd_opportunities(function: &IRFunction) -> SimdOpportunitySummary {
        let mut summary = SimdOpportunitySummary::default();
        for block in function.cfg.blocks_iter() {
            for inst in &block.instructions {
                match inst {
                    Instruction::Binary { op, .. } => {
                        if matches!(
                            op,
                            BinaryOp::Add
                                | BinaryOp::Subtract
                                | BinaryOp::Multiply
                                | BinaryOp::Divide
                                | BinaryOp::Modulo
                        ) {
                            summary.arithmetic_ops += 1;
                        }
                    }
                    Instruction::Load { .. }
                    | Instruction::Store { .. }
                    | Instruction::ArrayAccess { .. }
                    | Instruction::ArraySet { .. } => {
                        summary.memory_ops += 1;
                    }
                    Instruction::SimdSplat { .. } | Instruction::SimdReduceAdd { .. } => {
                        summary.vector_ops += 1;
                    }
                    _ => {}
                }
            }
        }
        summary
    }

    fn resolve_vector_width_bits(profile: &HardwareProfile) -> Option<u32> {
        if let Some(bits) = profile.max_vector_bits {
            Some(bits)
        } else if profile.sve_enabled {
            Some(512)
        } else if profile.apx_enabled {
            Some(256)
        } else {
            None
        }
    }

    fn estimate_speedup(op_count: usize, vector_bits: u32) -> f32 {
        if op_count == 0 {
            return 1.0;
        }
        let width_factor = (vector_bits.max(128) as f32) / 128.0;
        let op_factor = (op_count.min(64) as f32) / 64.0;
        1.0 + 0.5 * width_factor * op_factor
    }

    fn apply_register_pressure_plan(
        &mut self,
        function: &mut IRFunction,
        plan: &RegisterPressurePlan,
    ) -> IRResult<Option<RegisterPlanOutcome>> {
        if plan.target_register_budget == 0
            || function.register_count <= plan.target_register_budget
        {
            return Ok(None);
        }

        let before = function.register_count;
        let mut remaining = Self::count_register_uses(function);
        if remaining.is_empty() {
            return Ok(None);
        }

        let mut allocator = RegisterAllocatorState::new();
        let mut mapping: HashMap<u32, u32> = HashMap::new();

        for block in function.cfg.blocks_iter_mut() {
            for inst in &mut block.instructions {
                Self::remap_instruction_registers(
                    inst,
                    &mut allocator,
                    &mut mapping,
                    &mut remaining,
                );
            }
            if let Some(term) = block.terminator.as_mut() {
                Self::remap_instruction_registers(
                    term,
                    &mut allocator,
                    &mut mapping,
                    &mut remaining,
                );
            }
        }

        for local in function.locals_iter_mut() {
            if let Some(old_reg) = local.register {
                if let Some(&new_reg) = mapping.get(&old_reg) {
                    local.register = Some(new_reg);
                }
            }
        }

        let after = allocator.max_register_index();
        if after >= before {
            function.register_count = after;
            return Ok(None);
        }

        function.register_count = after;
        Ok(Some(RegisterPlanOutcome {
            reused: allocator.reused(),
        }))
    }

    fn superoptimize_loop_chains(&mut self, function: &mut IRFunction) -> IRResult<()> {
        let mut rewrites = 0;
        for block in function.cfg.blocks_iter_mut() {
            if block.label.0.contains("loop") {
                rewrites += Self::superoptimize_block(block);
            }
        }
        if rewrites > 0 {
            self.stats.lens_superoptimizer_rewrites += rewrites;
            self.stats
                .passes_run
                .push("lens_superoptimizer".to_string());
        }
        Ok(())
    }

    fn superoptimize_block(block: &mut BasicBlock) -> usize {
        let use_counts = Self::block_register_use_counts(block);
        Self::fuse_linear_pairs(block, &use_counts)
    }

    fn block_register_use_counts(block: &BasicBlock) -> HashMap<u32, usize> {
        let mut counts = HashMap::new();
        for inst in &block.instructions {
            Self::count_uses_in_instruction(inst, &mut counts);
        }
        if let Some(term) = &block.terminator {
            Self::count_uses_in_instruction(term, &mut counts);
        }
        counts
    }

    fn fuse_linear_pairs(block: &mut BasicBlock, use_counts: &HashMap<u32, usize>) -> usize {
        let mut rewrites = 0;
        let mut idx = 0;
        while idx + 1 < block.instructions.len() {
            if let Some(new_inst) = Self::try_linear_fusion(
                &block.instructions[idx],
                &block.instructions[idx + 1],
                use_counts,
            ) {
                block.instructions[idx + 1] = new_inst;
                block.instructions.remove(idx);
                rewrites += 1;
                if idx > 0 {
                    idx -= 1;
                }
                continue;
            }
            idx += 1;
        }
        rewrites
    }

    fn try_linear_fusion(
        first: &Instruction,
        second: &Instruction,
        use_counts: &HashMap<u32, usize>,
    ) -> Option<Instruction> {
        match (first, second) {
            (
                Instruction::Binary {
                    op: op1,
                    left: left1,
                    right: right1,
                    result: IRValue::Register(tmp),
                },
                Instruction::Binary {
                    op: op2,
                    left: left2,
                    right: right2,
                    result,
                },
            ) => {
                if use_counts.get(tmp).copied().unwrap_or(0) != 1 {
                    return None;
                }
                if let Some((base, offset)) = Self::decompose_affine_component(op1, left1, right1) {
                    if let Some(second_offset) =
                        Self::affine_second_offset(op2, left2, right2, *tmp)
                    {
                        return Some(Self::build_affine_instruction(
                            base,
                            offset + second_offset,
                            result.clone(),
                        ));
                    }
                }
                if let Some((base, factor)) =
                    Self::decompose_multiplicative_component(op1, left1, right1)
                {
                    if let Some(second_factor) =
                        Self::multiplicative_second_factor(op2, left2, right2, *tmp)
                    {
                        return Some(Self::build_multiplicative_instruction(
                            base,
                            factor * second_factor,
                            result.clone(),
                        ));
                    }
                }
                if let Some((base, shift)) = Self::decompose_shift_component(op1, left1, right1) {
                    if let Some(second_shift) = Self::shift_second_amount(op2, left2, right2, *tmp)
                    {
                        return Some(Self::build_shift_instruction(
                            base,
                            shift + second_shift,
                            result.clone(),
                        ));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn decompose_affine_component(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
    ) -> Option<(IRValue, i64)> {
        match (op, left, right) {
            (BinaryOp::Add, IRValue::Integer(c), other)
            | (BinaryOp::Add, other, IRValue::Integer(c)) => Some((other.clone(), *c)),
            (BinaryOp::Subtract, other, IRValue::Integer(c)) => Some((other.clone(), -*c)),
            _ => None,
        }
    }

    fn affine_second_offset(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
        tmp: u32,
    ) -> Option<i64> {
        match (left, right) {
            (IRValue::Register(reg), IRValue::Integer(c)) if *reg == tmp => match op {
                BinaryOp::Add => Some(*c),
                BinaryOp::Subtract => Some(-*c),
                _ => None,
            },
            (IRValue::Integer(c), IRValue::Register(reg)) if *reg == tmp => match op {
                BinaryOp::Add => Some(*c),
                _ => None,
            },
            _ => None,
        }
    }

    fn build_affine_instruction(base: IRValue, offset: i64, result: IRValue) -> Instruction {
        if offset == 0 {
            Instruction::Move {
                source: base,
                dest: result,
            }
        } else {
            Instruction::Binary {
                op: BinaryOp::Add,
                left: base,
                right: IRValue::Integer(offset),
                result,
            }
        }
    }

    fn decompose_multiplicative_component(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
    ) -> Option<(IRValue, i64)> {
        if !matches!(op, BinaryOp::Multiply) {
            return None;
        }
        match (left, right) {
            (IRValue::Integer(c), other) | (other, IRValue::Integer(c)) => {
                Some((other.clone(), *c))
            }
            _ => None,
        }
    }

    fn multiplicative_second_factor(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
        tmp: u32,
    ) -> Option<i64> {
        if !matches!(op, BinaryOp::Multiply) {
            return None;
        }
        match (left, right) {
            (IRValue::Register(reg), IRValue::Integer(c))
            | (IRValue::Integer(c), IRValue::Register(reg))
                if *reg == tmp =>
            {
                Some(*c)
            }
            _ => None,
        }
    }

    fn build_multiplicative_instruction(
        base: IRValue,
        factor: i64,
        result: IRValue,
    ) -> Instruction {
        if factor == 1 {
            Instruction::Move {
                source: base,
                dest: result,
            }
        } else {
            Instruction::Binary {
                op: BinaryOp::Multiply,
                left: base,
                right: IRValue::Integer(factor),
                result,
            }
        }
    }

    fn decompose_shift_component(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
    ) -> Option<(IRValue, i64)> {
        if !matches!(op, BinaryOp::LeftShift) {
            return None;
        }
        match right {
            IRValue::Integer(amount) if *amount >= 0 => Some((left.clone(), *amount)),
            _ => None,
        }
    }

    fn shift_second_amount(
        op: &BinaryOp,
        left: &IRValue,
        right: &IRValue,
        tmp: u32,
    ) -> Option<i64> {
        if !matches!(op, BinaryOp::LeftShift) {
            return None;
        }
        match (left, right) {
            (IRValue::Register(reg), IRValue::Integer(amount)) if *reg == tmp && *amount >= 0 => {
                Some(*amount)
            }
            _ => None,
        }
    }

    fn build_shift_instruction(base: IRValue, shift: i64, result: IRValue) -> Instruction {
        if shift == 0 {
            Instruction::Move {
                source: base,
                dest: result,
            }
        } else {
            Instruction::Binary {
                op: BinaryOp::LeftShift,
                left: base,
                right: IRValue::Integer(shift),
                result,
            }
        }
    }

    fn count_register_uses(function: &IRFunction) -> HashMap<u32, usize> {
        let mut counts = HashMap::new();
        for block in function.cfg.blocks_iter() {
            for inst in &block.instructions {
                Self::count_uses_in_instruction(inst, &mut counts);
            }
            if let Some(term) = &block.terminator {
                Self::count_uses_in_instruction(term, &mut counts);
            }
        }
        counts
    }

    fn count_uses_in_instruction(inst: &Instruction, counts: &mut HashMap<u32, usize>) {
        match inst {
            Instruction::Binary { left, right, .. } => {
                Self::accumulate_read_registers(left, counts);
                Self::accumulate_read_registers(right, counts);
            }
            Instruction::Unary { operand, .. } => {
                Self::accumulate_read_registers(operand, counts);
            }
            Instruction::Move { source, .. } => {
                Self::accumulate_read_registers(source, counts);
            }
            Instruction::Load { source, .. } => {
                Self::accumulate_read_registers(source, counts);
            }
            Instruction::Store { value, dest } => {
                Self::accumulate_read_registers(value, counts);
                Self::accumulate_read_registers(dest, counts);
            }
            Instruction::SimdSplat { scalar, .. } => {
                Self::accumulate_read_registers(scalar, counts);
            }
            Instruction::SimdReduceAdd { vector, .. } => {
                Self::accumulate_read_registers(vector, counts);
            }
            Instruction::Call { target, args, .. } => {
                Self::accumulate_read_registers(target, counts);
                for arg in args {
                    Self::accumulate_read_registers(arg, counts);
                }
            }
            Instruction::Return(Some(value)) => {
                Self::accumulate_read_registers(value, counts);
            }
            Instruction::JumpIf { condition, .. } | Instruction::JumpIfNot { condition, .. } => {
                Self::accumulate_read_registers(condition, counts);
            }
            Instruction::Allocate { size, .. } => {
                Self::accumulate_read_registers(size, counts);
            }
            Instruction::Deallocate { pointer } => {
                Self::accumulate_read_registers(pointer, counts);
            }
            Instruction::ArrayAccess { array, index, .. } => {
                Self::accumulate_read_registers(array, counts);
                Self::accumulate_read_registers(index, counts);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                Self::accumulate_read_registers(array, counts);
                Self::accumulate_read_registers(index, counts);
                Self::accumulate_read_registers(value, counts);
            }
            Instruction::ArrayLength { array, .. } => {
                Self::accumulate_read_registers(array, counts);
            }
            Instruction::FieldAccess { struct_val, .. } => {
                Self::accumulate_read_registers(struct_val, counts);
            }
            Instruction::FieldSet {
                struct_val, value, ..
            } => {
                Self::accumulate_read_registers(struct_val, counts);
                Self::accumulate_read_registers(value, counts);
            }
            Instruction::GetEnumTag { enum_value, .. }
            | Instruction::GetEnumField { enum_value, .. } => {
                Self::accumulate_read_registers(enum_value, counts);
            }
            Instruction::Cast { value, .. }
            | Instruction::TypeCheck { value, .. }
            | Instruction::StringLength { string: value, .. } => {
                Self::accumulate_read_registers(value, counts);
            }
            Instruction::StringConcat { left, right, .. } => {
                Self::accumulate_read_registers(left, counts);
                Self::accumulate_read_registers(right, counts);
            }
            Instruction::VirtualCall { receiver, args, .. } => {
                Self::accumulate_read_registers(receiver, counts);
                for arg in args {
                    Self::accumulate_read_registers(arg, counts);
                }
            }
            Instruction::StaticCall { args, .. } => {
                for arg in args {
                    Self::accumulate_read_registers(arg, counts);
                }
            }
            Instruction::ConstructObject { args, .. }
            | Instruction::ConstructEnum { fields: args, .. } => {
                for arg in args {
                    Self::accumulate_read_registers(arg, counts);
                }
            }
            Instruction::Print(value) => {
                Self::accumulate_read_registers(value, counts);
            }
            Instruction::Debug { value, .. } => {
                if let Some(val) = value {
                    Self::accumulate_read_registers(val, counts);
                }
            }
            Instruction::ChannelSelect { cases, .. } => {
                for arm in cases {
                    Self::accumulate_read_registers(&arm.channel, counts);
                }
            }
            Instruction::Scoped { .. }
            | Instruction::Spawn { .. }
            | Instruction::PushFrame
            | Instruction::PopFrame
            | Instruction::Jump(_)
            | Instruction::Label(_)
            | Instruction::Return(None)
            | Instruction::Nop => {}
        }
    }

    fn accumulate_read_registers(value: &IRValue, counts: &mut HashMap<u32, usize>) {
        match value {
            IRValue::Register(reg) => {
                *counts.entry(*reg).or_insert(0) += 1;
            }
            IRValue::Array(values) => {
                for v in values {
                    Self::accumulate_read_registers(v, counts);
                }
            }
            IRValue::Struct { fields, .. } => {
                for v in fields.values() {
                    Self::accumulate_read_registers(v, counts);
                }
            }
            IRValue::AddressOf(inner) => {
                Self::accumulate_read_registers(inner, counts);
            }
            _ => {}
        }
    }

    fn remap_instruction_registers(
        inst: &mut Instruction,
        allocator: &mut RegisterAllocatorState,
        mapping: &mut HashMap<u32, u32>,
        remaining: &mut HashMap<u32, usize>,
    ) {
        match inst {
            Instruction::Binary {
                left,
                right,
                result,
                ..
            } => {
                Self::remap_read_value(left, allocator, mapping, remaining);
                Self::remap_read_value(right, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Unary {
                operand, result, ..
            } => {
                Self::remap_read_value(operand, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Move { source, dest } => {
                Self::remap_read_value(source, allocator, mapping, remaining);
                Self::remap_write_value(dest, allocator, mapping, remaining);
            }
            Instruction::Load { source, dest } => {
                Self::remap_read_value(source, allocator, mapping, remaining);
                Self::remap_write_value(dest, allocator, mapping, remaining);
            }
            Instruction::Store { value, dest } => {
                Self::remap_read_value(value, allocator, mapping, remaining);
                Self::remap_read_value(dest, allocator, mapping, remaining);
            }
            Instruction::SimdSplat { scalar, result, .. } => {
                Self::remap_read_value(scalar, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::SimdReduceAdd { vector, result, .. } => {
                Self::remap_read_value(vector, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Call {
                target,
                args,
                result,
                ..
            } => {
                Self::remap_read_value(target, allocator, mapping, remaining);
                for arg in args {
                    Self::remap_read_value(arg, allocator, mapping, remaining);
                }
                if let Some(res) = result {
                    Self::remap_write_value(res, allocator, mapping, remaining);
                }
            }
            Instruction::Return(Some(value)) => {
                Self::remap_read_value(value, allocator, mapping, remaining);
            }
            Instruction::JumpIf { condition, .. } | Instruction::JumpIfNot { condition, .. } => {
                Self::remap_read_value(condition, allocator, mapping, remaining);
            }
            Instruction::Allocate { size, result } => {
                Self::remap_read_value(size, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Deallocate { pointer } => {
                Self::remap_read_value(pointer, allocator, mapping, remaining);
            }
            Instruction::ArrayAccess {
                array,
                index,
                result,
                ..
            } => {
                Self::remap_read_value(array, allocator, mapping, remaining);
                Self::remap_read_value(index, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                Self::remap_read_value(array, allocator, mapping, remaining);
                Self::remap_read_value(index, allocator, mapping, remaining);
                Self::remap_read_value(value, allocator, mapping, remaining);
            }
            Instruction::ArrayLength { array, result } => {
                Self::remap_read_value(array, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::FieldAccess {
                struct_val, result, ..
            } => {
                Self::remap_read_value(struct_val, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::FieldSet {
                struct_val, value, ..
            } => {
                Self::remap_read_value(struct_val, allocator, mapping, remaining);
                Self::remap_read_value(value, allocator, mapping, remaining);
            }
            Instruction::GetEnumTag { enum_value, result } => {
                Self::remap_read_value(enum_value, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::GetEnumField {
                enum_value, result, ..
            } => {
                Self::remap_read_value(enum_value, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Cast { value, result, .. }
            | Instruction::TypeCheck { value, result, .. }
            | Instruction::StringLength {
                string: value,
                result,
            } => {
                Self::remap_read_value(value, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::StringConcat {
                left,
                right,
                result,
            } => {
                Self::remap_read_value(left, allocator, mapping, remaining);
                Self::remap_read_value(right, allocator, mapping, remaining);
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::VirtualCall {
                receiver,
                args,
                result,
                ..
            } => {
                Self::remap_read_value(receiver, allocator, mapping, remaining);
                for arg in args {
                    Self::remap_read_value(arg, allocator, mapping, remaining);
                }
                if let Some(res) = result {
                    Self::remap_write_value(res, allocator, mapping, remaining);
                }
            }
            Instruction::StaticCall { args, result, .. } => {
                for arg in args {
                    Self::remap_read_value(arg, allocator, mapping, remaining);
                }
                if let Some(res) = result {
                    Self::remap_write_value(res, allocator, mapping, remaining);
                }
            }
            Instruction::ConstructObject { args, result, .. } => {
                for arg in args {
                    Self::remap_read_value(arg, allocator, mapping, remaining);
                }
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::ConstructEnum { fields, result, .. } => {
                for field in fields {
                    Self::remap_read_value(field, allocator, mapping, remaining);
                }
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Print(value) => {
                Self::remap_read_value(value, allocator, mapping, remaining);
            }
            Instruction::Debug { value, .. } => {
                if let Some(val) = value {
                    Self::remap_read_value(val, allocator, mapping, remaining);
                }
            }
            Instruction::ChannelSelect {
                cases,
                payload_result,
                index_result,
                status_result,
            } => {
                for arm in cases {
                    Self::remap_read_value(&mut arm.channel, allocator, mapping, remaining);
                }
                Self::remap_write_value(payload_result, allocator, mapping, remaining);
                Self::remap_write_value(index_result, allocator, mapping, remaining);
                Self::remap_write_value(status_result, allocator, mapping, remaining);
            }
            Instruction::Scoped { result, .. } | Instruction::Spawn { result, .. } => {
                Self::remap_write_value(result, allocator, mapping, remaining);
            }
            Instruction::Jump(_)
            | Instruction::Label(_)
            | Instruction::PushFrame
            | Instruction::PopFrame
            | Instruction::Return(None)
            | Instruction::Nop => {}
        }
    }

    fn remap_read_value(
        value: &mut IRValue,
        allocator: &mut RegisterAllocatorState,
        mapping: &mut HashMap<u32, u32>,
        remaining: &mut HashMap<u32, usize>,
    ) {
        match value {
            IRValue::Register(old) => {
                let original = *old;
                let new_reg = *mapping
                    .entry(original)
                    .or_insert_with(|| allocator.allocate_new());
                *value = IRValue::Register(new_reg);
                allocator.consume_use(original, new_reg, remaining);
            }
            IRValue::Array(values) => {
                for v in values {
                    Self::remap_read_value(v, allocator, mapping, remaining);
                }
            }
            IRValue::Struct { fields, .. } => {
                for v in fields.values_mut() {
                    Self::remap_read_value(v, allocator, mapping, remaining);
                }
            }
            IRValue::AddressOf(inner) => {
                Self::remap_read_value(inner, allocator, mapping, remaining);
            }
            _ => {}
        }
    }

    fn remap_write_value(
        value: &mut IRValue,
        allocator: &mut RegisterAllocatorState,
        mapping: &mut HashMap<u32, u32>,
        remaining: &mut HashMap<u32, usize>,
    ) {
        if let IRValue::Register(old) = value {
            let original = *old;
            let new_reg = allocator.allocate_new();
            mapping.insert(original, new_reg);
            *value = IRValue::Register(new_reg);
            if remaining.get(&original).copied().unwrap_or(0) == 0 {
                allocator.release(new_reg);
            }
        }
    }

    /// Constant folding optimization
    fn constant_folding(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks_iter_mut() {
            let mut new_instructions = Vec::new();

            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary {
                        op,
                        left,
                        right,
                        result,
                    } => {
                        if let (IRValue::Integer(l), IRValue::Integer(r)) = (left, right) {
                            let folded_value = match op {
                                BinaryOp::Add => Some(IRValue::Integer(l + r)),
                                BinaryOp::Subtract => Some(IRValue::Integer(l - r)),
                                BinaryOp::Multiply => Some(IRValue::Integer(l * r)),
                                BinaryOp::Divide if *r != 0 => Some(IRValue::Integer(l / r)),
                                BinaryOp::Modulo if *r != 0 => Some(IRValue::Integer(l % r)),
                                BinaryOp::Equal => Some(IRValue::Boolean(l == r)),
                                BinaryOp::NotEqual => Some(IRValue::Boolean(l != r)),
                                BinaryOp::LessThan => Some(IRValue::Boolean(l < r)),
                                BinaryOp::LessEqual => Some(IRValue::Boolean(l <= r)),
                                BinaryOp::GreaterThan => Some(IRValue::Boolean(l > r)),
                                BinaryOp::GreaterEqual => Some(IRValue::Boolean(l >= r)),
                                _ => None,
                            };

                            if let Some(value) = folded_value {
                                // Replace with a move instruction
                                new_instructions.push(Instruction::Move {
                                    source: value,
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                                continue;
                            }
                        }

                        // Handle float constants
                        if let (IRValue::Float(l), IRValue::Float(r)) = (left, right) {
                            let folded_value = match op {
                                BinaryOp::Add => Some(IRValue::Float(l + r)),
                                BinaryOp::Subtract => Some(IRValue::Float(l - r)),
                                BinaryOp::Multiply => Some(IRValue::Float(l * r)),
                                BinaryOp::Divide if *r != 0.0 => Some(IRValue::Float(l / r)),
                                BinaryOp::Equal => {
                                    Some(IRValue::Boolean((l - r).abs() < f64::EPSILON))
                                }
                                BinaryOp::NotEqual => {
                                    Some(IRValue::Boolean((l - r).abs() >= f64::EPSILON))
                                }
                                BinaryOp::LessThan => Some(IRValue::Boolean(l < r)),
                                BinaryOp::LessEqual => Some(IRValue::Boolean(l <= r)),
                                BinaryOp::GreaterThan => Some(IRValue::Boolean(l > r)),
                                BinaryOp::GreaterEqual => Some(IRValue::Boolean(l >= r)),
                                _ => None,
                            };

                            if let Some(value) = folded_value {
                                new_instructions.push(Instruction::Move {
                                    source: value,
                                    dest: result.clone(),
                                });
                                self.stats.constant_folding_operations += 1;
                                continue;
                            }
                        }

                        new_instructions.push(instruction.clone());
                    }

                    Instruction::Unary {
                        op,
                        operand,
                        result,
                    } => match (op, operand) {
                        (UnaryOp::Negate, IRValue::Integer(i)) => {
                            new_instructions.push(Instruction::Move {
                                source: IRValue::Integer(-i),
                                dest: result.clone(),
                            });
                            self.stats.constant_folding_operations += 1;
                        }
                        (UnaryOp::Negate, IRValue::Float(f)) => {
                            new_instructions.push(Instruction::Move {
                                source: IRValue::Float(-f),
                                dest: result.clone(),
                            });
                            self.stats.constant_folding_operations += 1;
                        }
                        (UnaryOp::Not, IRValue::Boolean(b)) => {
                            new_instructions.push(Instruction::Move {
                                source: IRValue::Boolean(!b),
                                dest: result.clone(),
                            });
                            self.stats.constant_folding_operations += 1;
                        }
                        _ => new_instructions.push(instruction.clone()),
                    },

                    _ => new_instructions.push(instruction.clone()),
                }
            }

            block.instructions = new_instructions;
        }

        Ok(())
    }

    /// Eliminate redundant move instructions
    fn eliminate_redundant_moves(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks_iter_mut() {
            let mut new_instructions = Vec::new();

            for instruction in &block.instructions {
                match instruction {
                    Instruction::Move { source, dest } if source == dest => {
                        // Skip redundant moves (mov %r1 -> %r1)
                        self.stats.redundant_moves_eliminated += 1;
                        self.stats.instructions_eliminated += 1;
                    }
                    _ => new_instructions.push(instruction.clone()),
                }
            }

            block.instructions = new_instructions;
        }

        Ok(())
    }

    /// Eliminate no-op instructions
    fn eliminate_nops(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks_iter_mut() {
            let original_len = block.instructions.len();
            block
                .instructions
                .retain(|inst| !matches!(inst, Instruction::Nop));

            let eliminated = original_len - block.instructions.len();
            self.stats.instructions_eliminated += eliminated;
        }

        Ok(())
    }

    /// Dead code elimination
    fn dead_code_elimination(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // Find all used values
        let mut used_values = HashSet::new();

        // Mark values used in instructions
        for block in function.cfg.blocks_iter() {
            for instruction in &block.instructions {
                self.mark_used_values(instruction, &mut used_values);
            }
            if let Some(terminator) = &block.terminator {
                self.mark_used_values(terminator, &mut used_values);
            }
        }

        // Remove instructions that produce unused values
        for block in function.cfg.blocks_iter_mut() {
            let mut new_instructions = Vec::new();

            for instruction in &block.instructions {
                if self.instruction_produces_used_value(instruction, &used_values) {
                    new_instructions.push(instruction.clone());
                } else if self.has_side_effects(instruction) {
                    new_instructions.push(instruction.clone());
                } else {
                    self.stats.instructions_eliminated += 1;
                }
            }

            block.instructions = new_instructions;
        }

        Ok(())
    }

    /// Strength reduction (replace expensive operations with cheaper ones)
    fn strength_reduction(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks_iter_mut() {
            let mut new_instructions = Vec::new();

            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary {
                        op: BinaryOp::Multiply,
                        left,
                        right,
                        result,
                    } => {
                        // Replace multiplication by power of 2 with left shift
                        if let IRValue::Integer(n) = right {
                            if *n > 0 && (*n & (*n - 1)) == 0 {
                                // Check if power of 2
                                let shift_amount = (*n as u64).trailing_zeros();
                                new_instructions.push(Instruction::Binary {
                                    op: BinaryOp::LeftShift,
                                    left: left.clone(),
                                    right: IRValue::Integer(shift_amount as i64),
                                    result: result.clone(),
                                });
                                continue;
                            }
                        }
                        new_instructions.push(instruction.clone());
                    }

                    Instruction::Binary {
                        op: BinaryOp::Divide,
                        left,
                        right,
                        result,
                    } => {
                        // Replace division by power of 2 with right shift
                        if let IRValue::Integer(n) = right {
                            if *n > 0 && (*n & (*n - 1)) == 0 {
                                // Check if power of 2
                                let shift_amount = (*n as u64).trailing_zeros();
                                new_instructions.push(Instruction::Binary {
                                    op: BinaryOp::RightShift,
                                    left: left.clone(),
                                    right: IRValue::Integer(shift_amount as i64),
                                    result: result.clone(),
                                });
                                continue;
                            }
                        }
                        new_instructions.push(instruction.clone());
                    }

                    _ => new_instructions.push(instruction.clone()),
                }
            }

            block.instructions = new_instructions;
        }

        Ok(())
    }

    /// Common subexpression elimination
    fn common_subexpression_elimination(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // This is a simplified CSE that works within basic blocks
        for block in function.cfg.blocks_iter_mut() {
            let mut expression_map: HashMap<String, IRValue> = HashMap::new();
            let mut new_instructions = Vec::new();

            for instruction in &block.instructions {
                match instruction {
                    Instruction::Binary {
                        op,
                        left,
                        right,
                        result,
                    } => {
                        let expr_key = format!("{}:{}:{}", op, left, right);

                        if let Some(existing_result) = expression_map.get(&expr_key) {
                            // Replace with move from existing result
                            new_instructions.push(Instruction::Move {
                                source: existing_result.clone(),
                                dest: result.clone(),
                            });
                        } else {
                            new_instructions.push(instruction.clone());
                            expression_map.insert(expr_key, result.clone());
                        }
                    }

                    Instruction::Unary {
                        op,
                        operand,
                        result,
                    } => {
                        let expr_key = format!("{}:{}", op, operand);

                        if let Some(existing_result) = expression_map.get(&expr_key) {
                            new_instructions.push(Instruction::Move {
                                source: existing_result.clone(),
                                dest: result.clone(),
                            });
                        } else {
                            new_instructions.push(instruction.clone());
                            expression_map.insert(expr_key, result.clone());
                        }
                    }

                    _ => {
                        new_instructions.push(instruction.clone());
                        // Invalidate expressions that might be affected by this instruction
                        if self.has_side_effects(instruction) {
                            expression_map.clear();
                        }
                    }
                }
            }

            block.instructions = new_instructions;
        }

        Ok(())
    }

    /// Loop optimization (basic loop invariant code motion)
    fn loop_optimization(&mut self, function: &mut IRFunction) -> IRResult<()> {
        // Implement basic loop optimizations

        // 1. Loop invariant code motion - move loop-invariant expressions outside loops
        let mut invariant_instructions = Vec::new();

        // Simple analysis: identify instructions that don't depend on loop variables
        for block in function.cfg.blocks_iter() {
            for instruction in &block.instructions {
                if self.is_loop_invariant(instruction) {
                    invariant_instructions.push(instruction.clone());
                }
            }
        }

        // For simplicity, record that we attempted loop optimization
        self.stats.passes_run.push("loop_optimization".to_string());

        Ok(())
    }

    /// Check if an instruction is loop invariant (simplified analysis)
    fn is_loop_invariant(&self, instruction: &crate::instruction::Instruction) -> bool {
        use crate::instruction::Instruction;
        match instruction {
            // Constants are always loop invariant
            Instruction::Load {
                source: crate::IRValue::Integer(_),
                ..
            } => true,
            Instruction::Load {
                source: crate::IRValue::Float(_),
                ..
            } => true,
            Instruction::Load {
                source: crate::IRValue::Boolean(_),
                ..
            } => true,
            Instruction::Load {
                source: crate::IRValue::String(_),
                ..
            } => true,

            // Most other instructions could be loop variant
            _ => false,
        }
    }

    /// Optimize loop invariant code motion
    #[allow(dead_code)]
    fn optimize_loop_invariants(&mut self, _function: &mut IRFunction) -> IRResult<()> {
        // Loop invariant code motion: identify computations that don't change within loops
        // and move them to execute before the loop begins

        // Real implementation would:
        // 1. Identify natural loops in the CFG
        // 2. Find loop-invariant instructions (those that only use values not modified in the loop)
        // 3. Check that moving the instruction is safe (no side effects, dominance preserved)
        // 4. Move instructions to loop preheader blocks

        Ok(())
    }

    /// Optimize loop unrolling for small loops
    #[allow(dead_code)]
    fn optimize_loop_unrolling(&mut self, _function: &mut IRFunction) -> IRResult<()> {
        // Loop unrolling: duplicate loop body to reduce loop overhead
        // and enable better instruction scheduling

        // Real implementation would:
        // 1. Identify loops with small, constant trip counts
        // 2. Check that unrolling is profitable (small body, few iterations)
        // 3. Duplicate the loop body and adjust branch targets
        // 4. Update phi nodes and induction variables

        Ok(())
    }

    /// Helper: Mark values used by an instruction
    fn mark_used_values(&self, instruction: &Instruction, used_values: &mut HashSet<String>) {
        match instruction {
            Instruction::Binary { left, right, .. } => {
                self.mark_value_used(left, used_values);
                self.mark_value_used(right, used_values);
            }
            Instruction::Unary { operand, .. } => {
                self.mark_value_used(operand, used_values);
            }
            Instruction::Move { source, .. } => {
                self.mark_value_used(source, used_values);
            }
            Instruction::Store { value, dest } => {
                self.mark_value_used(value, used_values);
                self.mark_value_used(dest, used_values);
            }
            Instruction::Load { source, .. } => {
                self.mark_value_used(source, used_values);
            }
            Instruction::Call { target, args, .. } => {
                self.mark_value_used(target, used_values);
                for arg in args {
                    self.mark_value_used(arg, used_values);
                }
            }
            Instruction::Return(Some(value)) => {
                self.mark_value_used(value, used_values);
            }
            Instruction::JumpIf { condition, .. } => {
                self.mark_value_used(condition, used_values);
            }
            Instruction::JumpIfNot { condition, .. } => {
                self.mark_value_used(condition, used_values);
            }
            Instruction::Print(value) => {
                self.mark_value_used(value, used_values);
            }
            // Array operations
            Instruction::ArrayLength { array, .. } => {
                self.mark_value_used(array, used_values);
            }
            Instruction::ArrayAccess { array, index, .. } => {
                self.mark_value_used(array, used_values);
                self.mark_value_used(index, used_values);
            }
            Instruction::ArraySet { array, index, value, .. } => {
                self.mark_value_used(array, used_values);
                self.mark_value_used(index, used_values);
                self.mark_value_used(value, used_values);
            }
            // Field operations
            Instruction::FieldAccess { struct_val, .. } => {
                self.mark_value_used(struct_val, used_values);
            }
            Instruction::FieldSet { struct_val, value, .. } => {
                self.mark_value_used(struct_val, used_values);
                self.mark_value_used(value, used_values);
            }
            // Cast operations
            Instruction::Cast { value, .. } => {
                self.mark_value_used(value, used_values);
            }
            // String operations
            Instruction::StringConcat { left, right, .. } => {
                self.mark_value_used(left, used_values);
                self.mark_value_used(right, used_values);
            }
            // Allocate and other operations
            Instruction::Allocate { size, .. } => {
                self.mark_value_used(size, used_values);
            }
            Instruction::Deallocate { pointer } => {
                self.mark_value_used(pointer, used_values);
            }
            // Add other instruction types as needed
            _ => {}
        }
    }

    /// Helper: Mark a specific value as used
    fn mark_value_used(&self, value: &IRValue, used_values: &mut HashSet<String>) {
        match value {
            IRValue::Variable(name) => {
                used_values.insert(name.clone());
            }
            IRValue::Register(reg) => {
                used_values.insert(format!("r{}", reg));
            }
            IRValue::GlobalVariable(name) => {
                used_values.insert(name.clone());
            }
            _ => {}
        }
    }

    /// Helper: Check if instruction produces a used value
    fn instruction_produces_used_value(
        &self,
        instruction: &Instruction,
        used_values: &HashSet<String>,
    ) -> bool {
        match instruction {
            Instruction::Binary { result, .. }
            | Instruction::Unary { result, .. }
            | Instruction::Load { dest: result, .. }
            | Instruction::ArrayAccess { result, .. }
            | Instruction::FieldAccess { result, .. } => self.value_is_used(result, used_values),
            Instruction::Call {
                result: Some(result),
                ..
            } => self.value_is_used(result, used_values),
            _ => true, // Conservative: assume other instructions are needed
        }
    }

    /// Helper: Check if a value is used
    fn value_is_used(&self, value: &IRValue, used_values: &HashSet<String>) -> bool {
        match value {
            IRValue::Variable(name) => used_values.contains(name),
            IRValue::Register(reg) => used_values.contains(&format!("r{}", reg)),
            IRValue::GlobalVariable(name) => used_values.contains(name),
            _ => true,
        }
    }

    /// Helper: Check if instruction has side effects
    fn has_side_effects(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::Store { .. }
                | Instruction::Call { .. }
                | Instruction::Print(_)
                | Instruction::Allocate { .. }
                | Instruction::Deallocate { .. }
                | Instruction::ArraySet { .. }
                | Instruction::FieldSet { .. }
                | Instruction::Return(_)
                | Instruction::Jump(_)
                | Instruction::JumpIf { .. }
                | Instruction::JumpIfNot { .. }
        )
    }

    fn equality_saturation(&mut self, function: &mut IRFunction) -> IRResult<()> {
        for block in function.cfg.blocks_iter_mut() {
            for inst in &mut block.instructions {
                if let Instruction::Binary {
                    op,
                    left,
                    right,
                    result,
                } = inst
                {
                    if let Some(simplified) = try_simplify_binary(op, left, right) {
                        match simplified {
                            SimplifiedExpr::Value(val) => {
                                *inst = Instruction::Move {
                                    source: val,
                                    dest: result.clone(),
                                };
                                self.stats.equality_saturation_applied += 1;
                            }
                            SimplifiedExpr::Binary {
                                op: new_op,
                                left: new_left,
                                right: new_right,
                            } => {
                                if *op != new_op || *left != new_left || *right != new_right {
                                    *op = new_op;
                                    *left = new_left;
                                    *right = new_right;
                                    self.stats.equality_saturation_applied += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for IROptimizer {
    fn default() -> Self {
        Self::new(OptimizationLevel::Standard)
    }
}

struct RegisterPlanOutcome {
    reused: usize,
}

struct RegisterAllocatorState {
    next_register: u32,
    free_list: Vec<u32>,
    max_assigned: u32,
    reused: usize,
}

impl RegisterAllocatorState {
    fn new() -> Self {
        Self {
            next_register: 0,
            free_list: Vec::new(),
            max_assigned: 0,
            reused: 0,
        }
    }

    fn allocate_new(&mut self) -> u32 {
        if let Some(reg) = self.free_list.pop() {
            self.reused += 1;
            reg
        } else {
            let reg = self.next_register;
            self.next_register += 1;
            if reg > self.max_assigned {
                self.max_assigned = reg;
            }
            reg
        }
    }

    fn release(&mut self, reg: u32) {
        self.free_list.push(reg);
    }

    fn consume_use(&mut self, original: u32, new_reg: u32, remaining: &mut HashMap<u32, usize>) {
        if let Some(count) = remaining.get_mut(&original) {
            if *count > 0 {
                *count -= 1;
                if *count == 0 {
                    self.release(new_reg);
                }
            }
        }
    }

    fn max_register_index(&self) -> u32 {
        if self.next_register == 0 {
            0
        } else {
            self.max_assigned.saturating_add(1)
        }
    }

    fn reused(&self) -> usize {
        self.reused
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruction::{BasicBlock, Label},
        value::IRValue,
    };

    #[test]
    fn test_constant_folding() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Basic);
        let mut function = IRFunction::new("test", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("entry"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Integer(5),
            right: IRValue::Integer(3),
            result: IRValue::Register(1),
        });

        function.add_block(block);

        optimizer.optimize_function(&mut function).unwrap();

        let block = function.get_block("entry").unwrap();
        assert_eq!(block.instructions.len(), 1);

        if let Instruction::Move { source, dest } = &block.instructions[0] {
            assert_eq!(*source, IRValue::Integer(8));
            assert_eq!(*dest, IRValue::Register(1));
        } else {
            panic!("Expected move instruction after constant folding");
        }
    }

    #[test]
    fn test_redundant_move_elimination() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Basic);
        let mut function = IRFunction::new("test", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("entry"));
        block.add_instruction(Instruction::Move {
            source: IRValue::Register(1),
            dest: IRValue::Register(1),
        });

        function.add_block(block);

        let original_count = function.get_block("entry").unwrap().instructions.len();
        optimizer.optimize_function(&mut function).unwrap();

        let block = function.get_block("entry").unwrap();
        assert_eq!(block.instructions.len(), original_count - 1);
        assert_eq!(optimizer.stats.redundant_moves_eliminated, 1);
    }

    #[test]
    fn test_strength_reduction() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("test", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("entry"));
        // Multiply by 8 (power of 2)
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(1),
            right: IRValue::Integer(8),
            result: IRValue::Register(2),
        });
        // Add a return instruction to make the result used
        block.add_instruction(Instruction::Return(Some(IRValue::Register(2))));

        function.add_block(block);

        optimizer.optimize_function(&mut function).unwrap();

        let block = function.get_block("entry").unwrap();
        assert!(
            !block.instructions.is_empty(),
            "Block should have instructions after optimization"
        );

        if let Instruction::Binary { op, right, .. } = &block.instructions[0] {
            assert_eq!(*op, BinaryOp::LeftShift);
            assert_eq!(*right, IRValue::Integer(3)); // log2(8) = 3
        } else {
            panic!("Expected left shift instruction after strength reduction");
        }
    }

    #[test]
    fn register_pressure_plan_reduces_virtual_register_count() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("pressure", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("loop_body"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(0),
            right: IRValue::Integer(1),
            result: IRValue::Register(1),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(1),
            right: IRValue::Integer(2),
            result: IRValue::Register(2),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(2),
            right: IRValue::Integer(3),
            result: IRValue::Register(3),
        });
        block.add_instruction(Instruction::Return(Some(IRValue::Register(3))));
        function.add_block(block);
        function.cfg.entry_block = Some("loop_body".to_string());
        function.register_count = 4;

        let plan = RegisterPressurePlan {
            target_register_budget: 2,
        };
        let outcome = optimizer
            .apply_register_pressure_plan(&mut function, &plan)
            .expect("plan result");
        assert!(outcome.is_some());
        assert!(function.register_count < 4);
    }

    #[test]
    fn lens_superoptimizer_folds_constant_add_chain() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("loop_fn", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("loop_body"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(0),
            right: IRValue::Integer(2),
            result: IRValue::Register(1),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(1),
            right: IRValue::Integer(3),
            result: IRValue::Register(2),
        });
        block.add_instruction(Instruction::Return(Some(IRValue::Register(2))));
        function.add_block(block);
        function.cfg.entry_block = Some("loop_body".to_string());
        function.register_count = 3;

        optimizer.optimize_function(&mut function).unwrap();
        let block = function.get_block("loop_body").unwrap();
        assert_eq!(block.instructions.len(), 1);
        match &block.instructions[0] {
            Instruction::Binary {
                op, left, right, ..
            } => {
                assert_eq!(*op, BinaryOp::Add);
                assert_eq!(*left, IRValue::Register(0));
                assert_eq!(*right, IRValue::Integer(5));
            }
            other => panic!("expected fused add, got {:?}", other),
        }
        assert!(optimizer.stats.lens_superoptimizer_rewrites >= 1);
    }

    #[test]
    fn lens_superoptimizer_combines_multiply_chain() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("mul_loop", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("loop_step"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(0),
            right: IRValue::Integer(2),
            result: IRValue::Register(1),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(1),
            right: IRValue::Integer(4),
            result: IRValue::Register(2),
        });
        block.add_instruction(Instruction::Return(Some(IRValue::Register(2))));
        function.add_block(block);
        function.cfg.entry_block = Some("loop_step".to_string());
        function.register_count = 3;

        optimizer.optimize_function(&mut function).unwrap();
        let block = function.get_block("loop_step").unwrap();
        assert_eq!(block.instructions.len(), 1);
        match &block.instructions[0] {
            Instruction::Binary {
                op, left, right, ..
            } => {
                assert_eq!(*left, IRValue::Register(0));
                match op {
                    BinaryOp::Multiply => assert_eq!(*right, IRValue::Integer(8)),
                    BinaryOp::LeftShift => assert_eq!(*right, IRValue::Integer(3)),
                    other => panic!("unexpected op {:?}", other),
                }
            }
            other => panic!("expected fused multiply, got {:?}", other),
        }
    }

    #[test]
    fn test_equality_saturation_eliminates_add_zero() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = IRFunction::new("simplify", IRType::Integer);

        let mut block = BasicBlock::new(Label::new("entry"));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(1),
            right: IRValue::Integer(0),
            result: IRValue::Register(2),
        });
        block.add_instruction(Instruction::Return(Some(IRValue::Register(2))));
        function.add_block(block);

        optimizer.optimize_function(&mut function).unwrap();
        let block = function.get_block("entry").unwrap();
        match &block.instructions[0] {
            Instruction::Move { source, dest } => {
                assert_eq!(*source, IRValue::Register(1));
                assert_eq!(*dest, IRValue::Register(2));
            }
            other => panic!("expected move after equality saturation, found {:?}", other),
        }
    }

    #[test]
    fn simd_auto_requires_hot_loops() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = make_arithmetic_function("entry", false);

        let mut profile = HardwareProfile::default();
        profile.simd_policy = SimdPolicy::Auto;

        optimizer
            .optimize_function_with_profile(&mut function, &profile)
            .unwrap();

        assert_eq!(function.simd_metadata.mode, SimdMode::Scalar);
        assert_eq!(
            function.simd_metadata.reason,
            SimdDecisionReason::AutoNoHotLoops
        );
    }

    #[test]
    fn simd_auto_vectorizes_loop_body() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = make_arithmetic_function("loop_body", true);

        let mut profile = HardwareProfile::default();
        profile.simd_policy = SimdPolicy::Auto;
        profile.max_vector_bits = Some(256);

        optimizer
            .optimize_function_with_profile(&mut function, &profile)
            .unwrap();

        assert_eq!(function.simd_metadata.mode, SimdMode::Vectorized);
        assert_eq!(
            function.simd_metadata.reason,
            SimdDecisionReason::AutoHotLoop
        );
    }

    #[test]
    fn simd_max_forces_vector_mode() {
        let mut optimizer = IROptimizer::new(OptimizationLevel::Standard);
        let mut function = make_arithmetic_function("entry", false);

        let mut profile = HardwareProfile::default();
        profile.simd_policy = SimdPolicy::Max;

        optimizer
            .optimize_function_with_profile(&mut function, &profile)
            .unwrap();

        assert_eq!(function.simd_metadata.mode, SimdMode::Vectorized);
        assert_eq!(function.simd_metadata.reason, SimdDecisionReason::ForcedMax);
    }

    fn make_arithmetic_function(label: &str, mark_loop: bool) -> IRFunction {
        let mut function = IRFunction::new("simd_fn", IRType::Integer);
        let mut block = BasicBlock::new(Label::new(if mark_loop {
            format!("{}_loop", label)
        } else {
            label.to_string()
        }));
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(0),
            right: IRValue::Integer(1),
            result: IRValue::Register(1),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Multiply,
            left: IRValue::Register(1),
            right: IRValue::Integer(2),
            result: IRValue::Register(2),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Subtract,
            left: IRValue::Register(2),
            right: IRValue::Integer(3),
            result: IRValue::Register(3),
        });
        block.add_instruction(Instruction::Binary {
            op: BinaryOp::Add,
            left: IRValue::Register(3),
            right: IRValue::Integer(4),
            result: IRValue::Register(4),
        });
        block.add_instruction(Instruction::Return(Some(IRValue::Register(4))));
        function.add_block(block);
        function.cfg.entry_block = Some(if mark_loop {
            format!("{}_loop", label)
        } else {
            label.to_string()
        });
        function.register_count = 5;
        function
    }
}

#[derive(Default)]
struct SimdOpportunitySummary {
    arithmetic_ops: usize,
    memory_ops: usize,
    vector_ops: usize,
}
