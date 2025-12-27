use std::{
    collections::HashMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(
    name = "abi_guard",
    about = "Seen stdlib ABI snapshot + verification tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Produce a JSON snapshot of module hashes (and optionally update a lock file).
    Snapshot(SnapshotArgs),
    /// Verify that the current sources match the hashes stored in a lock file.
    Verify(VerifyArgs),
}

#[derive(Args, Debug)]
struct SnapshotArgs {
    /// Path to Seen.toml describing the package modules.
    #[arg(long)]
    manifest: PathBuf,
    /// Output JSON file for the snapshot.
    #[arg(long)]
    output: PathBuf,
    /// Optional lock file (Seen.lock) to update with the new hashes.
    #[arg(long)]
    lock: Option<PathBuf>,
    /// Write the computed hashes back into the provided lock file.
    #[arg(long, default_value_t = false)]
    update_lock: bool,
}

#[derive(Args, Debug)]
struct VerifyArgs {
    /// Path to Seen.toml describing the package modules.
    #[arg(long)]
    manifest: PathBuf,
    /// Lock file containing the expected hashes.
    #[arg(long)]
    lock: PathBuf,
}

#[derive(Deserialize)]
struct ManifestToml {
    project: ProjectSection,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    version: String,
    modules: Vec<String>,
}

#[derive(Serialize)]
struct AbiSnapshot {
    schema_version: u32,
    package: String,
    version: String,
    timestamp: String,
    modules: Vec<SnapshotModule>,
}

#[derive(Serialize)]
struct SnapshotModule {
    path: String,
    sha256: String,
}

#[derive(Default, Serialize, Deserialize)]
struct LockFile {
    version: u32,
    modules: Vec<LockModule>,
}

#[derive(Clone, Serialize, Deserialize)]
struct LockModule {
    path: String,
    hash: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Snapshot(args) => run_snapshot(&args),
        Command::Verify(args) => run_verify(&args),
    }
}

fn run_snapshot(args: &SnapshotArgs) -> Result<()> {
    let manifest = load_manifest(&args.manifest)?;
    let manifest_dir = args
        .manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let module_hashes = hash_modules(&manifest.project.modules, &manifest_dir)?;

    let snapshot = AbiSnapshot {
        schema_version: 1,
        package: manifest.project.name.clone(),
        version: manifest.project.version.clone(),
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        modules: module_hashes
            .iter()
            .map(|(path, hash)| SnapshotModule {
                path: path.clone(),
                sha256: hash.clone(),
            })
            .collect(),
    };

    write_snapshot(&args.output, &snapshot)?;

    if let Some(lock_path) = &args.lock {
        if args.update_lock {
            update_lock(lock_path, &manifest.project.modules, &module_hashes)?;
        }
    }

    Ok(())
}

fn run_verify(args: &VerifyArgs) -> Result<()> {
    let manifest = load_manifest(&args.manifest)?;
    let manifest_dir = args
        .manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let expected = load_lock(&args.lock)?;
    let expected_map: HashMap<_, _> = expected
        .modules
        .into_iter()
        .map(|module| (module.path, module.hash))
        .collect();

    for module in &manifest.project.modules {
        let actual = hash_file(&manifest_dir.join(module))
            .with_context(|| format!("failed to hash module {}", module))?;
        match expected_map.get(module) {
            Some(hash) if hash == &actual => continue,
            Some(_) => {
                return Err(anyhow!(
                    "hash mismatch for {module}: expected {}, got {}",
                    expected_map.get(module).unwrap(),
                    actual
                ));
            }
            None => {
                return Err(anyhow!(
                    "module {module} missing from lock file {}",
                    args.lock.display()
                ));
            }
        }
    }

    println!(
        "✅ ABI verified for {} ({} modules)",
        manifest.project.name,
        manifest.project.modules.len()
    );
    Ok(())
}

fn load_manifest(path: &Path) -> Result<ManifestToml> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    let manifest: ManifestToml = toml::from_str(&contents)
        .with_context(|| format!("invalid manifest {}", path.display()))?;
    if manifest.project.modules.is_empty() {
        return Err(anyhow!("manifest {} defines no modules", path.display()));
    }
    Ok(manifest)
}

fn hash_modules(modules: &[String], base: &Path) -> Result<Vec<(String, String)>> {
    let mut results = Vec::with_capacity(modules.len());
    for module in modules {
        let sha = hash_file(&base.join(module))
            .with_context(|| format!("failed to hash module {}", module))?;
        results.push((module.clone(), sha));
    }
    Ok(results)
}

fn hash_file(path: &Path) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("unable to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_snapshot(path: &Path, snapshot: &AbiSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data =
        serde_json::to_vec_pretty(snapshot).context("failed to serialize ABI snapshot JSON")?;
    fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn load_lock(path: &Path) -> Result<LockFile> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let lock: LockFile = toml::from_str(&contents)
        .with_context(|| format!("invalid lock file {}", path.display()))?;
    Ok(lock)
}

fn update_lock(
    lock_path: &Path,
    module_order: &[String],
    module_hashes: &[(String, String)],
) -> Result<()> {
    let mut lock = if lock_path.exists() {
        load_lock(lock_path)?
    } else {
        LockFile {
            version: 1,
            modules: Vec::new(),
        }
    };
    lock.version = 1;

    let mut map: HashMap<String, LockModule> = lock
        .modules
        .into_iter()
        .map(|module| (module.path.clone(), module))
        .collect();

    for (path, hash) in module_hashes {
        map.insert(
            path.clone(),
            LockModule {
                path: path.clone(),
                hash: hash.clone(),
            },
        );
    }

    lock.modules = module_order
        .iter()
        .filter_map(|path| map.remove(path))
        .collect();

    let toml_data =
        toml::to_string_pretty(&lock).context("failed to serialize lock file contents")?;
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(lock_path, toml_data)
        .with_context(|| format!("failed to write {}", lock_path.display()))?;
    Ok(())
}
