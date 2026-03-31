use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

use crate::graph::model::EngineType;

/// Project workspace information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
    pub engine: EngineType,
    pub file_count: u32,
    pub scan_time: Option<String>,
}

/// Detect game engine type from project directory
pub fn detect_engine(project_path: &Path) -> EngineType {
    // Unity: has ProjectSettings/ and Assets/ directories
    if project_path.join("ProjectSettings").is_dir() && project_path.join("Assets").is_dir() {
        return EngineType::Unity;
    }
    // Godot: has project.godot file
    if project_path.join("project.godot").is_file() {
        return EngineType::Godot;
    }
    EngineType::Unknown
}

/// Scan project directory and return file inventory
pub fn scan_project(project_path: &Path) -> Result<ProjectInfo, String> {
    if !project_path.is_dir() {
        return Err(format!("Path is not a directory: {}", project_path.display()));
    }

    let engine = detect_engine(project_path);
    let name = project_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let gitignore_dirs = parse_gitignore_dirs(project_path);

    // Count relevant files (skip hidden dirs, Library, .git, etc.)
    let file_count = WalkDir::new(project_path)
        .into_iter()
        .filter_entry(|e| !is_ignored_entry(e.file_name().to_str().unwrap_or(""), &gitignore_dirs))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count() as u32;

    let scan_time = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    Ok(ProjectInfo {
        name,
        path: project_path.to_string_lossy().to_string(),
        engine,
        file_count,
        scan_time: Some(scan_time),
    })
}

/// Directories to skip during scanning (hardcoded list)
fn is_hardcoded_ignored(name: &str) -> bool {
    matches!(
        name,
        // VCS
        ".git" | ".svn" | ".hg"
        // Unity
        | "Library" | "Temp" | "Logs" | "obj" | "UserSettings"
        | "Packages" | "ProjectSettings"
        // Godot
        | ".import" | ".godot" | "export_presets"
        | "android" | "ios" | "web"
        // Build outputs
        | "Builds" | "Build" | "bin" | "dist" | "out"
        // Package managers / caches
        | "node_modules" | "__pycache__" | "target"
    ) || name.starts_with('.')
}

/// Check if a directory entry should be ignored, combining hardcoded list and .gitignore
pub fn is_ignored_entry(name: &str, gitignore_dirs: &HashSet<String>) -> bool {
    is_hardcoded_ignored(name) || gitignore_dirs.contains(name)
}

/// Parse .gitignore at project root and extract directory names to ignore
pub fn parse_gitignore_dirs(project_path: &Path) -> HashSet<String> {
    let mut dirs = HashSet::new();
    let gitignore_path = project_path.join(".gitignore");
    if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Extract directory name from patterns like "dirname/", "dirname", "/dirname/"
            let pattern = trimmed.trim_start_matches('/');
            let pattern = pattern.trim_end_matches('/');
            // Only treat simple single-segment names as directory excludes
            // (skip glob patterns like *.log, complex paths like foo/bar)
            if !pattern.is_empty()
                && !pattern.contains('*')
                && !pattern.contains('?')
                && !pattern.contains('/')
                && !pattern.contains('\\')
            {
                dirs.insert(pattern.to_string());
            }
        }
    }
    dirs
}

/// List files in the project relevant to analysis
pub fn list_project_files(project_path: &Path, engine: &EngineType) -> Vec<String> {
    let base = project_path.to_path_buf();
    let scan_root = match engine {
        EngineType::Unity => project_path.join("Assets"),
        _ => project_path.to_path_buf(),
    };

    if !scan_root.is_dir() {
        return vec![];
    }

    let gitignore_dirs = parse_gitignore_dirs(project_path);

    WalkDir::new(&scan_root)
        .into_iter()
        .filter_entry(|e| !is_ignored_entry(e.file_name().to_str().unwrap_or(""), &gitignore_dirs))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_relevant_file(e.path(), engine))
        .filter_map(|e| {
            e.path()
                .strip_prefix(&base)
                .ok()
                .map(|p| p.to_string_lossy().to_string().replace('\\', "/"))
        })
        .collect()
}

/// Check if a file is relevant for analysis based on engine type
fn is_relevant_file(path: &Path, engine: &EngineType) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match engine {
        EngineType::Unity => matches!(
            ext.as_str(),
            "cs" | "unity" | "prefab" | "mat" | "asset"
                | "shader" | "cginc"
                | "png" | "jpg" | "jpeg" | "tga" | "psd" | "tif"
                | "wav" | "mp3" | "ogg" | "aiff"
                | "fbx" | "obj" | "blend"
                | "anim" | "controller" | "overrideController"
                | "renderTexture" | "cubemap" | "flare"
                | "fontsettings" | "guiskin" | "mixer"
                | "physicMaterial" | "physicsMaterial2D"
        ),
        EngineType::Godot => matches!(
            ext.as_str(),
            "gd" | "cs" | "tscn" | "tres" | "gdshader"
                | "png" | "jpg" | "jpeg" | "svg"
                | "wav" | "mp3" | "ogg"
                | "glb" | "gltf" | "obj"
                | "gdnlib" | "gdns"
                | "import" | "cfg"
        ),
        EngineType::Unknown => false,
    }
}
