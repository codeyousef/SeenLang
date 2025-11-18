use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{BufReader, Read, Write},
    path::Path,
};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
pub struct ManifestToml {
    pub project: ProjectSection,
}

#[derive(Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub version: String,
    pub modules: Vec<String>,
}

#[derive(Serialize)]
pub struct AbiSnapshot {
    pub schema_version: u32,
    pub package: String,
    pub version: String,
    pub timestamp: String,
    pub modules: Vec<SnapshotModule>,
}

#[derive(Serialize)]
pub struct SnapshotModule {
    pub path: String,
    pub sha256: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub modules: Vec<LockModule>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockModule {
    pub path: String,
    pub hash: String,
}

pub fn load_manifest(path: &Path) -> Result<ManifestToml> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    let manifest: ManifestToml = toml::from_str(&contents)
        .with_context(|| format!("invalid manifest {}", path.display()))?;
    if manifest.project.modules.is_empty() {
        return Err(anyhow!("manifest {} defines no modules", path.display()));
    }
    Ok(manifest)
}

pub fn hash_modules(modules: &[String], base: &Path) -> Result<Vec<(String, String)>> {
    let mut results = Vec::with_capacity(modules.len());
    for module in modules {
        let sha = hash_file(&base.join(module))
            .with_context(|| format!("failed to hash module {}", module))?;
        results.push((module.clone(), sha));
    }
    Ok(results)
}

pub fn hash_file(path: &Path) -> Result<String> {
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

pub fn write_snapshot(path: &Path, snapshot: &AbiSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data =
        serde_json::to_vec_pretty(snapshot).context("failed to serialize ABI snapshot JSON")?;
    fs::write(path, data).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_lock(path: &Path) -> Result<LockFile> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let lock: LockFile = toml::from_str(&contents)
        .with_context(|| format!("invalid lock file {}", path.display()))?;
    Ok(lock)
}

pub fn update_lock(
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

    let mut map: BTreeMap<String, LockModule> = lock
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
        .filter_map(|path| map.get(path).cloned())
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

pub fn build_snapshot(manifest: &ManifestToml, module_hashes: &[(String, String)]) -> AbiSnapshot {
    AbiSnapshot {
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
    }
}

pub fn verify_against_lock(
    _manifest: &ManifestToml,
    lock: &LockFile,
    module_hashes: &[(String, String)],
) -> Result<()> {
    let expected: HashMap<_, _> = lock
        .modules
        .iter()
        .map(|module| (module.path.clone(), module.hash.clone()))
        .collect();
    for (path, actual) in module_hashes {
        match expected.get(path) {
            Some(expected_hash) if expected_hash == actual => continue,
            Some(expected_hash) => {
                return Err(anyhow!(
                    "hash mismatch for {}: expected {}, got {}",
                    path,
                    expected_hash,
                    actual
                ))
            }
            None => return Err(anyhow!("module {} missing from lock file", path)),
        }
    }
    if expected.len() != module_hashes.len() {
        return Err(anyhow!(
            "lock file contains modules not present in manifest (lock modules: {}, manifest modules: {})",
            expected.len(),
            module_hashes.len()
        ));
    }
    Ok(())
}
