use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessExt, System, SystemExt};

#[derive(Debug, Parser)]
#[command(author, version, about = "Seen Language performance baseline harness")]
struct Args {
    /// Path to the TOML configuration file describing benchmark tasks
    #[arg(short, long, default_value = "scripts/perf_baseline.toml")]
    config: PathBuf,

    /// Optional path for the JSON report (defaults to perf_baseline_report.json next to the config)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Emit the JSON report to stdout in addition to writing to disk
    #[arg(long)]
    json_stdout: bool,

    /// Optional baseline JSON report; regressions beyond thresholds fail with an error
    #[arg(long)]
    baseline: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    tasks: Vec<TaskConfig>,
}

#[derive(Debug, Deserialize)]
struct TaskConfig {
    name: String,
    kind: TaskKind,
    command: Vec<String>,
    #[serde(default)]
    working_dir: Option<PathBuf>,
    #[serde(default = "default_warmups")]
    warmups: u32,
    #[serde(default = "default_runs")]
    runs: u32,
    #[serde(default)]
    clean_before: bool,
    #[serde(default)]
    artifacts: Vec<PathBuf>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default = "default_threshold_pct")]
    threshold_pct: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TaskKind {
    Micro,
    Macro,
    Compile,
}

fn default_warmups() -> u32 {
    1
}

fn default_runs() -> u32 {
    3
}

fn default_threshold_pct() -> f64 {
    5.0
}

#[derive(Debug, Serialize, Deserialize)]
struct PerfReport {
    generated_at: String,
    tasks: Vec<TaskReport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskReport {
    name: String,
    kind: TaskKind,
    #[serde(default = "default_threshold_pct")]
    threshold_pct: f64,
    warmups: u32,
    runs: Vec<RunResult>,
    stats: Option<AggregateStats>,
    artifacts: Vec<ArtifactSize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunResult {
    run_index: usize,
    duration_ms: f64,
    peak_memory_kib: u64,
    status: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AggregateStats {
    mean_time_ms: f64,
    median_time_ms: f64,
    p95_time_ms: f64,
    max_peak_memory_kib: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactSize {
    path: String,
    bytes: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let config_bytes = fs::read(&args.config)
        .with_context(|| format!("Failed to read config at {}", args.config.display()))?;
    let config_str = std::str::from_utf8(&config_bytes)
        .with_context(|| format!("Config file {} is not valid UTF-8", args.config.display()))?;
    let config: Config = toml::from_str(config_str)
        .with_context(|| format!("Invalid TOML in {}", args.config.display()))?;

    if config.tasks.is_empty() {
        return Err(anyhow!("Configuration must specify at least one task"));
    }

    let report_path = args
        .output
        .clone()
        .or_else(|| default_report_path(&args.config))
        .context("Unable to determine output path")?;

    let mut task_reports = Vec::new();
    for task in &config.tasks {
        task_reports.push(run_task(task)?);
    }

    let report = PerfReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        tasks: task_reports,
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    fs::create_dir_all(
        report_path
            .parent()
            .ok_or_else(|| anyhow!("report path missing parent"))?,
    )?;
    fs::write(&report_path, &report_json)
        .with_context(|| format!("Failed to write report to {}", report_path.display()))?;

    println!("Performance baseline written to {}", report_path.display());
    for task in &report.tasks {
        if let Some(stats) = &task.stats {
            println!(
                "- {:<32} {:<7} mean={:>8.2}ms p95={:>8.2}ms maxRSS={:>8} KiB",
                task.name,
                format_kind(task.kind),
                stats.mean_time_ms,
                stats.p95_time_ms,
                stats.max_peak_memory_kib,
            );
        } else {
            println!(
                "- {:<32} {:<7} (no successful runs recorded)",
                task.name,
                format_kind(task.kind)
            );
        }
    }

    if args.json_stdout {
        println!("\n{}", report_json);
    }

    if let Some(baseline_path) = &args.baseline {
        let baseline_bytes = fs::read(baseline_path).with_context(|| {
            format!(
                "Failed to read baseline report at {}",
                baseline_path.display()
            )
        })?;
        let baseline: PerfReport = serde_json::from_slice(&baseline_bytes)
            .with_context(|| format!("Baseline {} is not valid JSON", baseline_path.display()))?;
        compare_against_baseline(&report, &baseline)?;
    }

    Ok(())
}

fn format_kind(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Micro => "micro",
        TaskKind::Macro => "macro",
        TaskKind::Compile => "compile",
    }
}

fn default_report_path(config_path: &Path) -> Option<PathBuf> {
    let parent = config_path.parent()?;
    Some(parent.join("perf_baseline_report.json"))
}

fn run_task(task: &TaskConfig) -> Result<TaskReport> {
    if task.command.is_empty() {
        return Err(anyhow!("task '{}' command must not be empty", task.name));
    }

    let mut samples = Vec::new();
    let warmups = task.warmups as usize;
    let measurements = task.runs as usize;

    if task.kind == TaskKind::Compile && task.clean_before {
        run_cargo_clean(task)?;
    }

    for idx in 0..warmups {
        let _ = run_once(task, idx, true)?;
    }

    let mut need_clean = task.kind == TaskKind::Compile && task.clean_before;
    for idx in 0..measurements {
        if need_clean {
            run_cargo_clean(task)?;
            need_clean = false;
        }

        let sample = run_once(task, idx, false)?;
        samples.push(sample);
    }

    let stats = compute_stats(&samples);
    let artifacts = collect_artifact_sizes(task)?;

    Ok(TaskReport {
        name: task.name.clone(),
        kind: task.kind,
        threshold_pct: task.threshold_pct,
        warmups: task.warmups,
        runs: samples
            .into_iter()
            .enumerate()
            .map(|(idx, sample)| RunResult {
                run_index: idx + 1,
                duration_ms: sample.duration_ms,
                peak_memory_kib: sample.peak_memory_kib,
                status: sample.status,
            })
            .collect(),
        stats,
        artifacts,
    })
}

struct RawSample {
    duration_ms: f64,
    peak_memory_kib: u64,
    status: Option<i32>,
}

fn run_once(task: &TaskConfig, idx: usize, is_warmup: bool) -> Result<RawSample> {
    let mut cmd = Command::new(&task.command[0]);
    if task.command.len() > 1 {
        cmd.args(&task.command[1..]);
    }
    if let Some(dir) = &task.working_dir {
        cmd.current_dir(dir);
    }
    if is_warmup {
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
    }
    for (key, value) in &task.env {
        cmd.env(key, value);
    }

    let start = Instant::now();
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "failed to spawn task '{}' command {:?}",
            task.name, task.command
        )
    })?;
    let pid = child.id();
    let mut system = System::new();
    let mut peak_kib = 0u64;
    let sys_pid = Pid::from(pid as usize);

