use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use flate2::read::GzDecoder;
use tar::Archive;
use crate::error::{KainError, KainResult};

const REGISTRY_URL: &str = "https://greeble.co/KAIN/index.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageManifest {
    pub package: PackageInfo,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_entry")]
    pub entry: PathBuf,
    #[serde(default = "default_output")]
    pub output: PathBuf,
    #[serde(default)]
    pub targets: Vec<String>,
}

fn default_entry() -> PathBuf { PathBuf::from("src/main.kn") }
fn default_output() -> PathBuf { PathBuf::from("dist") }

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            entry: default_entry(),
            output: default_output(),
            targets: vec!["wasm".to_string()],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

// Registry Structures
#[derive(Debug, Deserialize)]
struct RegistryIndex {
    packages: HashMap<String, String>, // name -> meta.json path
}

#[derive(Debug, Deserialize)]
struct PackageMeta {
    versions: HashMap<String, PackageVersion>,
}

#[derive(Debug, Deserialize)]
struct PackageVersion {
    url: String,
    checksum: String,
}

impl PackageManifest {
    pub fn default(name: &str) -> Self {
        Self {
            package: PackageInfo {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                authors: vec![],
                description: None,
            },
            build: BuildConfig::default(),
            dependencies: HashMap::new(),
        }
    }
}

pub fn init_project(path: &PathBuf, name: Option<String>) -> KainResult<()> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| KainError::Io(e))?;
    }

    let name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("my_project")
            .to_string()
    });

    // Create KAIN.toml
    let manifest = PackageManifest::default(&name);
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| KainError::runtime(format!("Failed to serialize manifest: {}", e)))?;
    
    fs::write(path.join("KAIN.toml"), toml).map_err(|e| KainError::Io(e))?;

    // Create src directory
    let src_dir = path.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| KainError::Io(e))?;

    // Create main.kn
    let main_src = format!(r#"
# {} - Main Entry Point

fn main():
    println("Hello, KAIN World!")
"#, name);
    
    fs::write(src_dir.join("main.kn"), main_src.trim()).map_err(|e| KainError::Io(e))?;

    // Create .gitignore
    fs::write(path.join(".gitignore"), "target/\ndeps/\n").map_err(|e| KainError::Io(e))?;

    println!(" Initialized new KAIN project: {}", name);
    Ok(())
}

pub fn load_manifest(path: &PathBuf) -> KainResult<PackageManifest> {
    let manifest_path = if path.ends_with("KAIN.toml") {
        path.clone()
    } else {
        path.join("KAIN.toml")
    };

    if !manifest_path.exists() {
        return Err(KainError::runtime(format!("Manifest not found at {}", manifest_path.display())));
    }

    let content = fs::read_to_string(&manifest_path).map_err(|e| KainError::Io(e))?;
    let manifest: PackageManifest = toml::from_str(&content)
        .map_err(|e| KainError::runtime(format!("Failed to parse KAIN.toml: {}", e)))?;

    Ok(manifest)
}

pub fn add_dependency(package_name: &str, version: Option<String>) -> KainResult<()> {
    println!(" Fetching registry index...");
    let index: RegistryIndex = reqwest::blocking::get(REGISTRY_URL)
        .map_err(|e| KainError::runtime(format!("Failed to fetch registry: {}", e)))?
        .json()
        .map_err(|e| KainError::runtime(format!("Failed to parse registry index: {}", e)))?;

    let meta_path = index.packages.get(package_name)
        .ok_or_else(|| KainError::runtime(format!("Package '{}' not found in registry.", package_name)))?;

    let meta_url = format!("https://greeble.co/KAIN/{}", meta_path);
    println!(" Fetching metadata for {}...", package_name);
    
    let meta: PackageMeta = reqwest::blocking::get(&meta_url)
        .map_err(|e| KainError::runtime(format!("Failed to fetch package metadata: {}", e)))?
        .json()
        .map_err(|e| KainError::runtime(format!("Failed to parse package metadata: {}", e)))?;

    // Determine version
    let version_to_install = version.unwrap_or_else(|| {
        // Pick latest (naive)
        meta.versions.keys().max().unwrap().clone()
    });

    let pkg_ver = meta.versions.get(&version_to_install)
        .ok_or_else(|| KainError::runtime(format!("Version {} not found for package {}", version_to_install, package_name)))?;

    println!(" Resolving {} v{}...", package_name, version_to_install);

    // Update KAIN.toml
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut manifest = load_manifest(&cwd)?;
    
    manifest.dependencies.insert(package_name.to_string(), version_to_install.clone());
    
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| KainError::runtime(format!("Failed to serialize manifest: {}", e)))?;
    
    fs::write(cwd.join("KAIN.toml"), toml).map_err(|e| KainError::Io(e))?;

    println!(" Added {} v{} to KAIN.toml", package_name, version_to_install);

    // Install it
    install_package(package_name, &version_to_install, &pkg_ver.url)
}

