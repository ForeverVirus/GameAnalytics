use regex::Regex;
use sha2::{Sha256, Digest};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::graph::model::*;
use crate::graph::store::GraphStore;
use crate::workspace;

/// Build GUID → relative file path mapping from Unity .meta files
pub fn build_unity_guid_map(project_root: &Path) -> HashMap<String, String> {
    let assets_dir = project_root.join("Assets");
    let mut map = HashMap::new();
    let guid_re = Regex::new(r"guid:\s*([0-9a-fA-F]{32})").unwrap();
    let gitignore_dirs = workspace::parse_gitignore_dirs(project_root);

    for entry in WalkDir::new(&assets_dir)
        .into_iter()
        .filter_entry(|e| {
            !workspace::is_ignored_entry(e.file_name().to_str().unwrap_or(""), &gitignore_dirs)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("meta") {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            if let Some(caps) = guid_re.captures(&content) {
                let guid = caps[1].to_lowercase();
                let actual_file = path.with_extension("");
                if let Ok(rel) = actual_file.strip_prefix(project_root) {
                    map.insert(guid, rel.to_string_lossy().to_string().replace('\\', "/"));
                }
            }
        }
    }

    map
}

/// Analyze Unity YAML files (.prefab, .unity, .mat, etc.) for GUID references
/// Returns DependsOn edges between files
pub fn analyze_unity_references(
    project_root: &Path,
    files: &[String],
    guid_map: &HashMap<String, String>,
    node_ids: &HashSet<String>,
) -> Vec<GraphEdge> {
    let guid_ref_re = Regex::new(r"guid:\s*([0-9a-fA-F]{32})").unwrap();
    let yaml_extensions = [
        "prefab",
        "unity",
        "mat",
        "asset",
        "controller",
        "overrideController",
        "anim",
    ];
    let mut edges = Vec::new();

    for file_rel in files {
        let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();
        if !yaml_extensions.contains(&ext.as_str()) {
            continue;
        }

        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut seen_guids = HashSet::new();
        for caps in guid_ref_re.captures_iter(&content) {
            let guid = caps[1].to_lowercase();
            if seen_guids.contains(&guid) {
                continue;
            }
            seen_guids.insert(guid.clone());

            if let Some(target_path) = guid_map.get(&guid) {
                if target_path == file_rel {
                    continue;
                }
                if node_ids.contains(target_path) {
                    edges.push(GraphEdge {
                        source: file_rel.clone(),
                        target: target_path.clone(),
                        edge_type: EdgeType::DependsOn,
                        reference_class: ReferenceClass::Official,
                        label: None,
                        evidence: Some(Evidence {
                            parser_type: "unity_yaml_guid".to_string(),
                            source_file: file_rel.clone(),
                            source_line: None,
                            rule: Some(format!("guid:{}", guid)),
                        }),
                    });
                }
            }
        }
    }

    edges
}

/// Build class name → file path mapping from all code files (first pass of cross-reference analysis)
pub fn build_class_map(project_root: &Path, code_files: &[String]) -> HashMap<String, String> {
    let class_re = Regex::new(r"(?:public|internal|private|protected)?\s*(?:abstract|sealed|static|partial)?\s*class\s+(\w+)").unwrap();
    let mut class_to_file: HashMap<String, String> = HashMap::new();

    for file_rel in code_files {
        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for caps in class_re.captures_iter(&content) {
            class_to_file.insert(caps[1].to_string(), file_rel.to_string());
        }
    }

    class_to_file
}

/// Extract all identifiers (word-boundary tokens) from source code, skipping 'using' lines.
/// Used for O(1) class-name lookups instead of O(classes) regex compilations per file.
fn extract_identifiers(content: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("using ") {
            continue;
        }
        let bytes = line.as_bytes();
        let mut start: Option<usize> = None;
        for i in 0..=bytes.len() {
            let is_ident =
                i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_');
            if is_ident {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(s) = start {
                ids.insert(line[s..i].to_string());
                start = None;
            }
        }
    }
    ids
}

/// Find code cross-references for a batch of files using a pre-built class map
pub fn analyze_code_references_batch(
    project_root: &Path,
    batch_files: &[String],
    class_to_file: &HashMap<String, String>,
) -> Vec<GraphEdge> {
    let inherit_re = Regex::new(r"class\s+\w+\s*(?:<[^>]*>)?\s*:\s*([A-Za-z_][\w.]*)").unwrap();
    let mut edges = Vec::new();

    for file_rel in batch_files {
        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut referenced_files = HashSet::new();

        // Check inheritance
        for caps in inherit_re.captures_iter(&content) {
            let base_class = caps[1].to_string();
            if matches!(
                base_class.as_str(),
                "MonoBehaviour"
                    | "ScriptableObject"
                    | "Editor"
                    | "EditorWindow"
                    | "Component"
                    | "Behaviour"
            ) {
                continue;
            }
            if let Some(target_file) = class_to_file.get(&base_class) {
                if *target_file != *file_rel && !referenced_files.contains(target_file) {
                    referenced_files.insert(target_file.clone());
                    edges.push(GraphEdge {
                        source: file_rel.to_string(),
                        target: target_file.clone(),
                        edge_type: EdgeType::Inherits,
                        reference_class: ReferenceClass::Official,
                        label: Some(base_class),
                        evidence: None,
                    });
                }
            }
        }

        // Extract identifiers once, then O(1) HashSet lookup per class — replaces O(classes) regex
        let identifiers = extract_identifiers(&content);

        for (class_name, target_file) in class_to_file {
            if *target_file == *file_rel || referenced_files.contains(target_file) {
                continue;
            }
            if identifiers.contains(class_name.as_str()) {
                referenced_files.insert(target_file.clone());
                edges.push(GraphEdge {
                    source: file_rel.to_string(),
                    target: target_file.clone(),
                    edge_type: EdgeType::References,
                    reference_class: ReferenceClass::Official,
                    label: Some(class_name.clone()),
                    evidence: None,
                });
            }
        }
    }

    edges
}

/// Detect hardcoded values in source code files
pub fn detect_hardcodes(project_root: &Path, files: &[String]) -> Vec<HardcodeFinding> {
    let mut findings = Vec::new();
    let path_re = Regex::new(r#""((?:Assets|Resources|res://|Packages)[^"]{3,})""#).unwrap();
    let url_re = Regex::new(r#""(https?://[^"]+)""#).unwrap();
    let color_re = Regex::new(r#""(#[0-9a-fA-F]{6,8})""#).unwrap();

    let code_extensions = ["cs", "gd"];
    let mut finding_id = 0u32;

    for file_rel in files {
        let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();
        if !code_extensions.contains(&ext.as_str()) {
            continue;
        }

        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments and attributes
            if trimmed.starts_with("//")
                || trimmed.starts_with("///")
                || trimmed.starts_with("[")
                || trimmed.starts_with("#")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*")
            {
                continue;
            }

            // Path hardcodes
            for caps in path_re.captures_iter(line) {
                finding_id += 1;
                findings.push(HardcodeFinding {
                    id: format!("hc_{}", finding_id),
                    file_path: file_rel.clone(),
                    line_number: (line_num + 1) as u32,
                    value: caps[1].to_string(),
                    code_excerpt: trimmed.to_string(),
                    category: HardcodeCategory::Path,
                    severity: Severity::High,
                });
            }

            // URL hardcodes
            for caps in url_re.captures_iter(line) {
                finding_id += 1;
                findings.push(HardcodeFinding {
                    id: format!("hc_{}", finding_id),
                    file_path: file_rel.clone(),
                    line_number: (line_num + 1) as u32,
                    value: caps[1].to_string(),
                    code_excerpt: trimmed.to_string(),
                    category: HardcodeCategory::Url,
                    severity: Severity::Medium,
                });
            }

            // Color hex hardcodes
            for caps in color_re.captures_iter(line) {
                finding_id += 1;
                findings.push(HardcodeFinding {
                    id: format!("hc_{}", finding_id),
                    file_path: file_rel.clone(),
                    line_number: (line_num + 1) as u32,
                    value: caps[1].to_string(),
                    code_excerpt: trimmed.to_string(),
                    category: HardcodeCategory::Color,
                    severity: Severity::Low,
                });
            }
        }
    }

    findings
}

fn infer_asset_kind_from_path(file_rel: &str) -> Option<AssetKind> {
    let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();

    match ext.as_str() {
        "unity" | "tscn" => Some(AssetKind::Scene),
        "prefab" | "tres" => Some(AssetKind::Prefab),
        "mat" => Some(AssetKind::Material),
        "shader" | "cginc" | "gdshader" => Some(AssetKind::Shader),
        "png" | "jpg" | "jpeg" | "tga" | "psd" | "tif" | "svg" => Some(AssetKind::Texture),
        "wav" | "mp3" | "ogg" | "aiff" => Some(AssetKind::Audio),
        "anim" | "controller" | "overridecontroller" => Some(AssetKind::Animation),
        "fontsettings" | "ttf" | "otf" | "fnt" => Some(AssetKind::Data),
        "asset" | "rendertexture" | "cubemap" | "txt" | "json" | "bytes" | "xml" | "csv" => {
            Some(AssetKind::Data)
        }
        _ => None,
    }
}

fn matches_unity_resource_type(file_rel: &str, unity_type: Option<&str>) -> bool {
    let Some(type_name) = unity_type else {
        return true;
    };

    let normalized = type_name
        .rsplit('.')
        .next()
        .unwrap_or(type_name)
        .to_lowercase();
    let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();

    match normalized.as_str() {
        "texture" | "texture2d" | "sprite" | "cubemap" | "rendertexture" => {
            matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "tga" | "psd" | "tif" | "svg"
            )
        }
        "material" => ext == "mat",
        "audioclip" => matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "aiff"),
        "gameobject" => ext == "prefab",
        "animationclip" => ext == "anim",
        "runtimeanimatorcontroller" | "animatorcontroller" => {
            matches!(ext.as_str(), "controller" | "overridecontroller")
        }
        "shader" | "computeshader" => matches!(ext.as_str(), "shader" | "cginc"),
        "textasset" => matches!(ext.as_str(), "txt" | "json" | "bytes" | "xml" | "csv"),
        "scriptableobject" => ext == "asset",
        "sceneasset" => ext == "unity",
        "font" | "fontasset" | "tmp_fontasset" => {
            matches!(ext.as_str(), "fontsettings" | "ttf" | "otf" | "fnt")
        }
        "texture[]" | "texture2d[]" | "sprite[]" | "gameobject[]" | "material[]"
        | "audioclip[]" => {
            matches_unity_resource_type(file_rel, Some(normalized.trim_end_matches("[]")))
        }
        _ => true,
    }
}

fn build_asset_inventory(project_root: &Path) -> Vec<String> {
    let assets_dir = project_root.join("Assets");
    if !assets_dir.is_dir() {
        return Vec::new();
    }

    let gitignore_dirs = workspace::parse_gitignore_dirs(project_root);
    let mut files = Vec::new();

    for entry in WalkDir::new(&assets_dir)
        .into_iter()
        .filter_entry(|e| {
            !workspace::is_ignored_entry(e.file_name().to_str().unwrap_or(""), &gitignore_dirs)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = match entry.path().strip_prefix(project_root) {
            Ok(rel) => rel.to_string_lossy().to_string().replace('\\', "/"),
            Err(_) => continue,
        };
        if infer_asset_kind_from_path(&rel).is_some() {
            files.push(rel);
        }
    }

    files.sort();
    files
}

fn strip_extension(path: &str) -> &str {
    match path.rfind('.') {
        Some(idx) => &path[..idx],
        None => path,
    }
}

fn resource_relative_no_ext(asset_rel: &str) -> Option<String> {
    let normalized = asset_rel.replace('\\', "/");
    let after_resources = normalized.strip_prefix("Assets/Resources/")?;
    Some(strip_extension(after_resources).to_string())
}

fn normalize_query_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

fn resolve_inventory_candidates(
    asset_inventory: &[String],
    query: &str,
    unity_type: Option<&str>,
    load_all: bool,
    prefer_resources: bool,
) -> Vec<(String, Option<AssetKind>, String)> {
    let normalized_query = normalize_query_path(query);
    let query_no_ext = strip_extension(&normalized_query).to_lowercase();
    let query_with_assets = if normalized_query.starts_with("Assets/") {
        normalized_query.clone()
    } else {
        format!("Assets/{}", normalized_query)
    };
    let query_segments: Vec<&str> = normalized_query
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let query_basename = query_segments.last().copied().unwrap_or("").to_lowercase();

    let mut results: Vec<(String, Option<AssetKind>, String)> = Vec::new();

    for asset_rel in asset_inventory {
        if !matches_unity_resource_type(asset_rel, unity_type) {
            continue;
        }

        let normalized_asset = asset_rel.replace('\\', "/");
        let asset_no_ext = strip_extension(&normalized_asset).to_string();
        let asset_basename = std::path::Path::new(&normalized_asset)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let mut matched_reason: Option<String> = None;

        if prefer_resources {
            if let Some(resource_rel) = resource_relative_no_ext(&normalized_asset) {
                let resource_rel_lower = resource_rel.to_lowercase();
                if load_all {
                    if normalized_query.is_empty()
                        || resource_rel_lower == query_no_ext
                        || resource_rel_lower.starts_with(&(query_no_ext.clone() + "/"))
                    {
                        matched_reason = Some("Resources 目录展开".to_string());
                    }
                } else if resource_rel_lower == query_no_ext {
                    matched_reason = Some("Resources 精确匹配".to_string());
                }
            }
        }

        if matched_reason.is_none() && !normalized_query.is_empty() {
            let asset_lower = normalized_asset.to_lowercase();
            let asset_no_ext_lower = asset_no_ext.to_lowercase();
            let query_assets_lower = query_with_assets.to_lowercase();

            if load_all {
                let asset_dir = std::path::Path::new(&normalized_asset)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string().replace('\\', "/"))
                    .unwrap_or_default();
                let asset_dir_no_assets = asset_dir
                    .strip_prefix("Assets/")
                    .unwrap_or(&asset_dir)
                    .to_string();
                let asset_dir_lower = asset_dir.to_lowercase();
                let asset_dir_no_assets_lower = asset_dir_no_assets.to_lowercase();

                if asset_dir_lower == query_assets_lower
                    || asset_dir_lower
                        .ends_with(&(String::from("/") + &normalized_query.to_lowercase()))
                    || asset_dir_no_assets_lower == query_no_ext
                    || asset_dir_no_assets_lower.ends_with(&(String::from("/") + &query_no_ext))
                {
                    matched_reason = Some("目录批量加载推断".to_string());
                }
            } else if asset_lower == query_assets_lower.to_lowercase() {
                matched_reason = Some("资产路径精确匹配".to_string());
            } else if asset_lower.ends_with(&(String::from("/") + &normalized_query.to_lowercase()))
            {
                matched_reason = Some("资产路径后缀匹配".to_string());
            } else if asset_no_ext_lower == query_assets_lower.to_lowercase()
                || asset_no_ext_lower.ends_with(&(String::from("/") + &query_no_ext))
            {
                matched_reason = Some("去扩展名路径匹配".to_string());
            } else if !query_basename.is_empty() && asset_basename == query_basename {
                matched_reason = Some("文件名启发式匹配".to_string());
            }
        }

        if let Some(reason) = matched_reason {
            results.push((
                normalized_asset.clone(),
                infer_asset_kind_from_path(&normalized_asset),
                reason,
            ));
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results.dedup_by(|a, b| a.0 == b.0);
    results
}

fn resolve_unity_resources_candidates(
    project_root: &Path,
    resource_path: &str,
    unity_type: Option<&str>,
    load_all: bool,
) -> Vec<(String, Option<AssetKind>)> {
    let resources_root = project_root.join("Assets").join("Resources");
    if !resources_root.is_dir() {
        return Vec::new();
    }

    let normalized = resource_path
        .replace('\\', "/")
        .trim_start_matches("Resources/")
        .trim_matches('/')
        .to_string();

    let mut results = Vec::new();

    if load_all {
        let target_dir = if normalized.is_empty() {
            resources_root.clone()
        } else {
            resources_root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR))
        };

        if !target_dir.is_dir() {
            return results;
        }

        for entry in WalkDir::new(&target_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let rel = match path.strip_prefix(project_root) {
                Ok(rel) => rel.to_string_lossy().to_string().replace('\\', "/"),
                Err(_) => continue,
            };
            if infer_asset_kind_from_path(&rel).is_none()
                || !matches_unity_resource_type(&rel, unity_type)
            {
                continue;
            }
            results.push((rel.clone(), infer_asset_kind_from_path(&rel)));
        }
    } else {
        let parent = std::path::Path::new(&normalized)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let target_name = std::path::Path::new(&normalized)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if target_name.is_empty() {
            return results;
        }

        let search_dir = if parent.is_empty() {
            resources_root.clone()
        } else {
            resources_root.join(parent.replace('/', std::path::MAIN_SEPARATOR_STR))
        };

        if !search_dir.is_dir() {
            return results;
        }

        for entry in WalkDir::new(&search_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem != target_name {
                continue;
            }
            let rel = match path.strip_prefix(project_root) {
                Ok(rel) => rel.to_string_lossy().to_string().replace('\\', "/"),
                Err(_) => continue,
            };
            if infer_asset_kind_from_path(&rel).is_none()
                || !matches_unity_resource_type(&rel, unity_type)
            {
                continue;
            }
            results.push((rel.clone(), infer_asset_kind_from_path(&rel)));
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results.dedup_by(|a, b| a.0 == b.0);
    results
}

fn push_suspected_ref(
    suspected_refs: &mut Vec<SuspectedReference>,
    seen_refs: &mut HashSet<String>,
    ref_id: &mut u32,
    file_rel: &str,
    line_num: u32,
    trimmed: &str,
    method_name: &str,
    resource_path: String,
    resource_type: Option<AssetKind>,
    confidence: f32,
    ai_explanation: Option<String>,
) {
    let dedupe_key = format!(
        "{}|{}|{}|{}",
        file_rel, line_num, method_name, resource_path
    );
    if !seen_refs.insert(dedupe_key) {
        return;
    }

    *ref_id += 1;
    suspected_refs.push(SuspectedReference {
        id: format!("sr_{}", *ref_id),
        resource_path,
        resource_type,
        code_location: file_rel.to_string(),
        code_line: Some(line_num),
        code_excerpt: Some(trimmed.to_string()),
        load_method: method_name.to_string(),
        confidence,
        status: SuspectedStatus::Pending,
        ai_explanation,
    });
}

fn push_unity_resources_candidates(
    project_root: &Path,
    suspected_refs: &mut Vec<SuspectedReference>,
    seen_refs: &mut HashSet<String>,
    ref_id: &mut u32,
    file_rel: &str,
    line_num: u32,
    trimmed: &str,
    method_name: &str,
    resource_path: &str,
    unity_type: Option<&str>,
    confidence: f32,
    load_all: bool,
) {
    let candidates =
        resolve_unity_resources_candidates(project_root, resource_path, unity_type, load_all);
    let explanation = unity_type.map(|t| format!("从 {} 推断资源类型为 {}", method_name, t));

    if candidates.is_empty() {
        push_suspected_ref(
            suspected_refs,
            seen_refs,
            ref_id,
            file_rel,
            line_num,
            trimmed,
            method_name,
            resource_path.to_string(),
            None,
            confidence,
            explanation,
        );
        return;
    }

    for (candidate_path, asset_kind) in candidates {
        push_suspected_ref(
            suspected_refs,
            seen_refs,
            ref_id,
            file_rel,
            line_num,
            trimmed,
            method_name,
            candidate_path.clone(),
            asset_kind,
            confidence,
            Some(format!(
                "由 {}(\"{}\") 展开得到的候选资源{}",
                method_name,
                resource_path,
                unity_type
                    .map(|t| format!("，类型约束 {}", t))
                    .unwrap_or_default()
            )),
        );
    }
}

fn push_asset_inventory_candidates(
    asset_inventory: &[String],
    suspected_refs: &mut Vec<SuspectedReference>,
    seen_refs: &mut HashSet<String>,
    ref_id: &mut u32,
    file_rel: &str,
    line_num: u32,
    trimmed: &str,
    method_name: &str,
    resource_path: &str,
    unity_type: Option<&str>,
    confidence: f32,
    load_all: bool,
    prefer_resources: bool,
    heuristic_tag: &str,
) {
    let candidates = resolve_inventory_candidates(
        asset_inventory,
        resource_path,
        unity_type,
        load_all,
        prefer_resources,
    );

    if candidates.is_empty() {
        push_suspected_ref(
            suspected_refs,
            seen_refs,
            ref_id,
            file_rel,
            line_num,
            trimmed,
            method_name,
            resource_path.to_string(),
            None,
            confidence,
            Some(format!(
                "{}：未能静态定位到具体资源，保留原始键值",
                heuristic_tag
            )),
        );
        return;
    }

    for (candidate_path, asset_kind, reason) in candidates {
        push_suspected_ref(
            suspected_refs,
            seen_refs,
            ref_id,
            file_rel,
            line_num,
            trimmed,
            method_name,
            candidate_path,
            asset_kind,
            confidence,
            Some(format!(
                "{}：由 {}(\"{}\") 推断命中，{}{}",
                heuristic_tag,
                method_name,
                resource_path,
                reason,
                unity_type
                    .map(|t| format!("，类型约束 {}", t))
                    .unwrap_or_default()
            )),
        );
    }
}

fn line_context_at_offset(content: &str, offset: usize) -> (u32, String) {
    let safe_offset = offset.min(content.len());
    let prefix = &content[..safe_offset];
    let line_number = prefix.bytes().filter(|b| *b == b'\n').count() as u32 + 1;
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = content[safe_offset..]
        .find('\n')
        .map(|idx| safe_offset + idx)
        .unwrap_or(content.len());
    let excerpt = content[line_start..line_end].trim().to_string();
    (line_number, excerpt)
}

fn context_window_at_offset(content: &str, offset: usize, before_lines: usize) -> String {
    let safe_offset = offset.min(content.len());
    let mut start = 0usize;
    let prefix = &content[..safe_offset];
    let mut search_end = prefix.len();

    for _ in 0..before_lines {
        if let Some(pos) = prefix[..search_end].rfind('\n') {
            start = pos;
            search_end = pos;
        } else {
            start = 0;
            break;
        }
    }

    content[start..safe_offset].to_string()
}

fn is_comment_like_excerpt(excerpt: &str) -> bool {
    let trimmed = excerpt.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*")
}

fn compact_expression(expr: &str) -> String {
    expr.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_static_string_value(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    if trimmed.starts_with('$') {
        return None;
    }
    for prefix in ["@\"", "\""] {
        if trimmed.starts_with(prefix)
            && trimmed.ends_with('"')
            && trimmed.len() >= prefix.len() + 1
        {
            let inner = &trimmed[prefix.len()..trimmed.len() - 1];
            return Some(inner.to_string());
        }
    }
    None
}

fn resolve_expression_to_static_string(expr: &str, content_before_offset: &str) -> Option<String> {
    if let Some(value) = extract_static_string_value(expr) {
        return Some(value);
    }

    let compact = compact_expression(expr);
    let identifier_re = Regex::new(r#"^[A-Za-z_][A-Za-z0-9_]*$"#).ok()?;
    if !identifier_re.is_match(&compact) {
        return None;
    }

    let assigned = lookup_variable_assignment(&compact, content_before_offset)?;
    extract_static_string_value(&assigned)
}

fn extract_invocation_slice(content: &str, start: usize) -> Option<String> {
    let slice = &content[start..];
    let mut in_string = false;
    let mut escape = false;
    let mut angle_depth = 0i32;
    let mut open_paren_idx = None;

    for (idx, ch) in slice.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '<' if open_paren_idx.is_none() => angle_depth += 1,
            '>' if open_paren_idx.is_none() && angle_depth > 0 => angle_depth -= 1,
            '(' if angle_depth == 0 => {
                open_paren_idx = Some(idx);
                break;
            }
            _ => {}
        }
    }

    let open_idx = open_paren_idx?;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    in_string = false;
    escape = false;

    for (idx, ch) in slice[open_idx..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' => paren_depth += 1,
            ')' => {
                paren_depth -= 1;
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
                    return Some(slice[..open_idx + idx + 1].to_string());
                }
            }
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' if brace_depth > 0 => brace_depth -= 1,
            _ => {}
        }
    }

    None
}

fn extract_first_argument_expression(invocation: &str) -> Option<String> {
    let open_idx = invocation.find('(')?;
    let args = &invocation[open_idx + 1..];
    let mut in_string = false;
    let mut escape = false;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut angle_depth = 0i32;

    for (idx, ch) in args.char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' => paren_depth += 1,
            ')' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0 =>
            {
                return Some(args[..idx].trim().to_string());
            }
            ')' if paren_depth > 0 => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' if bracket_depth > 0 => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' if brace_depth > 0 => brace_depth -= 1,
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            ',' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0 =>
            {
                return Some(args[..idx].trim().to_string());
            }
            _ => {}
        }
    }

    None
}

