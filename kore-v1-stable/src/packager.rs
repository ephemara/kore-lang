use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use flate2::read::GzDecoder;
use tar::Archive;
use crate::error::{KoreError, KoreResult};

const REGISTRY_URL: &str = "https://greeble.co/kore/index.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageManifest {
    pub package: PackageInfo,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
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
            dependencies: HashMap::new(),
        }
    }
}

pub fn init_project(path: &PathBuf, name: Option<String>) -> KoreResult<()> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| KoreError::Io(e))?;
    }

    let name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("my_project")
            .to_string()
    });

    // Create kore.toml
    let manifest = PackageManifest::default(&name);
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| KoreError::runtime(format!("Failed to serialize manifest: {}", e)))?;
    
    fs::write(path.join("kore.toml"), toml).map_err(|e| KoreError::Io(e))?;

    // Create src directory
    let src_dir = path.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| KoreError::Io(e))?;

    // Create main.kr
    let main_src = format!(r#"
# {} - Main Entry Point

fn main():
    println("Hello, KORE World!")
"#, name);
    
    fs::write(src_dir.join("main.kr"), main_src.trim()).map_err(|e| KoreError::Io(e))?;

    // Create .gitignore
    fs::write(path.join(".gitignore"), "target/\ndeps/\n").map_err(|e| KoreError::Io(e))?;

    println!(" Initialized new KORE project: {}", name);
    Ok(())
}

pub fn load_manifest(path: &PathBuf) -> KoreResult<PackageManifest> {
    let manifest_path = if path.ends_with("kore.toml") {
        path.clone()
    } else {
        path.join("kore.toml")
    };

    if !manifest_path.exists() {
        return Err(KoreError::runtime(format!("Manifest not found at {}", manifest_path.display())));
    }

    let content = fs::read_to_string(&manifest_path).map_err(|e| KoreError::Io(e))?;
    let manifest: PackageManifest = toml::from_str(&content)
        .map_err(|e| KoreError::runtime(format!("Failed to parse kore.toml: {}", e)))?;

    Ok(manifest)
}

pub fn add_dependency(package_name: &str, version: Option<String>) -> KoreResult<()> {
    println!(" Fetching registry index...");
    let index: RegistryIndex = reqwest::blocking::get(REGISTRY_URL)
        .map_err(|e| KoreError::runtime(format!("Failed to fetch registry: {}", e)))?
        .json()
        .map_err(|e| KoreError::runtime(format!("Failed to parse registry index: {}", e)))?;

    let meta_path = index.packages.get(package_name)
        .ok_or_else(|| KoreError::runtime(format!("Package '{}' not found in registry.", package_name)))?;

    let meta_url = format!("https://greeble.co/kore/{}", meta_path);
    println!(" Fetching metadata for {}...", package_name);
    
    let meta: PackageMeta = reqwest::blocking::get(&meta_url)
        .map_err(|e| KoreError::runtime(format!("Failed to fetch package metadata: {}", e)))?
        .json()
        .map_err(|e| KoreError::runtime(format!("Failed to parse package metadata: {}", e)))?;

    // Determine version
    let version_to_install = version.unwrap_or_else(|| {
        // Pick latest (naive)
        meta.versions.keys().max().unwrap().clone()
    });

    let pkg_ver = meta.versions.get(&version_to_install)
        .ok_or_else(|| KoreError::runtime(format!("Version {} not found for package {}", version_to_install, package_name)))?;

    println!(" Resolving {} v{}...", package_name, version_to_install);

    // Update kore.toml
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut manifest = load_manifest(&cwd)?;
    
    manifest.dependencies.insert(package_name.to_string(), version_to_install.clone());
    
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| KoreError::runtime(format!("Failed to serialize manifest: {}", e)))?;
    
    fs::write(cwd.join("kore.toml"), toml).map_err(|e| KoreError::Io(e))?;

    println!(" Added {} v{} to kore.toml", package_name, version_to_install);

    // Install it
    install_package(package_name, &version_to_install, &pkg_ver.url)
}

pub fn install_all() -> KoreResult<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let manifest = load_manifest(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!(" No dependencies to install.");
        return Ok(());
    }

    println!(" Fetching registry index...");
    let index: RegistryIndex = reqwest::blocking::get(REGISTRY_URL)
        .map_err(|e| KoreError::runtime(format!("Failed to fetch registry: {}", e)))?
        .json()
        .map_err(|e| KoreError::runtime(format!("Failed to parse registry index: {}", e)))?;

    for (name, version) in manifest.dependencies {
        // Resolve URL (duplicate logic for now, proper solver later)
        if let Some(meta_path) = index.packages.get(&name) {
             let meta_url = format!("https://greeble.co/kore/{}", meta_path);
             let meta: PackageMeta = reqwest::blocking::get(&meta_url)
                 .map_err(|_| KoreError::runtime(format!("Failed to fetch meta for {}", name)))?
                 .json()
                 .map_err(|_| KoreError::runtime(format!("Failed to parse meta for {}", name)))?;
             
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

fn install_package(name: &str, version: &str, url: &str) -> KoreResult<()> {
    let deps_dir = PathBuf::from("deps");
    if !deps_dir.exists() {
        fs::create_dir_all(&deps_dir).map_err(|e| KoreError::Io(e))?;
    }

    let target_dir = deps_dir.join(name);
    if target_dir.exists() {
        println!(" {} v{} is already installed.", name, version);
        return Ok(());
    }

    println!(" Downloading {} from {}...", name, url);
    let response = reqwest::blocking::get(url)
        .map_err(|e| KoreError::runtime(format!("Download failed: {}", e)))?;
    
    let content = response.bytes()
        .map_err(|e| KoreError::runtime(format!("Failed to read bytes: {}", e)))?;

    println!(" Installing {}...", name);
    
    let tar = GzDecoder::new(std::io::Cursor::new(&content));
    let mut archive = Archive::new(tar);
    
    // Unpack to target directory
    archive.unpack(&target_dir).map_err(|e| KoreError::Io(e))?;

    // Verify lib.kr exists (optional safety check)
    if !target_dir.join("lib.kr").exists() {
        // If the package was packed with a root folder (e.g. package-1.0.0/), we might need to handle stripping
        println!(" Warning: installed package {} might be nested.", name);
    }

    println!(" Installed {} v{}", name, version);
    Ok(())
}
