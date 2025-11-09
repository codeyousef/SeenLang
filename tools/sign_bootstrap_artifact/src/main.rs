use std::{
    convert::TryInto,
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::Parser;
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(
    name = "sign_bootstrap_artifact",
    about = "Emit manifest + hash attestation for Stage2/Stage3 bootstrap outputs"
)]
struct Args {
    /// Path to the Stage2 binary emitted by Stage1.
    #[arg(long)]
    stage2: PathBuf,
    /// Path to the Stage3 binary emitted by Stage2.
    #[arg(long)]
    stage3: PathBuf,
    /// Optional path to the Stage1 binary (captured for completeness).
    #[arg(long)]
    stage1: Option<PathBuf>,
    /// Output manifest path (JSON).
    #[arg(long)]
    output: PathBuf,
    /// Matrix entry identifier.
    #[arg(long)]
    entry: String,
    /// Host triple that produced the artifact.
    #[arg(long)]
    host: String,
    /// Backend used for codegen (llvm/mlir/clif/etc).
    #[arg(long)]
    backend: String,
    /// Cargo/Seen profile used (release/deterministic/etc).
    #[arg(long, default_value = "release")]
    profile: String,
    /// Optional CLI path used to kick off the bootstrap.
    #[arg(long)]
    cli_path: Option<PathBuf>,
    /// CLI version string (if omitted we attempt to query cli_path --version).
    #[arg(long)]
    cli_version: Option<String>,
    /// Optional label for the target triple (if different from host).
    #[arg(long)]
    target: Option<String>,
    /// Optional repeated backend features (e.g. ["llvm", "mlir"]).
    #[arg(long = "feature")]
    features: Vec<String>,
    /// Optional Ed25519 signing key (raw bytes or hex) to sign the manifest.
    #[arg(long)]
    signing_key: Option<PathBuf>,
    /// Optional signature output path (defaults to <manifest>.sig) when signing.
    #[arg(long)]
    signature_output: Option<PathBuf>,
}

#[derive(Serialize, Clone)]
struct FileDigest {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Serialize)]
struct Manifest {
    schema_version: u32,
    matrix_entry: String,
    host_triple: String,
    target_triple: Option<String>,
    backend: String,
    profile: String,
    features: Vec<String>,
    timestamp: String,
    git_commit: String,
    cli_path: Option<String>,
    cli_version: String,
    stage1: Option<FileDigest>,
    stage2: FileDigest,
    stage3: FileDigest,
    stage2_equals_stage3: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let manifest = build_manifest(&args)?;
    let manifest_bytes = write_manifest(&args.output, &manifest)?;
    maybe_sign_manifest(&manifest_bytes, &args)?;
    Ok(())
}

fn build_manifest(args: &Args) -> Result<Manifest> {
    let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let git_commit = query_git_commit()?;

    let stage2 = digest_file(&args.stage2)
        .with_context(|| format!("failed to hash Stage2 binary at {}", args.stage2.display()))?;
    let stage3 = digest_file(&args.stage3)
        .with_context(|| format!("failed to hash Stage3 binary at {}", args.stage3.display()))?;
    let stage1 = if let Some(path) = &args.stage1 {
        Some(
            digest_file(path)
                .with_context(|| format!("failed to hash Stage1 binary at {}", path.display()))?,
        )
    } else {
        None
    };

    let cli_version = resolve_cli_version(args)?;

    Ok(Manifest {
        schema_version: 1,
        matrix_entry: args.entry.clone(),
        host_triple: args.host.clone(),
        target_triple: args.target.clone(),
        backend: args.backend.clone(),
        profile: args.profile.clone(),
        features: args.features.clone(),
        timestamp,
        git_commit,
        cli_path: args.cli_path.as_ref().map(|p| p.display().to_string()),
        cli_version,
        stage1,
        stage2: stage2.clone(),
        stage3: stage3.clone(),
        stage2_equals_stage3: stage2.sha256 == stage3.sha256,
    })
}

fn digest_file(path: &Path) -> Result<FileDigest> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
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
    let digest = hasher.finalize();
    Ok(FileDigest {
        path: path.display().to_string(),
        size: metadata.len(),
        sha256: hex::encode(digest),
    })
}

fn query_git_commit() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("failed to execute git rev-parse HEAD")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn resolve_cli_version(args: &Args) -> Result<String> {
    if let Some(version) = &args.cli_version {
        if !version.trim().is_empty() {
            return Ok(version.trim().to_string());
        }
    }
    if let Some(path) = &args.cli_path {
        let output = Command::new(path)
            .arg("--version")
            .output()
            .with_context(|| format!("failed to execute {} --version", path.display()))?;
        if !output.status.success() {
            return Err(anyhow!(
                "{} --version failed: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("unknown")
            .trim()
            .to_string());
    }
    Ok("unknown".to_string())
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<Vec<u8>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .with_context(|| format!("failed to serialize manifest for {}", path.display()))?;
    fs::write(path, &bytes)
        .with_context(|| format!("failed to write manifest to {}", path.display()))?;
    Ok(bytes)
}

fn maybe_sign_manifest(bytes: &[u8], args: &Args) -> Result<()> {
    let key_path = match &args.signing_key {
        Some(path) => path,
        None => return Ok(()),
    };

    let signing_key = load_signing_key(key_path)
        .with_context(|| format!("failed to load signing key from {}", key_path.display()))?;
    let signature = signing_key.sign(bytes);
    let sig_hex = hex::encode(signature.to_bytes());

    let mut target_path = args.signature_output.clone();
    let sig_path = target_path.get_or_insert_with(|| {
        let mut default = args.output.with_extension("json.sig");
        if default == args.output {
            default = args.output.with_extension("sig");
        }
        default
    });

    let sig_path_ref = sig_path.as_path();

    if let Some(parent) = sig_path_ref.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(sig_path_ref, sig_hex.as_bytes())
        .with_context(|| format!("failed to write signature to {}", sig_path_ref.display()))?;

    Ok(())
}

fn load_signing_key(path: &Path) -> Result<SigningKey> {
    let raw = fs::read(path)?;
    if let Some(key) = parse_seed_from_raw(&raw) {
        return Ok(key);
    }

    let text =
        String::from_utf8(raw).context("signing key must be UTF-8 text or raw 32/64 byte seed")?;
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        bail!("signing key file was empty");
    }
    let decoded = hex::decode(&cleaned)
        .with_context(|| format!("failed to decode signing key hex from {}", path.display()))?;
    parse_seed_from_raw(&decoded)
        .ok_or_else(|| anyhow!("signing key must decode to 32 or 64 bytes"))
}

fn parse_seed_from_raw(bytes: &[u8]) -> Option<SigningKey> {
    match bytes.len() {
        32 => {
            let seed: [u8; 32] = bytes.try_into().ok()?;
            Some(SigningKey::from_bytes(&seed))
        }
        64 => {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes[..32]);
            Some(SigningKey::from_bytes(&seed))
        }
        _ => None,
    }
}