fn extract_unity_type_from_invocation(invocation: &str) -> Option<String> {
    let generic_re =
        Regex::new(r#"\.\s*[A-Za-z_]\w*\s*<\s*([A-Za-z_][\w\.\[\],<>]*)\s*>\s*\("#).ok()?;
    if let Some(caps) = generic_re.captures(invocation) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    let typeof_re = Regex::new(r#"typeof\s*\(\s*([A-Za-z_][\w\.\[\],<>]*)\s*\)"#).ok()?;
    typeof_re
        .captures(invocation)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

fn asset_kind_from_unity_type(unity_type: Option<&str>) -> Option<AssetKind> {
    let type_name = unity_type?;
    let normalized = type_name
        .rsplit('.')
        .next()
        .unwrap_or(type_name)
        .trim_end_matches("[]")
        .to_lowercase();

    match normalized.as_str() {
        "texture" | "texture2d" | "sprite" | "cubemap" | "rendertexture" => {
            Some(AssetKind::Texture)
        }
        "material" => Some(AssetKind::Material),
        "audioclip" => Some(AssetKind::Audio),
        "gameobject" => Some(AssetKind::Prefab),
        "animationclip" | "runtimeanimatorcontroller" | "animatorcontroller" => {
            Some(AssetKind::Animation)
        }
        "shader" | "computeshader" => Some(AssetKind::Shader),
        "sceneasset" => Some(AssetKind::Scene),
        "scriptableobject" | "textasset" | "font" | "fontasset" | "tmp_fontasset" => {
            Some(AssetKind::Data)
        }
        _ => None,
    }
}

fn lookup_variable_assignment(var_name: &str, content_before_offset: &str) -> Option<String> {
    let escaped = regex::escape(var_name);
    let assign_re = Regex::new(&format!(r#"\b{}\b\s*=\s*(.+?);?\s*$"#, escaped)).ok()?;
    let field_re = Regex::new(&format!(
        r#"(?:public|private|protected|internal|static|readonly|const|\s)+[A-Za-z_][\w<>\[\]\.,]*\s+{}\s*=\s*(.+?);?\s*$"#,
        escaped
    )).ok()?;

    for line in content_before_offset.lines().rev() {
        let trimmed = line.trim();
        if is_comment_like_excerpt(trimmed) {
            continue;
        }
        if let Some(caps) = field_re.captures(trimmed) {
            return caps.get(1).map(|m| m.as_str().trim().to_string());
        }
        if let Some(caps) = assign_re.captures(trimmed) {
            return caps.get(1).map(|m| m.as_str().trim().to_string());
        }
    }

    None
}

fn classify_dynamic_resource_expression(
    expr: &str,
    context: &str,
    content_before_offset: &str,
) -> Option<(String, f32, String)> {
    let compact = compact_expression(expr);
    if compact.is_empty() || extract_static_string_value(&compact).is_some() {
        return None;
    }

    let mut effective_expr = compact.clone();
    let identifier_re = Regex::new(r#"^[A-Za-z_][A-Za-z0-9_]*$"#).ok()?;
    let mut assignment_note = String::new();

    if identifier_re.is_match(&compact) {
        if let Some(assigned) = lookup_variable_assignment(&compact, content_before_offset) {
            effective_expr = compact_expression(&assigned);
            assignment_note = format!("，变量 {} 来源于 {}", compact, effective_expr);
        }
    }

    let lower_expr = effective_expr.to_lowercase();
    let lower_context = context.to_lowercase();
    let asset_keywords = [
        "path",
        "icon",
        "sprite",
        "texture",
        "tex",
        "prefab",
        "model",
        "asset",
        "address",
        "bundle",
        "scene",
        "audio",
        "clip",
        "bgm",
        "sfx",
        "effect",
        "fx",
        "material",
        "mat",
        "avatar",
        "anim",
        "controller",
        "font",
        "ui",
        "res",
        "resource",
    ];
    let data_source_keywords = [
        "cfg",
        "conf",
        "config",
        "table",
        "row",
        "record",
        "excel",
        "luban",
        "proto",
        "protobuf",
        "deserialize",
        "deserializ",
        "parsefrom",
        "messageparser",
        "serializer.deserialize",
        "jsonutility.fromjson",
        "fromjson",
        "datatable",
        "tb",
        "template",
    ];

    let asset_hit = asset_keywords.iter().any(|kw| lower_expr.contains(kw));
    let data_hit = data_source_keywords
        .iter()
        .any(|kw| lower_expr.contains(kw) || lower_context.contains(kw));
    let dynamic_shape = effective_expr.contains('.')
        || effective_expr.contains('[')
        || effective_expr.contains('+')
        || effective_expr.contains('?')
        || effective_expr.contains(':')
        || effective_expr.contains("Get")
        || effective_expr.contains("get");
    let identifier_asset_hint = identifier_re.is_match(&compact)
        && asset_keywords
            .iter()
            .any(|kw| compact.to_lowercase().contains(kw));

    if !(data_hit
        || (asset_hit && dynamic_shape)
        || identifier_asset_hint
        || effective_expr != compact)
    {
        return None;
    }

    let label = if data_hit {
        "配置表/序列化字段"
    } else {
        "动态资源表达式"
    };
    let confidence = if data_hit { 0.72 } else { 0.58 };
    let detail = format!("{}驱动：{}{}", label, effective_expr, assignment_note);
    Some((effective_expr, confidence, detail))
}

fn push_dynamic_expression_suspected_ref(
    suspected_refs: &mut Vec<SuspectedReference>,
    seen_refs: &mut HashSet<String>,
    ref_id: &mut u32,
    file_rel: &str,
    line_num: u32,
    trimmed: &str,
    method_name: &str,
    arg_expr: &str,
    unity_type: Option<&str>,
    context: &str,
    content_before_offset: &str,
    base_confidence: f32,
    heuristic_tag: &str,
) {
    let Some((effective_expr, inferred_confidence, detail)) =
        classify_dynamic_resource_expression(arg_expr, context, content_before_offset)
    else {
        return;
    };

    let display_expr = if effective_expr.len() > 160 {
        format!("{}...", &effective_expr[..160])
    } else {
        effective_expr.clone()
    };

    push_suspected_ref(
        suspected_refs,
        seen_refs,
        ref_id,
        file_rel,
        line_num,
        trimmed,
        method_name,
        format!("[动态键] {}", display_expr),
        asset_kind_from_unity_type(unity_type),
        base_confidence.max(inferred_confidence),
        Some(format!(
            "{}：{}{}",
            heuristic_tag,
            detail,
            unity_type
                .map(|t| format!("，类型约束 {}", t))
                .unwrap_or_default()
        )),
    );
}

/// Detect suspected dynamic resource loading patterns in code
pub fn detect_suspected_references(
    project_root: &Path,
    files: &[String],
) -> Vec<SuspectedReference> {
    let mut suspected_refs = Vec::new();
    let asset_inventory = build_asset_inventory(project_root);
    let unity_type_pattern = r#"([A-Za-z_][\w\.\[\],<>]*)"#;
    let unity_string_pattern = r#"[@$]?"([^"]*)""#;
    let resources_load_re = Regex::new(&format!(
        r#"Resources\.Load\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
        unity_type_pattern, unity_string_pattern, unity_type_pattern
    ))
    .unwrap();
    let resources_loadall_re = Regex::new(&format!(
        r#"Resources\.LoadAll\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
        unity_type_pattern, unity_string_pattern, unity_type_pattern
    ))
    .unwrap();
    let resources_loadasync_re = Regex::new(&format!(
        r#"Resources\.LoadAsync\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
        unity_type_pattern, unity_string_pattern, unity_type_pattern
    ))
    .unwrap();
    let instantiate_resources_load_re = Regex::new(
        &format!(
            r#"Instantiate\s*\(\s*Resources\.Load\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
            unity_type_pattern,
            unity_string_pattern,
            unity_type_pattern
        )
    ).unwrap();
    let known_engine_loader_re = Regex::new(
        &format!(
            r#"\b(?:Addressables|YooAssets|package|[A-Za-z_][\w]*(?:Package|Handle|Operation|Bundle))\s*\.\s*(LoadAssetAsync|LoadAsset|LoadAssetWithSubAssets|LoadAllAssets|LoadAllAssetsAsync|LoadSceneAsync|InstantiateAsync|LoadSubAssetsAsync|LoadAssetsAsync)\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
            unity_type_pattern,
            unity_string_pattern,
            unity_type_pattern
        )
    ).unwrap();
    let custom_manager_loader_re = Regex::new(
        &format!(
            r#"\b([A-Za-z_][\w]*(?:Manager|Mgr|Loader|Resource|Resources|Res|Asset|Assets|Bundle|Package)[A-Za-z_0-9]*)\s*\.\s*(Load|LoadAll|LoadAsync|LoadAsset|LoadAssetAsync|LoadAssetsAsync|LoadAllAssets|LoadAllAssetsAsync|LoadSceneAsync|InstantiateAsync)\s*(?:<\s*{}\s*>)?\s*\(\s*{}(?:\s*,\s*typeof\s*\(\s*{}\s*\))?"#,
            unity_type_pattern,
            unity_string_pattern,
            unity_type_pattern
        )
    ).unwrap();
    let resources_dynamic_call_re =
        Regex::new(r#"\bResources\.(Load|LoadAll|LoadAsync)\b"#).unwrap();
    let engine_loader_call_re = Regex::new(
        r#"\b(?:Addressables|YooAssets|package|[A-Za-z_][\w]*(?:Package|Handle|Operation|Bundle))\s*\.\s*(LoadAssetAsync|LoadAsset|LoadAssetWithSubAssets|LoadAllAssets|LoadAllAssetsAsync|LoadSceneAsync|InstantiateAsync|LoadSubAssetsAsync|LoadAssetsAsync)\b"#
    ).unwrap();
    let custom_manager_call_re = Regex::new(
        r#"\b([A-Za-z_][\w]*(?:Manager|Mgr|Loader|Resource|Resources|Res|Asset|Assets|Bundle|Package)[A-Za-z_0-9]*)\s*\.\s*(Load|LoadAll|LoadAsync|LoadAsset|LoadAssetAsync|LoadAssetsAsync|LoadAllAssets|LoadAllAssetsAsync|LoadSceneAsync|InstantiateAsync)\b"#
    ).unwrap();
    let simple_load_patterns: Vec<(Regex, &str, f32)> = vec![
        (
            Regex::new(r#"AssetBundle\.Load(?:From(?:File|Memory|Stream))?\s*\(\s*"([^"]+)""#)
                .unwrap(),
            "AssetBundle.Load",
            0.7,
        ),
        // Godot GDScript
        (
            Regex::new(r#"(?:load|preload)\s*\(\s*"([^"]+)""#).unwrap(),
            "GDScript.load",
            0.95,
        ),
        (
            Regex::new(r#"ResourceLoader\.load\s*\(\s*"([^"]+)""#).unwrap(),
            "ResourceLoader.load",
            0.9,
        ),
    ];

    let code_extensions = ["cs", "gd"];
    let mut ref_id = 0u32;
    let mut seen_refs = HashSet::new();

    for file_rel in files {
        let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();
        if !code_extensions.contains(&ext.as_str()) {
            continue;
        }

        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut handled_instantiate_ranges = Vec::new();
        for caps in instantiate_resources_load_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            handled_instantiate_ranges.push((full_match.start(), full_match.end()));
            let unity_type = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
            let resource_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            push_unity_resources_candidates(
                project_root,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                "Instantiate+Resources.Load",
                resource_path,
                unity_type,
                0.9,
                false,
            );
        }

        for caps in resources_loadall_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let unity_type = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
            let resource_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            push_unity_resources_candidates(
                project_root,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                "Resources.LoadAll",
                resource_path,
                unity_type,
                0.85,
                true,
            );
        }

        for caps in resources_loadasync_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let unity_type = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
            let resource_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            push_unity_resources_candidates(
                project_root,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                "Resources.LoadAsync",
                resource_path,
                unity_type,
                0.9,
                false,
            );
        }

        'resources_loads: for caps in resources_load_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            for (start, end) in &handled_instantiate_ranges {
                if full_match.start() >= *start && full_match.end() <= *end {
                    continue 'resources_loads;
                }
            }

            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let unity_type = caps.get(1).or_else(|| caps.get(3)).map(|m| m.as_str());
            let resource_path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            push_unity_resources_candidates(
                project_root,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                "Resources.Load",
                resource_path,
                unity_type,
                0.9,
                false,
            );
        }

        for caps in known_engine_loader_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let method_name = caps.get(1).map(|m| m.as_str()).unwrap_or("LoadAssetAsync");
            let unity_type = caps
                .get(2)
                .or_else(|| caps.get(4))
                .map(|m| m.as_str())
                .or_else(|| {
                    if method_name.contains("Instantiate") {
                        Some("GameObject")
                    } else if method_name.contains("Scene") {
                        Some("SceneAsset")
                    } else {
                        None
                    }
                });
            let resource_path = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let load_all = matches!(method_name, "LoadAllAssets" | "LoadAllAssetsAsync");
            push_asset_inventory_candidates(
                &asset_inventory,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                method_name,
                resource_path,
                unity_type,
                0.72,
                load_all,
                false,
                "引擎资源加载推断",
            );
        }

        for caps in custom_manager_loader_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let receiver = caps.get(1).map(|m| m.as_str()).unwrap_or("CustomManager");
            let method = caps.get(2).map(|m| m.as_str()).unwrap_or("Load");
            let receiver_lower = receiver.to_lowercase();
            let unity_type = caps
                .get(3)
                .or_else(|| caps.get(5))
                .map(|m| m.as_str())
                .or_else(|| {
                    if method.contains("Instantiate") {
                        Some("GameObject")
                    } else if method.contains("Scene") {
                        Some("SceneAsset")
                    } else {
                        None
                    }
                });
            let resource_path = caps.get(4).map(|m| m.as_str()).unwrap_or("");
            let load_all = matches!(method, "LoadAll" | "LoadAllAssets" | "LoadAllAssetsAsync");
            let prefer_resources = receiver_lower.contains("resource")
                || receiver_lower.contains("resmanager")
                || receiver_lower.contains("resources")
                || matches!(method, "Load" | "LoadAll" | "LoadAsync");
            push_asset_inventory_candidates(
                &asset_inventory,
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                &format!("{}.{}", receiver, method),
                resource_path,
                unity_type,
                0.6,
                load_all,
                prefer_resources,
                "自定义加载器启发式推断",
            );
        }

        'dynamic_resources: for caps in resources_dynamic_call_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            for (start, end) in &handled_instantiate_ranges {
                if full_match.start() >= *start && full_match.end() <= *end {
                    continue 'dynamic_resources;
                }
            }
            let Some(invocation) = extract_invocation_slice(&content, full_match.start()) else {
                continue;
            };
            let Some(arg_expr) = extract_first_argument_expression(&invocation) else {
                continue;
            };
            if extract_static_string_value(&arg_expr).is_some() {
                continue;
            }
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let context = context_window_at_offset(&content, full_match.start(), 5);
            let content_before_offset = &content[..full_match.start()];
            let method_name = caps.get(1).map(|m| m.as_str()).unwrap_or("Load");
            let unity_type = extract_unity_type_from_invocation(&invocation);
            if let Some(static_path) =
                resolve_expression_to_static_string(&arg_expr, content_before_offset)
            {
                push_unity_resources_candidates(
                    project_root,
                    &mut suspected_refs,
                    &mut seen_refs,
                    &mut ref_id,
                    file_rel,
                    line_number,
                    &excerpt,
                    &format!("Resources.{}", method_name),
                    &static_path,
                    unity_type.as_deref(),
                    0.82,
                    method_name == "LoadAll",
                );
                continue;
            }
            push_dynamic_expression_suspected_ref(
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                &format!("Resources.{}", method_name),
                &arg_expr,
                unity_type.as_deref(),
                &context,
                content_before_offset,
                0.62,
                "Resources 动态键推断",
            );
        }

        for caps in engine_loader_call_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let Some(invocation) = extract_invocation_slice(&content, full_match.start()) else {
                continue;
            };
            let Some(arg_expr) = extract_first_argument_expression(&invocation) else {
                continue;
            };
            if extract_static_string_value(&arg_expr).is_some() {
                continue;
            }
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let context = context_window_at_offset(&content, full_match.start(), 5);
            let content_before_offset = &content[..full_match.start()];
            let method_name = caps.get(1).map(|m| m.as_str()).unwrap_or("LoadAssetAsync");
            let unity_type = extract_unity_type_from_invocation(&invocation).or_else(|| {
                if method_name.contains("Instantiate") {
                    Some("GameObject".to_string())
                } else if method_name.contains("Scene") {
                    Some("SceneAsset".to_string())
                } else {
                    None
                }
            });
            if let Some(static_path) =
                resolve_expression_to_static_string(&arg_expr, content_before_offset)
            {
                push_asset_inventory_candidates(
                    &asset_inventory,
                    &mut suspected_refs,
                    &mut seen_refs,
                    &mut ref_id,
                    file_rel,
                    line_number,
                    &excerpt,
                    method_name,
                    &static_path,
                    unity_type.as_deref(),
                    0.72,
                    matches!(method_name, "LoadAllAssets" | "LoadAllAssetsAsync"),
                    false,
                    "引擎资源加载推断",
                );
                continue;
            }
            push_dynamic_expression_suspected_ref(
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                method_name,
                &arg_expr,
                unity_type.as_deref(),
                &context,
                content_before_offset,
                0.58,
                "引擎资源系统动态键推断",
            );
        }

        for caps in custom_manager_call_re.captures_iter(&content) {
            let Some(full_match) = caps.get(0) else {
                continue;
            };
            let Some(invocation) = extract_invocation_slice(&content, full_match.start()) else {
                continue;
            };
            let Some(arg_expr) = extract_first_argument_expression(&invocation) else {
                continue;
            };
            if extract_static_string_value(&arg_expr).is_some() {
                continue;
            }
            let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
            if is_comment_like_excerpt(&excerpt) {
                continue;
            }
            let context = context_window_at_offset(&content, full_match.start(), 5);
            let content_before_offset = &content[..full_match.start()];
            let receiver = caps.get(1).map(|m| m.as_str()).unwrap_or("CustomManager");
            let method_name = caps.get(2).map(|m| m.as_str()).unwrap_or("Load");
            let unity_type = extract_unity_type_from_invocation(&invocation).or_else(|| {
                if method_name.contains("Instantiate") {
                    Some("GameObject".to_string())
                } else if method_name.contains("Scene") {
                    Some("SceneAsset".to_string())
                } else {
                    None
                }
            });
            let prefer_resources = {
                let receiver_lower = receiver.to_lowercase();
                receiver_lower.contains("resource")
                    || receiver_lower.contains("resmanager")
                    || receiver_lower.contains("resources")
                    || matches!(method_name, "Load" | "LoadAll" | "LoadAsync")
            };
            if let Some(static_path) =
                resolve_expression_to_static_string(&arg_expr, content_before_offset)
            {
                push_asset_inventory_candidates(
                    &asset_inventory,
                    &mut suspected_refs,
                    &mut seen_refs,
                    &mut ref_id,
                    file_rel,
                    line_number,
                    &excerpt,
                    &format!("{}.{}", receiver, method_name),
                    &static_path,
                    unity_type.as_deref(),
                    0.64,
                    matches!(
                        method_name,
                        "LoadAll" | "LoadAllAssets" | "LoadAllAssetsAsync"
                    ),
                    prefer_resources,
                    "自定义加载器启发式推断",
                );
                continue;
            }
            push_dynamic_expression_suspected_ref(
                &mut suspected_refs,
                &mut seen_refs,
                &mut ref_id,
                file_rel,
                line_number,
                &excerpt,
                &format!("{}.{}", receiver, method_name),
                &arg_expr,
                unity_type.as_deref(),
                &context,
                content_before_offset,
                0.52,
                "自定义加载器动态键推断",
            );
        }

        for (pattern, method_name, confidence) in &simple_load_patterns {
            for caps in pattern.captures_iter(&content) {
                let Some(full_match) = caps.get(0) else {
                    continue;
                };
                let (line_number, excerpt) = line_context_at_offset(&content, full_match.start());
                if is_comment_like_excerpt(&excerpt) {
                    continue;
                }
                let resource_path = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
                push_suspected_ref(
                    &mut suspected_refs,
                    &mut seen_refs,
                    &mut ref_id,
                    file_rel,
                    line_number,
                    &excerpt,
                    method_name,
                    resource_path.clone(),
                    infer_asset_kind_from_path(&resource_path),
                    *confidence,
                    None,
                );
            }
        }
    }

    suspected_refs
}