pub fn install_all() -> KainResult<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let manifest = load_manifest(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!(" No dependencies to install.");
        return Ok(());
    }

    println!(" Fetching registry index...");
    let index: RegistryIndex = reqwest::blocking::get(REGISTRY_URL)
        .map_err(|e| KainError::runtime(format!("Failed to fetch registry: {}", e)))?
        .json()
        .map_err(|e| KainError::runtime(format!("Failed to parse registry index: {}", e)))?;

    for (name, version) in manifest.dependencies {
        // Resolve URL (duplicate logic for now, proper solver later)
        if let Some(meta_path) = index.packages.get(&name) {
             let meta_url = format!("https://greeble.co/KAIN/{}", meta_path);
             let meta: PackageMeta = reqwest::blocking::get(&meta_url)
                 .map_err(|_| KainError::runtime(format!("Failed to fetch meta for {}", name)))?
                 .json()
                 .map_err(|_| KainError::runtime(format!("Failed to parse meta for {}", name)))?;
             
             if let Some(v) = meta.versions.get(&version) {
                 install_package(&name, &version, &v.url)?;
             } else {
                 eprintln!(" Version {} not found for {}", version, name);
             }
        } else {
            eprintln!(" Package {} not found in registry", name);
        }
    }
    
    Ok(())
}

fn install_package(name: &str, version: &str, url: &str) -> KainResult<()> {
    let deps_dir = PathBuf::from("deps");
    if !deps_dir.exists() {
        fs::create_dir_all(&deps_dir).map_err(|e| KainError::Io(e))?;
    }

    let target_dir = deps_dir.join(name);
    if target_dir.exists() {
        println!(" {} v{} is already installed.", name, version);
        return Ok(());
    }

    println!(" Downloading {} from {}...", name, url);
    let response = reqwest::blocking::get(url)
        .map_err(|e| KainError::runtime(format!("Download failed: {}", e)))?;
    
    let content = response.bytes()
        .map_err(|e| KainError::runtime(format!("Failed to read bytes: {}", e)))?;

    println!(" Installing {}...", name);
    
    let tar = GzDecoder::new(std::io::Cursor::new(&content));
    let mut archive = Archive::new(tar);
    
    // Unpack to target directory
    archive.unpack(&target_dir).map_err(|e| KainError::Io(e))?;

    // Verify lib.kn exists (optional safety check)
    if !target_dir.join("lib.kn").exists() {
        // If the package was packed with a root folder (e.g. package-1.0.0/), we might need to handle stripping
        println!(" Warning: installed package {} might be nested.", name);
    }

    println!(" Installed {} v{}", name, version);
    Ok(())
}

/// Build all targets specified in KAIN.toml
pub fn build_project(target_overrides: Option<Vec<String>>) -> KainResult<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let manifest = load_manifest(&cwd)?;
    
    // Use overrides or manifest targets
    let targets = target_overrides.unwrap_or_else(|| manifest.build.targets.clone());
    
    if targets.is_empty() {
        println!(" No targets specified in KAIN.toml [build.targets]");
        println!(" Defaulting to wasm");
        return build_targets(&manifest, &cwd, &["wasm".to_string()]);
    }
    
    build_targets(&manifest, &cwd, &targets)
}

fn build_targets(manifest: &PackageManifest, cwd: &PathBuf, targets: &[String]) -> KainResult<()> {
    use crate::{compile, CompileTarget};
    
    // Ensure output directory exists
    let output_dir = cwd.join(&manifest.build.output);
    fs::create_dir_all(&output_dir).map_err(|e| KainError::Io(e))?;
    
    // Read source file
    let entry_path = cwd.join(&manifest.build.entry);
    if !entry_path.exists() {
        return Err(KainError::runtime(format!(
            "Entry file not found: {}", entry_path.display()
        )));
    }
    
    let source = fs::read_to_string(&entry_path).map_err(|e| KainError::Io(e))?;
    let file_stem = entry_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    println!(" Building {} v{}", manifest.package.name, manifest.package.version);
    println!(" Entry: {}", manifest.build.entry.display());
    println!(" Output: {}/", manifest.build.output.display());
    println!();
    
    for target_str in targets {
        let target = parse_target(target_str)?;
        let ext = target_extension(target);
        let out_path = output_dir.join(file_stem).with_extension(ext);
        
        match compile(&source, target) {
            Ok(output) => {
                fs::write(&out_path, &output).map_err(|e| KainError::Io(e))?;
                println!(" [{}] -> {} ({} bytes)", target_str, out_path.display(), output.len());
            }
            Err(e) => {
                eprintln!(" [{}] FAILED: {}", target_str, e);
            }
        }
    }
    
    println!();
    println!(" Build complete!");
    Ok(())
}

fn parse_target(s: &str) -> KainResult<crate::CompileTarget> {
    use crate::CompileTarget;
    match s.to_lowercase().as_str() {
        "wasm" | "w" => Ok(CompileTarget::Wasm),
        "llvm" | "native" | "n" => Ok(CompileTarget::Llvm),
        "spirv" | "gpu" | "s" => Ok(CompileTarget::SpirV),
        "hlsl" | "h" => Ok(CompileTarget::Hlsl),
        "usf" | "ue5" => Ok(CompileTarget::Usf),
        "js" | "javascript" => Ok(CompileTarget::Js),
        "rust" | "rs" => Ok(CompileTarget::Rust),
        "hybrid" => Ok(CompileTarget::Hybrid),
        _ => Err(KainError::runtime(format!("Unknown target: {}", s)))
    }
}

fn target_extension(target: crate::CompileTarget) -> &'static str {
    use crate::CompileTarget;
    match target {
        CompileTarget::Wasm => "wasm",
        CompileTarget::Llvm => "ll",
        CompileTarget::SpirV => "spv",
        CompileTarget::Hlsl => "hlsl",
        CompileTarget::Usf => "usf",
        CompileTarget::Js => "js",
        CompileTarget::Rust => "rs",
        CompileTarget::Hybrid => "js",
        CompileTarget::Interpret | CompileTarget::Test => "txt",
    }
}

