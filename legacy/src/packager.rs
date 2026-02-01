use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::error::{KoreError, KoreResult};

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

    // Create god.toml
    let manifest = PackageManifest::default(&name);
    let toml = toml::to_string_pretty(&manifest)
        .map_err(|e| KoreError::runtime(format!("Failed to serialize manifest: {}", e)))?;
    
    fs::write(path.join("god.toml"), toml).map_err(|e| KoreError::Io(e))?;

    // Create src directory
    let src_dir = path.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| KoreError::Io(e))?;

    // Create main.kr
    let main_god = format!(r#"
# {} - Main Entry Point

fn main():
    print("Hello, KORE World!")
"#, name);
    
    fs::write(src_dir.join("main.kr"), main_god.trim()).map_err(|e| KoreError::Io(e))?;

    // Create .gitignore
    fs::write(path.join(".gitignore"), "target/\n").map_err(|e| KoreError::Io(e))?;

    println!(" Initialized new KORE project: {}", name);
    Ok(())
}

pub fn load_manifest(path: &PathBuf) -> KoreResult<PackageManifest> {
    let manifest_path = if path.ends_with("god.toml") {
        path.clone()
    } else {
        path.join("god.toml")
    };

    if !manifest_path.exists() {
        return Err(KoreError::runtime(format!("Manifest not found at {}", manifest_path.display())));
    }

    let content = fs::read_to_string(&manifest_path).map_err(|e| KoreError::Io(e))?;
    let manifest: PackageManifest = toml::from_str(&content)
        .map_err(|e| KoreError::runtime(format!("Failed to parse god.toml: {}", e)))?;

    Ok(manifest)
}