// Directory exclusion is handled by workspace::is_ignored_entry()

/// Parse C# and GDScript files to extract classes, methods, and member variables
pub fn parse_code_structure(
    project_root: &Path,
    files: &[String],
) -> (HashMap<String, GraphNode>, Vec<GraphEdge>) {
    let mut store = GraphStore::new();
    for file_rel in files {
        let ext = file_rel.rsplit('.').next().unwrap_or("").to_lowercase();
        let full_path = project_root.join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        match ext.as_str() {
            "cs" => parse_csharp_file(file_rel, &content, &mut store),
            "gd" => parse_gdscript_file(file_rel, &content, &mut store),
            _ => {}
        }
    }
    (store.nodes, store.edges)
}

fn parse_csharp_file(file_rel: &str, content: &str, store: &mut GraphStore) {
    let class_re = Regex::new(
        r"(?:public|private|protected|internal)\s+(?:(?:abstract|sealed|static|partial)\s+)*(?:class|struct)\s+(\w+)"
    ).unwrap();
    let iface_re =
        Regex::new(r"(?:public|private|protected|internal)\s+(?:partial\s+)?interface\s+(\w+)")
            .unwrap();

    #[derive(Clone)]
    struct TypeContext {
        node_id: String,
        body_depth: i32,
    }

    let mut type_stack: Vec<TypeContext> = Vec::new();
    let mut depth: i32 = 0;
    let mut in_block_comment = false;

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let normalized = strip_csharp_leading_attributes(trimmed);
        let line_num = (line_idx + 1) as u32;

        // Handle block comments
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = true;
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        // Skip single-line comments, empty lines, and pure attribute lines
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("///")
            || normalized.is_empty()
        {
            continue;
        }

        // Check for class/struct declaration
        if let Some(caps) = class_re.captures(&normalized) {
            let class_name = caps[1].to_string();
            let parent_id = type_stack.last().map(|ctx| ctx.node_id.clone());
            let node_id = if let Some(parent) = parent_id.clone() {
                format!("{}::{}", parent, class_name)
            } else {
                format!("{}::{}", file_rel, class_name)
            };

            store.add_node(GraphNode {
                id: node_id.clone(),
                name: class_name,
                node_type: NodeType::Class,
                asset_kind: None,
                file_path: Some(file_rel.to_string()),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });

            store.add_edge(GraphEdge {
                source: parent_id.unwrap_or_else(|| file_rel.to_string()),
                target: node_id.clone(),
                edge_type: EdgeType::Contains,
                reference_class: ReferenceClass::Official,
                label: None,
                evidence: None,
            });

            type_stack.push(TypeContext {
                node_id,
                body_depth: depth,
            });
        }
        // Check for interface declaration
        else if let Some(caps) = iface_re.captures(&normalized) {
            let iface_name = caps[1].to_string();
            let parent_id = type_stack.last().map(|ctx| ctx.node_id.clone());
            let node_id = if let Some(parent) = parent_id.clone() {
                format!("{}::{}", parent, iface_name)
            } else {
                format!("{}::{}", file_rel, iface_name)
            };

            store.add_node(GraphNode {
                id: node_id.clone(),
                name: iface_name,
                node_type: NodeType::Interface,
                asset_kind: None,
                file_path: Some(file_rel.to_string()),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });

            store.add_edge(GraphEdge {
                source: parent_id.unwrap_or_else(|| file_rel.to_string()),
                target: node_id.clone(),
                edge_type: EdgeType::Contains,
                reference_class: ReferenceClass::Official,
                label: None,
                evidence: None,
            });

            type_stack.push(TypeContext {
                node_id,
                body_depth: depth,
            });
        }
        // Inside a class: check for methods and fields at the class body level
        else if let Some(class_ctx) = type_stack.last() {
            if depth == class_ctx.body_depth + 1 {
                if is_csharp_method(&normalized) {
                    if let Some(name) = extract_csharp_method_name(&normalized) {
                        let class_id = &class_ctx.node_id;
                        let method_id = format!("{}::{}", class_id, name);
                        let unique_id = if store.nodes.contains_key(&method_id) {
                            format!("{}::{}_{}", class_id, name, line_num)
                        } else {
                            method_id
                        };

                        store.add_node(GraphNode {
                            id: unique_id.clone(),
                            name,
                            node_type: NodeType::Method,
                            asset_kind: None,
                            file_path: Some(file_rel.to_string()),
                            line_number: Some(line_num),
                            metadata: HashMap::new(),
                        });

                        store.add_edge(GraphEdge {
                            source: class_id.clone(),
                            target: unique_id,
                            edge_type: EdgeType::Declares,
                            reference_class: ReferenceClass::Official,
                            label: None,
                            evidence: None,
                        });
                    }
                } else if is_csharp_field_or_property(&normalized) {
                    if let Some(name) = extract_csharp_field_name(&normalized) {
                        let class_id = &class_ctx.node_id;
                        let field_id = format!("{}::{}", class_id, name);
                        if !store.nodes.contains_key(&field_id) {
                            store.add_node(GraphNode {
                                id: field_id.clone(),
                                name,
                                node_type: NodeType::MemberVariable,
                                asset_kind: None,
                                file_path: Some(file_rel.to_string()),
                                line_number: Some(line_num),
                                metadata: HashMap::new(),
                            });

                            store.add_edge(GraphEdge {
                                source: class_id.clone(),
                                target: field_id,
                                edge_type: EdgeType::Declares,
                                reference_class: ReferenceClass::Official,
                                label: None,
                                evidence: None,
                            });
                        }
                    }
                }
            }
        }

        // Count braces (skip strings)
        let mut in_str = false;
        let mut in_chr = false;
        let mut chars = trimmed.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '"' if !in_chr => in_str = !in_str,
                '\'' if !in_str => in_chr = !in_chr,
                '\\' if in_str || in_chr => {
                    chars.next();
                }
                '{' if !in_str && !in_chr => depth += 1,
                '}' if !in_str && !in_chr => {
                    depth -= 1;
                    while type_stack
                        .last()
                        .map(|ctx| depth <= ctx.body_depth)
                        .unwrap_or(false)
                    {
                        type_stack.pop();
                    }
                }
                _ => {}
            }
        }
    }
}