    loop {
        match child.try_wait()? {
            Some(status) => {
                system.refresh_process(sys_pid);
                if let Some(proc_) = system.process(sys_pid) {
                    peak_kib = peak_kib.max(proc_.memory());
                }
                if !is_warmup && !status.success() {
                    return Err(anyhow!(
                        "task '{}' run #{} exited with status {:?}",
                        task.name,
                        idx + 1,
                        status.code()
                    ));
                }
                let duration = start.elapsed();
                return Ok(RawSample {
                    duration_ms: duration.as_secs_f64() * 1000.0,
                    peak_memory_kib: peak_kib,
                    status: status.code(),
                });
            }
            None => {
                system.refresh_process(sys_pid);
                if let Some(proc_) = system.process(sys_pid) {
                    peak_kib = peak_kib.max(proc_.memory());
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn compute_stats(samples: &[RawSample]) -> Option<AggregateStats> {
    if samples.is_empty() {
        return None;
    }
    let mut durations: Vec<f64> = samples.iter().map(|s| s.duration_ms).collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = durations.iter().copied().sum::<f64>() / durations.len() as f64;
    let median = if durations.len() % 2 == 1 {
        durations[durations.len() / 2]
    } else {
        let mid = durations.len() / 2;
        (durations[mid - 1] + durations[mid]) / 2.0
    };
    let p95_index = ((durations.len() as f64) * 0.95).ceil() as usize;
    let p95 = durations[p95_index.saturating_sub(1).min(durations.len() - 1)];
    let max_peak = samples.iter().map(|s| s.peak_memory_kib).max().unwrap_or(0);
    Some(AggregateStats {
        mean_time_ms: mean,
        median_time_ms: median,
        p95_time_ms: p95,
        max_peak_memory_kib: max_peak,
    })
}

fn collect_artifact_sizes(task: &TaskConfig) -> Result<Vec<ArtifactSize>> {
    if task.artifacts.is_empty() {
        return Ok(Vec::new());
    }
    let base = task.working_dir.as_deref().unwrap_or(Path::new("."));
    let mut sizes = Vec::new();
    for rel in &task.artifacts {
        let path = if rel.is_absolute() {
            rel.clone()
        } else {
            base.join(rel)
        };
        if path.exists() {
            let bytes = fs::metadata(&path)?.len();
            sizes.push(ArtifactSize {
                path: path.display().to_string(),
                bytes,
            });
        }
    }
    Ok(sizes)
}

fn run_cargo_clean(task: &TaskConfig) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("clean");
    if let Some(dir) = &task.working_dir {
        cmd.current_dir(dir);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(anyhow!("cargo clean failed for task '{}'", task.name));
    }
    Ok(())
}

fn compare_against_baseline(current: &PerfReport, baseline: &PerfReport) -> Result<()> {
    let baseline_map: HashMap<&str, &TaskReport> = baseline
        .tasks
        .iter()
        .map(|task| (task.name.as_str(), task))
        .collect();

    let mut regressions = Vec::new();

    for task in &current.tasks {
        let Some(current_stats) = &task.stats else {
            continue;
        };
        let Some(baseline_task) = baseline_map.get(task.name.as_str()) else {
            continue;
        };
        let Some(baseline_stats) = &baseline_task.stats else {
            continue;
        };

        let threshold = task.threshold_pct.max(0.0);
        let allowed = baseline_stats.mean_time_ms * (1.0 + threshold / 100.0);
        if current_stats.mean_time_ms > allowed {
            let delta_pct =
                ((current_stats.mean_time_ms / baseline_stats.mean_time_ms) - 1.0) * 100.0;
            regressions.push(format!(
                "{} mean {:.2}ms exceeded baseline {:.2}ms (+{:.2}% > {:.2}%)",
                task.name,
                current_stats.mean_time_ms,
                baseline_stats.mean_time_ms,
                delta_pct,
                threshold
            ));
        }
    }

    if regressions.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "Performance regressions detected:\n{}",
            regressions.join("\n")
        ))
    }
}
