use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(
    name = "sign_bootstrap_artifact",
    about = "Bootstrap manifest attestation & signature utility"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Produce a manifest for Stage outputs and optionally sign it.
    Sign(SignArgs),
    /// Verify a manifest + signature pair against an Ed25519 public key.
    Verify(VerifyArgs),
}

#[derive(Args, Debug)]
struct SignArgs {
    #[arg(long)]
    stage2: PathBuf,
    #[arg(long)]
    stage3: PathBuf,
    #[arg(long)]
    stage1: Option<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    entry: String,
    #[arg(long)]
    host: String,
    #[arg(long)]
    backend: String,
    #[arg(long, default_value = "release")]
    profile: String,
    #[arg(long)]
    cli_path: Option<PathBuf>,
    #[arg(long)]
    cli_version: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long = "feature")]
    features: Vec<String>,
    #[arg(long)]
    signing_key: Option<PathBuf>,
    #[arg(long)]
    signature_output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct VerifyArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    signature: PathBuf,
    #[arg(long)]
    public_key: PathBuf,
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
    let cli = Cli::parse();
    match cli.command {
        Command::Sign(args) => run_sign(&args),
        Command::Verify(args) => run_verify(&args),
    }
}

fn run_sign(args: &SignArgs) -> Result<()> {
    let manifest = build_manifest(args)?;
    let manifest_bytes = write_manifest(&args.output, &manifest)?;
    maybe_sign_manifest(&manifest_bytes, args)?;
    Ok(())
}

fn run_verify(args: &VerifyArgs) -> Result<()> {
    let manifest_bytes = fs::read(&args.manifest)
        .with_context(|| format!("failed to read manifest {}", args.manifest.display()))?;
    let signature = load_signature(&args.signature)?;
    let public_key = load_public_key(&args.public_key)?;

    public_key
        .verify(&manifest_bytes, &signature)
        .with_context(|| {
            format!(
                "signature verification failed for manifest {}",
                args.manifest.display()
            )
        })?;

    println!(
        "✅ signature valid for {} using {}",
        args.manifest.display(),
        args.public_key.display()
    );

    Ok(())
}

fn build_manifest(args: &SignArgs) -> Result<Manifest> {
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
    let output = ProcessCommand::new("git")
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

fn resolve_cli_version(args: &SignArgs) -> Result<String> {
    if let Some(version) = &args.cli_version {
        if !version.trim().is_empty() {
            return Ok(version.trim().to_string());
        }
    }
    if let Some(path) = &args.cli_path {
        let output = ProcessCommand::new(path)
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

fn maybe_sign_manifest(bytes: &[u8], args: &SignArgs) -> Result<()> {
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

    // Self-verify to catch mismatched keys early.
    let verifying_key = signing_key.verifying_key();
    verifying_key
        .verify(bytes, &signature)
        .context("self-verification failed; signing key may not match signature output")?;

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

fn load_signature(path: &Path) -> Result<Signature> {
    let raw = fs::read(path)?;
    if raw.len() == 64 {
        let bytes: [u8; 64] = raw
            .try_into()
            .map_err(|_| anyhow!("signature length mismatch for {}", path.display()))?;
        return Ok(Signature::from_bytes(&bytes));
    }

    let text = String::from_utf8(raw).with_context(|| {
        format!(
            "signature file {} must be UTF-8 or raw bytes",
            path.display()
        )
    })?;
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        bail!("signature file {} was empty", path.display());
    }
    let decoded = hex::decode(&cleaned)
        .with_context(|| format!("failed to decode hex signature at {}", path.display()))?;
    if decoded.len() != 64 {
        bail!(
            "signature {} must decode to 64 bytes, got {}",
            path.display(),
            decoded.len()
        );
    }
    let bytes: [u8; 64] = decoded
        .try_into()
        .expect("length already validated to 64 bytes");
    Ok(Signature::from_bytes(&bytes))
}

fn load_public_key(path: &Path) -> Result<VerifyingKey> {
    let raw = fs::read(path)?;
    if raw.len() == 32 {
        let bytes: [u8; 32] = raw
            .try_into()
            .map_err(|_| anyhow!("public key length mismatch for {}", path.display()))?;
        return VerifyingKey::from_bytes(&bytes)
            .map_err(|err| anyhow!("invalid public key {}: {err}", path.display()));
    }

    let text = String::from_utf8(raw)
        .with_context(|| format!("public key {} must be UTF-8 or raw bytes", path.display()))?;
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        bail!("public key file {} was empty", path.display());
    }
    let decoded = hex::decode(&cleaned)
        .with_context(|| format!("failed to decode hex public key at {}", path.display()))?;
    if decoded.len() != 32 {
        bail!(
            "public key {} must decode to 32 bytes, got {}",
            path.display(),
            decoded.len()
        );
    }
    let bytes: [u8; 32] = decoded
        .try_into()
        .expect("length already validated to 32 bytes");
    VerifyingKey::from_bytes(&bytes)
        .map_err(|err| anyhow!("invalid public key {}: {err}", path.display()))
}