fn strip_csharp_leading_attributes(line: &str) -> String {
    let mut rest = line.trim_start();

    loop {
        if !rest.starts_with('[') {
            break;
        }

        let mut depth = 0i32;
        let mut end_idx = None;
        for (idx, ch) in rest.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = Some(idx);
                        break;
                    }
                }
                _ => {}
            }
        }

        let Some(idx) = end_idx else {
            break;
        };
        rest = rest[idx + 1..].trim_start();
    }

    rest.to_string()
}

fn is_csharp_method(line: &str) -> bool {
    let has_modifier = line.starts_with("public ")
        || line.starts_with("private ")
        || line.starts_with("protected ")
        || line.starts_with("internal ")
        || line.starts_with("static ")
        || line.starts_with("override ")
        || line.starts_with("virtual ")
        || line.starts_with("abstract ")
        || line.starts_with("async ");

    has_modifier
        && line.contains('(')
        && !line.contains("class ")
        && !line.contains("struct ")
        && !line.contains("interface ")
        && !line.contains("delegate ")
        && !line.contains("event ")
        && !line.contains("enum ")
}

fn extract_csharp_method_name(line: &str) -> Option<String> {
    let paren_idx = line.find('(')?;
    let before_paren = line[..paren_idx].trim_end();
    let name = before_paren
        .rsplit(|c: char| c.is_whitespace() || c == '>' || c == ']')
        .next()?
        .trim();

    if name.is_empty() {
        return None;
    }

    let lower = name.to_lowercase();
    if [
        "if", "for", "while", "switch", "catch", "using", "lock", "foreach", "new", "return",
    ]
    .contains(&lower.as_str())
    {
        return None;
    }

    Some(name.to_string())
}

