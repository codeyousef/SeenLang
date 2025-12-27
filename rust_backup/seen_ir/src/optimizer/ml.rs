use crate::function::{IRFunction, InlineHint, RegisterPressureClass};
use serde::Deserialize;
use serde_json::json;
use indexmap::IndexMap;
use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

// Deterministic map to stabilize learned optimizer decisions
type HashMap<K, V> = IndexMap<K, V>;

#[derive(Debug, Clone, Deserialize)]
struct ModelWeights {
    bias: f64,
    instruction_weight: f64,
    call_weight: f64,
    loop_weight: f64,
    register_weight: f64,
    depth_weight: f64,
}

impl ModelWeights {
    fn score(&self, f: &FunctionFeatures) -> f64 {
        self.bias
            + self.instruction_weight * f.instruction_count as f64
            + self.call_weight * f.call_count as f64
            + self.loop_weight * f.loop_blocks as f64
            + self.register_weight * f.register_count as f64
            + self.depth_weight * f.loop_depth as f64
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ThresholdConfig {
    #[serde(default = "default_inline_threshold")]
    inline: f64,
    #[serde(default = "default_register_medium_threshold")]
    register_medium: f64,
    #[serde(default = "default_register_high_threshold")]
    register_high: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            inline: default_inline_threshold(),
            register_medium: default_register_medium_threshold(),
            register_high: default_register_high_threshold(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct HeuristicConfig {
    #[serde(default = "default_inline_weights")]
    inline: ModelWeights,
    #[serde(default = "default_register_weights")]
    register: ModelWeights,
    #[serde(default)]
    thresholds: ThresholdConfig,
}

impl Default for HeuristicConfig {
    fn default() -> Self {
        Self {
            inline: default_inline_weights(),
            register: default_register_weights(),
            thresholds: ThresholdConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyWeights {
    bias: f64,
    instruction_weight: f64,
    call_weight: f64,
    loop_weight: f64,
    register_weight: f64,
    depth_weight: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ConfigFormat {
    Full(HeuristicConfig),
    Legacy(LegacyWeights),
}

#[derive(Debug, Clone, Copy)]
struct PerfAdjustment {
    inline_delta: f64,
    register_delta: f64,
}

impl Default for PerfAdjustment {
    fn default() -> Self {
        Self {
            inline_delta: 0.0,
            register_delta: 0.0,
        }
    }
}

pub struct MlHeuristicModel {
    inline: ModelWeights,
    register: ModelWeights,
    thresholds: ThresholdConfig,
    adjustment: PerfAdjustment,
}

impl MlHeuristicModel {
    pub fn from_env() -> Option<Self> {
        let path = env::var("SEEN_ML_HEURISTICS").ok()?;
        let contents = fs::read_to_string(&path).ok()?;
        let raw: ConfigFormat = serde_json::from_str(&contents).ok()?;
        let cfg = match raw {
            ConfigFormat::Full(cfg) => cfg,
            ConfigFormat::Legacy(weights) => HeuristicConfig {
                inline: ModelWeights {
                    bias: weights.bias,
                    instruction_weight: weights.instruction_weight,
                    call_weight: weights.call_weight,
                    loop_weight: weights.loop_weight,
                    register_weight: weights.register_weight,
                    depth_weight: weights.depth_weight,
                },
                register: ModelWeights {
                    bias: weights.bias * 0.25,
                    instruction_weight: weights.instruction_weight * 0.25,
                    call_weight: weights.call_weight * 0.25,
                    loop_weight: weights.loop_weight * 0.25,
                    register_weight: weights.register_weight * 1.5,
                    depth_weight: weights.depth_weight * 0.25,
                },
                thresholds: ThresholdConfig::default(),
            },
        };
        Some(Self::from_config(cfg))
    }

    fn from_config(config: HeuristicConfig) -> Self {
        let adjustment = PerfAdjustment::from_baseline();
        Self {
            inline: config.inline,
            register: config.register,
            thresholds: config.thresholds,
            adjustment,
        }
    }

    pub fn decide(&self, features: &FunctionFeatures) -> MlDecision {
        let inline_score = self.inline.score(features) + self.adjustment.inline_delta;
        let register_score = self.register.score(features) + self.adjustment.register_delta;

        let inline_hint = if inline_score >= self.thresholds.inline {
            InlineHint::AlwaysInline
        } else if inline_score <= -self.thresholds.inline {
            InlineHint::NeverInline
        } else {
            InlineHint::Auto
        };

        let register_class = if register_score >= self.thresholds.register_high {
            RegisterPressureClass::High
        } else if register_score >= self.thresholds.register_medium {
            RegisterPressureClass::Medium
        } else {
            RegisterPressureClass::Low
        };

        let register_plan =
            RegisterPressurePlan::for_class(register_class, features.register_count as u32);

        let aggressive_allowed =
            inline_score >= 0.0 || features.loop_blocks == 0 || features.instruction_count < 32;

        MlDecision {
            inline_hint,
            register_class,
            register_plan,
            aggressive_allowed,
        }
    }
}

impl Default for MlHeuristicModel {
    fn default() -> Self {
        Self::from_config(HeuristicConfig::default())
    }
}

impl PerfAdjustment {
    fn from_baseline() -> Self {
        let path = env::var("SEEN_PERF_BASELINE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                let default = PathBuf::from("scripts/perf_baseline_report.json");
                if default.exists() {
                    Some(default)
                } else {
                    None
                }
            });
        let Some(path) = path else {
            return PerfAdjustment::default();
        };
        let bytes = match fs::read(&path) {
            Ok(data) => data,
            Err(err) => {
                eprintln!("warning: failed to read PB-Perf report {path:?}: {err}");
                return PerfAdjustment::default();
            }
        };
        #[derive(Deserialize)]
        struct PerfStats {
            mean_time_ms: f64,
        }
        #[derive(Deserialize)]
        struct TaskReport {
            stats: Option<PerfStats>,
        }
        #[derive(Deserialize)]
        struct PerfReport {
            tasks: Vec<TaskReport>,
        }
        let report: PerfReport = match serde_json::from_slice(&bytes) {
            Ok(rep) => rep,
            Err(err) => {
                eprintln!("warning: PB-Perf report at {path:?} is not valid JSON: {err}");
                return PerfAdjustment::default();
            }
        };

        let mut totals = Vec::new();
        for task in report.tasks.iter().filter_map(|t| t.stats.as_ref()) {
            totals.push(task.mean_time_ms);
        }
        if totals.is_empty() {
            return PerfAdjustment::default();
        }
        let avg = totals.iter().sum::<f64>() / totals.len() as f64;
        if avg == 0.0 {
            return PerfAdjustment::default();
        }
        let mut hotness = Vec::new();
        for val in totals {
            if val > avg {
                hotness.push((val / avg) - 1.0);
            }
        }
        if hotness.is_empty() {
            return PerfAdjustment::default();
        }
        let mean_hot = hotness.iter().sum::<f64>() / hotness.len() as f64;
        PerfAdjustment {
            inline_delta: mean_hot * 0.5,
            register_delta: mean_hot * 0.25,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionFeatures {
    pub instruction_count: usize,
    pub call_count: usize,
    pub loop_blocks: usize,
    pub register_count: usize,
    pub loop_depth: usize,
    pub block_count: usize,
}

impl FunctionFeatures {
    pub fn from_function(function: &IRFunction) -> Self {
        use crate::instruction::Instruction;
        use crate::value::IRValue;

        let mut features = FunctionFeatures::default();
        features.block_count = function.cfg.blocks_iter().count();
        for block in function.cfg.blocks_iter() {
            if block.label.0.contains("loop") {
                features.loop_blocks += 1;
            }
            for inst in &block.instructions {
                features.instruction_count += 1;
                match inst {
                    Instruction::Binary { result, .. }
                    | Instruction::Unary { result, .. }
                    | Instruction::Move { dest: result, .. }
                    | Instruction::Load { dest: result, .. }
                    | Instruction::Allocate { result, .. }
                    | Instruction::ArrayAccess { result, .. }
                    | Instruction::ArrayLength { result, .. }
                    | Instruction::FieldAccess { result, .. }
                    | Instruction::GetEnumTag { result, .. }
                    | Instruction::GetEnumField { result, .. }
                    | Instruction::Cast { result, .. }
                    | Instruction::TypeCheck { result, .. }
                    | Instruction::ConstructObject { result, .. }
                    | Instruction::ConstructEnum { result, .. }
                    | Instruction::Scoped { result, .. }
                    | Instruction::Spawn { result, .. } => {
                        if let IRValue::Register(reg) = result {
                            features.register_count =
                                features.register_count.max((*reg as usize) + 1);
                        }
                    }
                    Instruction::ChannelSelect {
                        payload_result,
                        index_result,
                        status_result,
                        ..
                    } => {
                        for res in [payload_result, index_result, status_result] {
                            if let IRValue::Register(reg) = res {
                                features.register_count =
                                    features.register_count.max((*reg as usize) + 1);
                            }
                        }
                    }
                    Instruction::Call { result, .. }
                    | Instruction::VirtualCall { result, .. }
                    | Instruction::StaticCall { result, .. } => {
                        features.call_count += 1;
                        if let Some(IRValue::Register(reg)) = result {
                            features.register_count =
                                features.register_count.max((*reg as usize) + 1);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(term) = &block.terminator {
                if let Instruction::Call { result, .. }
                | Instruction::VirtualCall { result, .. }
                | Instruction::StaticCall { result, .. } = term
                {
                    features.call_count += 1;
                    if let Some(IRValue::Register(reg)) = result {
                        features.register_count = features.register_count.max((*reg as usize) + 1);
                    }
                } else if let Instruction::Return(Some(IRValue::Register(reg))) = term {
                    features.register_count = features.register_count.max((*reg as usize) + 1);
                }
            }
        }
        features.register_count = features
            .register_count
            .max(function.register_count as usize);
        features.loop_depth = features.loop_blocks.saturating_sub(1);
        features
    }
}

pub struct OptimizationProfileWriter {
    file: BufWriter<File>,
}

impl OptimizationProfileWriter {
    pub fn from_env() -> Option<Self> {
        let path = env::var("SEEN_OPT_PROFILE").ok()?;
        let dir = Path::new(&path).parent().map(Path::to_path_buf);
        if let Some(dir) = dir {
            let _ = fs::create_dir_all(dir);
        }
        let file = File::create(PathBuf::from(path)).ok()?;
        Some(Self {
            file: BufWriter::new(file),
        })
    }

    pub fn record(&mut self, func_name: &str, features: &FunctionFeatures) {
        let entry = json!({
            "function": func_name,
            "instructions": features.instruction_count,
            "calls": features.call_count,
            "loop_blocks": features.loop_blocks,
            "registers": features.register_count,
            "loop_depth": features.loop_depth,
            "blocks": features.block_count,
        });
        if let Err(err) = writeln!(self.file, "{}", entry.to_string()) {
            eprintln!(
                "warning: failed to write optimization profile entry: {}",
                err
            );
        }
    }
}

pub struct DecisionLogger {
    file: BufWriter<File>,
    reward: f64,
}

impl DecisionLogger {
    pub fn from_env() -> Option<Self> {
        let path = env::var("SEEN_ML_DECISION_LOG").ok()?;
        let reward = env::var("SEEN_ML_REWARD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let file = File::create(&path).ok()?;
        Some(Self {
            file: BufWriter::new(file),
            reward,
        })
    }

    pub fn record(&mut self, func_name: &str, features: &FunctionFeatures, decision: &MlDecision) {
        let entry = json!({
            "function": func_name,
            "inline_hint": decision.inline_hint.as_str(),
            "register_class": decision.register_class.as_str(),
            "register_budget": decision.register_plan.as_ref().map(|p| p.target_register_budget),
            "reward": self.reward,
            "features": {
                "instructions": features.instruction_count,
                "calls": features.call_count,
                "loop_blocks": features.loop_blocks,
                "registers": features.register_count,
                "loop_depth": features.loop_depth,
                "blocks": features.block_count,
            }
        });
        if let Err(err) = writeln!(self.file, "{}", entry.to_string()) {
            eprintln!("warning: failed to write decision log entry: {}", err);
            return;
        }
        if let Err(err) = self.file.flush() {
            eprintln!("warning: failed to flush decision log entry: {}", err);
        }
    }

    #[cfg(test)]
    pub fn for_path(path: &Path, reward: f64) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        Ok(Self {
            file: BufWriter::new(file),
            reward,
        })
    }
}

pub struct DecisionReplay {
    entries: HashMap<String, ReplayEntry>,
}

impl DecisionReplay {
    pub fn from_env() -> Option<Self> {
        let path = env::var("SEEN_ML_DECISION_REPLAY").ok()?;
        let bytes = fs::read(&path).ok()?;
        let decoded: Vec<ReplayEntry> = serde_json::from_slice(&bytes).ok()?;
        let mut entries = HashMap::new();
        for entry in decoded {
            entries.insert(entry.function.clone(), entry);
        }
        Some(Self { entries })
    }

    pub fn apply(&self, name: &str, decision: &mut MlDecision) {
        if let Some(entry) = self.entries.get(name) {
            if let Some(hint) = entry.inline_hint.as_deref() {
                if let Some(parsed) = InlineHint::from_str(hint) {
                    decision.inline_hint = parsed;
                }
            }
            if let Some(class) = entry.register_class.as_deref() {
                if let Some(parsed) = RegisterPressureClass::from_str(class) {
                    decision.register_class = parsed;
                }
            }
            if let Some(budget) = entry.register_budget {
                decision.register_plan = Some(RegisterPressurePlan::new(budget));
            }
        }
    }

    #[cfg(test)]
    pub fn from_path(path: &Path) -> Option<Self> {
        let bytes = fs::read(path).ok()?;
        let decoded: Vec<ReplayEntry> = serde_json::from_slice(&bytes).ok()?;
        let mut entries = HashMap::new();
        for entry in decoded {
            entries.insert(entry.function.clone(), entry);
        }
        Some(Self { entries })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ReplayEntry {
    function: String,
    #[serde(default)]
    inline_hint: Option<String>,
    #[serde(default)]
    register_class: Option<String>,
    #[serde(default)]
    register_budget: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MlDecision {
    pub inline_hint: InlineHint,
    pub register_class: RegisterPressureClass,
    pub register_plan: Option<RegisterPressurePlan>,
    pub aggressive_allowed: bool,
}

impl MlDecision {
    pub fn from_level(
        level: crate::optimizer::OptimizationLevel,
        features: &FunctionFeatures,
    ) -> Self {
        let aggressive_allowed =
            level.should_run_pass(crate::optimizer::OptimizationLevel::Aggressive);
        Self {
            inline_hint: InlineHint::Auto,
            register_class: RegisterPressureClass::Unknown,
            register_plan: None,
            aggressive_allowed: aggressive_allowed || features.instruction_count < 64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterPressurePlan {
    pub target_register_budget: u32,
}

impl RegisterPressurePlan {
    pub fn for_class(class: RegisterPressureClass, current: u32) -> Option<Self> {
        match class {
            RegisterPressureClass::High => {
                if current <= 48 {
                    None
                } else {
                    Some(Self {
                        target_register_budget: 48,
                    })
                }
            }
            RegisterPressureClass::Medium => {
                if current <= 96 {
                    None
                } else {
                    Some(Self {
                        target_register_budget: 96,
                    })
                }
            }
            _ => None,
        }
    }

    pub fn new(target_register_budget: u32) -> Self {
        Self {
            target_register_budget,
        }
    }
}

const fn default_inline_threshold() -> f64 {
    0.0
}

const fn default_register_medium_threshold() -> f64 {
    0.5
}

const fn default_register_high_threshold() -> f64 {
    1.0
}

fn default_inline_weights() -> ModelWeights {
    ModelWeights {
        bias: -4.5,
        instruction_weight: 0.01,
        call_weight: 0.4,
        loop_weight: 0.7,
        register_weight: 0.01,
        depth_weight: 0.35,
    }
}

fn default_register_weights() -> ModelWeights {
    ModelWeights {
        bias: -1.5,
        instruction_weight: 0.02,
        call_weight: 0.05,
        loop_weight: 0.1,
        register_weight: 0.4,
        depth_weight: 0.05,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimizer::OptimizationLevel;
    use std::fs;
    #[test]
    fn inline_hint_switches_when_score_positive() {
        let config = HeuristicConfig {
            inline: ModelWeights {
                bias: -1.0,
                instruction_weight: 0.02,
                call_weight: 0.5,
                loop_weight: 0.5,
                register_weight: 0.01,
                depth_weight: 0.1,
            },
            register: default_register_weights(),
            thresholds: ThresholdConfig {
                inline: 0.0,
                register_medium: 0.5,
                register_high: 1.0,
            },
        };
        let model = MlHeuristicModel::from_config(config);
        let features = FunctionFeatures {
            instruction_count: 100,
            call_count: 5,
            loop_blocks: 2,
            register_count: 50,
            loop_depth: 1,
            block_count: 4,
        };
        let decision = model.decide(&features);
        assert!(matches!(decision.inline_hint, InlineHint::AlwaysInline));
    }

    #[test]
    fn register_plan_triggers_for_high_pressure() {
        let mut config = HeuristicConfig::default();
        config.register = ModelWeights {
            bias: -2.0,
            instruction_weight: 0.0,
            call_weight: 0.0,
            loop_weight: 0.0,
            register_weight: 0.05,
            depth_weight: 0.0,
        };
        let model = MlHeuristicModel::from_config(config);
        let features = FunctionFeatures {
            instruction_count: 10,
            call_count: 0,
            loop_blocks: 0,
            register_count: 200,
            loop_depth: 0,
            block_count: 1,
        };
        let decision = model.decide(&features);
        assert!(matches!(
            decision.register_class,
            RegisterPressureClass::High
        ));
        assert!(decision.register_plan.is_some());
    }

    #[test]
    fn decision_logger_writes_json_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log_path = dir.path().join("decisions.ndjson");
        let mut logger = DecisionLogger::for_path(&log_path, 2.5).expect("logger path should open");
        let mut decision =
            MlDecision::from_level(OptimizationLevel::Basic, &FunctionFeatures::default());
        decision.inline_hint = InlineHint::AlwaysInline;
        decision.register_class = RegisterPressureClass::High;
        decision.register_plan = Some(RegisterPressurePlan::new(32));
        logger.record("foo", &FunctionFeatures::default(), &decision);
        drop(logger);
        let contents = fs::read_to_string(&log_path).expect("log contents");
        assert!(
            contents.contains("\"function\":\"foo\""),
            "log contents missing function entry: {}",
            contents
        );
        assert!(contents.contains("\"register_budget\":32"));
        assert!(contents.contains("\"reward\":2.5"));
    }

    #[test]
    fn decision_replay_overrides_decision() {
        let dir = tempfile::tempdir().expect("temp dir");
        let replay_path = dir.path().join("replay.json");
        let payload = r#"[{"function":"foo","inline_hint":"never","register_class":"medium","register_budget":88}]"#;
        fs::write(&replay_path, payload).expect("write replay payload");
        let replay = DecisionReplay::from_path(&replay_path).expect("replay");

        let mut decision =
            MlDecision::from_level(OptimizationLevel::Basic, &FunctionFeatures::default());
        replay.apply("foo", &mut decision);
        assert!(matches!(decision.inline_hint, InlineHint::NeverInline));
        assert!(matches!(
            decision.register_class,
            RegisterPressureClass::Medium
        ));
        assert_eq!(
            decision
                .register_plan
                .as_ref()
                .map(|p| p.target_register_budget),
            Some(88)
        );
    }
}