fn is_csharp_field_or_property(line: &str) -> bool {
    let has_modifier = line.starts_with("public ")
        || line.starts_with("private ")
        || line.starts_with("protected ")
        || line.starts_with("internal ");

    has_modifier
        && !line.contains('(')
        && !line.contains("class ")
        && !line.contains("struct ")
        && !line.contains("interface ")
        && !line.contains("delegate ")
        && !line.contains("enum ")
        && !line.contains("event ")
        && (line.contains(';') || line.contains('{') || line.contains('='))
}

fn extract_csharp_field_name(line: &str) -> Option<String> {
    let end_idx = line
        .find(|c: char| c == ';' || c == '{' || c == '=')
        .unwrap_or(line.len());
    let before_end = line[..end_idx].trim_end();
    let name = before_end
        .rsplit(|c: char| c.is_whitespace() || c == '>' || c == ']')
        .next()?
        .trim();

    if name.is_empty() || name.starts_with('<') || name.starts_with('[') {
        return None;
    }

    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csharp_file_keeps_outer_class_members_after_nested_types() {
        let source = r#"
using UnityEngine;

public class SocketManager : MonoBehaviour
{
    [SerializeField] private DemoConfig config;
    private int counter;

    private struct NestedData
    {
        public int Value;
    }

    private void Awake()
    {
    }

    public void ForceRefreshSockets()
    {
    }
}
"#;

        let mut store = GraphStore::new();
        parse_csharp_file("Assets/Test/SocketManager.cs", source, &mut store);

        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::config"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::counter"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::Awake"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::ForceRefreshSockets"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::NestedData"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::NestedData::Value"));
    }

    #[test]
    fn parse_csharp_file_handles_socket_manager_like_structure() {
        let source = r#"
using UnityEngine;

namespace Demo.Interaction
{
    public class SocketManager : MonoBehaviour
    {
        [SerializeField] private DemoConfig config;
        [SerializeField] private Camera mainCamera;
        private SocketPoint[] _sockets;

        private struct AdhesiveBlobEntry
        {
            public SocketPoint SocketA;
            public SocketPoint SocketB;
        }

        private struct RendererFadeState
        {
            public Color Color;
            public int RenderQueue;
        }

        private float SnapDistanceThreshold => config != null ? config.snapDistanceThreshold : 0.15f;
        private float SnapSurfaceSeparation => config != null
            ? Mathf.Max(0f, config.snapSurfaceSeparation)
            : 0f;

        private void OnValidate()
        {
        }

        private void Awake()
        {
        }

        public void ForceRefreshSockets()
        {
        }
    }
}
"#;

        let mut store = GraphStore::new();
        parse_csharp_file("Assets/Test/SocketManager.cs", source, &mut store);

        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::OnValidate"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::Awake"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::ForceRefreshSockets"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::SnapDistanceThreshold"));
        assert!(store
            .nodes
            .contains_key("Assets/Test/SocketManager.cs::SocketManager::SnapSurfaceSeparation"));
        assert!(store.nodes.contains_key(
            "Assets/Test/SocketManager.cs::SocketManager::RendererFadeState::RenderQueue"
        ));
    }
}

fn parse_gdscript_file(file_rel: &str, content: &str, store: &mut GraphStore) {
    let func_re = Regex::new(r"^func\s+(\w+)").unwrap();
    let var_re = Regex::new(r"^(?:@\w+\s+)*var\s+(\w+)").unwrap();
    let class_name_re = Regex::new(r"^class_name\s+(\w+)").unwrap();

    let mut class_id: Option<String> = None;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = (line_idx + 1) as u32;

        // Only process non-indented lines (class-level declarations)
        if line.starts_with('\t') || line.starts_with("    ") {
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(caps) = class_name_re.captures(trimmed) {
            let name = caps[1].to_string();
            let id = format!("{}::{}", file_rel, name);
            store.add_node(GraphNode {
                id: id.clone(),
                name,
                node_type: NodeType::Class,
                asset_kind: None,
                file_path: Some(file_rel.to_string()),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });
            store.add_edge(GraphEdge {
                source: file_rel.to_string(),
                target: id.clone(),
                edge_type: EdgeType::Contains,
                reference_class: ReferenceClass::Official,
                label: None,
                evidence: None,
            });
            class_id = Some(id);
        } else if let Some(caps) = func_re.captures(trimmed) {
            let name = caps[1].to_string();
            let parent = class_id.as_deref().unwrap_or(file_rel);
            let id = format!("{}::{}", parent, name);
            store.add_node(GraphNode {
                id: id.clone(),
                name,
                node_type: NodeType::Method,
                asset_kind: None,
                file_path: Some(file_rel.to_string()),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });
            store.add_edge(GraphEdge {
                source: parent.to_string(),
                target: id,
                edge_type: EdgeType::Declares,
                reference_class: ReferenceClass::Official,
                label: None,
                evidence: None,
            });
        } else if let Some(caps) = var_re.captures(trimmed) {
            let name = caps[1].to_string();
            let parent = class_id.as_deref().unwrap_or(file_rel);
            let id = format!("{}::{}", parent, name);
            store.add_node(GraphNode {
                id: id.clone(),
                name,
                node_type: NodeType::MemberVariable,
                asset_kind: None,
                file_path: Some(file_rel.to_string()),
                line_number: Some(line_num),
                metadata: HashMap::new(),
            });
            store.add_edge(GraphEdge {
                source: parent.to_string(),
                target: id,
                edge_type: EdgeType::Declares,
                reference_class: ReferenceClass::Official,
                label: None,
                evidence: None,
            });
        }
    }
}

// ======================== V2: Redundancy Detection ========================

/// Detect orphan nodes — files with zero in-degree and zero out-degree (no references at all)
pub fn detect_orphan_nodes(
    store: &GraphStore,
    project_root: &Path,
) -> Vec<OrphanReport> {
    let mut in_degree: HashMap<&str, u32> = HashMap::new();
    let mut out_degree: HashMap<&str, u32> = HashMap::new();

    for edge in &store.edges {
        // Only count non-Contains/Declares edges (structural edges don't indicate usage)
        if matches!(edge.edge_type, EdgeType::Contains | EdgeType::Declares) {
            continue;
        }
        *in_degree.entry(edge.target.as_str()).or_insert(0) += 1;
        *out_degree.entry(edge.source.as_str()).or_insert(0) += 1;
    }

    let mut orphans = Vec::new();

    for node in store.nodes.values() {
        // Only check leaf-level nodes (files), skip directories and structural nodes
        if matches!(node.node_type, NodeType::Directory | NodeType::Class | NodeType::Method | NodeType::MemberVariable | NodeType::Interface | NodeType::Module) {
            continue;
        }

        let in_d = in_degree.get(node.id.as_str()).copied().unwrap_or(0);
        let out_d = out_degree.get(node.id.as_str()).copied().unwrap_or(0);

        if in_d == 0 && out_d == 0 {
            let file_size = node.file_path.as_ref()
                .map(|fp| {
                    let full = project_root.join(fp.replace('/', std::path::MAIN_SEPARATOR_STR));
                    fs::metadata(&full).map(|m| m.len()).unwrap_or(0)
                })
                .unwrap_or(0);

            let suggestion = match node.node_type {
                NodeType::Asset => "未被任何文件引用的资源，考虑删除以减小包体".to_string(),
                NodeType::CodeFile => "未被引用的代码文件，可能为死代码".to_string(),
                _ => "孤立节点，无引用关系".to_string(),
            };

            orphans.push(OrphanReport {
                node_id: node.id.clone(),
                node_name: node.name.clone(),
                node_type: node.node_type.clone(),
                asset_kind: node.asset_kind.clone(),
                file_path: node.file_path.clone(),
                file_size_bytes: file_size,
                suggestion,
            });
        }
    }

    orphans.sort_by(|a, b| b.file_size_bytes.cmp(&a.file_size_bytes));
    orphans
}

/// Detect duplicate files by size + SHA256 hash
pub fn detect_duplicates(
    store: &GraphStore,
    project_root: &Path,
) -> Vec<DuplicateGroup> {
    // Group asset/code file nodes by file size
    let mut size_groups: HashMap<u64, Vec<(&GraphNode, String)>> = HashMap::new();

    for node in store.nodes.values() {
        if !matches!(node.node_type, NodeType::Asset | NodeType::CodeFile) {
            continue;
        }
        let Some(fp) = &node.file_path else { continue };
        let full = project_root.join(fp.replace('/', std::path::MAIN_SEPARATOR_STR));
        let size = match fs::metadata(&full) {
            Ok(m) => m.len(),
            Err(_) => continue,
        };
        if size == 0 { continue; }
        size_groups.entry(size).or_default().push((node, full.to_string_lossy().to_string()));
    }

    let mut groups = Vec::new();
    let mut group_id = 0u32;

    for (size, candidates) in &size_groups {
        if candidates.len() < 2 { continue; }

        // Compute SHA256 for each file in this size group
        let mut hash_groups: HashMap<String, Vec<&GraphNode>> = HashMap::new();
        for (node, full_path) in candidates {
            let hash = match fs::read(full_path) {
                Ok(data) => {
                    let mut hasher = Sha256::new();
                    hasher.update(&data);
                    format!("{:x}", hasher.finalize())
                },
                Err(_) => continue,
            };
            hash_groups.entry(hash).or_default().push(node);
        }

        for (hash, nodes) in &hash_groups {
            if nodes.len() < 2 { continue; }
            group_id += 1;

            let items: Vec<DuplicateItem> = nodes.iter().map(|n| DuplicateItem {
                node_id: n.id.clone(),
                file_path: n.file_path.clone().unwrap_or_default(),
                file_size: *size,
                hash: Some(hash.clone()),
            }).collect();

            let total_size = *size * items.len() as u64;

            groups.push(DuplicateGroup {
                group_id: format!("dup_{}", group_id),
                asset_kind: nodes[0].asset_kind.clone(),
                files: items,
                total_size,
                similarity: 1.0,
            });
        }
    }

    groups.sort_by(|a, b| b.total_size.cmp(&a.total_size));
    groups
}

/// Detect hotspot nodes with high in-degree (many dependents)
pub fn detect_hotspots(
    store: &GraphStore,
    threshold: u32,
) -> Vec<HotspotReport> {
    let mut in_degree: HashMap<&str, Vec<&str>> = HashMap::new();

    for edge in &store.edges {
        if matches!(edge.edge_type, EdgeType::Contains | EdgeType::Declares) {
            continue;
        }
        in_degree.entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }

    let mut hotspots = Vec::new();

    for (node_id, dependents) in &in_degree {
        let count = dependents.len() as u32;
        if count < threshold { continue; }

        let Some(node) = store.nodes.get(*node_id) else { continue };

        let risk_level = if count >= threshold * 3 {
            RiskLevel::High
        } else if count >= threshold * 2 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        hotspots.push(HotspotReport {
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            node_type: node.node_type.clone(),
            file_path: node.file_path.clone(),
            in_degree: count,
            dependents: dependents.iter().map(|s| s.to_string()).collect(),
            risk_level,
        });
    }

    hotspots.sort_by(|a, b| b.in_degree.cmp(&a.in_degree));
    hotspots
}
