use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;

use crate::analysis;
use crate::asset_metrics;
use crate::ai_review;
use crate::device_profile;
use crate::device_transfer;
use crate::graph::model::*;
use crate::graph::store::{FrontendGraph, GraphStore};
use crate::profiler_report;
use crate::profiler_session;
use crate::unity_connection;
use crate::workspace;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Application state shared across commands
pub struct AppState {
    pub project: Mutex<Option<workspace::ProjectInfo>>,
    pub graph: Mutex<GraphStore>,
    pub unity_port: Mutex<Option<u16>>,
    pub profiler: profiler_session::ProfilerManager,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Mutex::new(None),
            graph: Mutex::new(GraphStore::new()),
            unity_port: Mutex::new(None),
            profiler: profiler_session::ProfilerManager::default(),
        }
    }
}

fn cloned_project_info(state: &State<'_, AppState>) -> Result<Option<workspace::ProjectInfo>, String> {
    let project = state.project.lock().map_err(|e| e.to_string())?;
    Ok(project.clone())
}

fn require_project_info(state: &State<'_, AppState>) -> Result<workspace::ProjectInfo, String> {
    cloned_project_info(state)?.ok_or("No project selected".to_string())
}

fn require_project_path(state: &State<'_, AppState>) -> Result<String, String> {
    Ok(require_project_info(state)?.path)
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("settings.json"))
}

fn sanitize_settings(mut settings: AppSettings) -> AppSettings {
    let defaults = AppSettings::default();
    let default_ai_cli = defaults.ai_cli.clone();
    let default_language = defaults.language.clone();
    let cli = settings.ai_cli.trim().to_ascii_lowercase();
    settings.ai_cli = match cli.as_str() {
        "claude" | "codex" | "gemini" | "copilot" => cli,
        _ => default_ai_cli,
    };

    let language = settings.language.trim().to_ascii_lowercase();
    settings.language = if language.starts_with("en") {
        "en".to_string()
    } else {
        default_language
    };

    // The backend only supports whole-project scans today. Normalize unsupported values
    // so saved settings match actual runtime behavior.
    settings.scan_scope = "full".to_string();

    settings.ai_model = settings.ai_model.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    settings.ai_thinking = settings.ai_thinking.and_then(|value| {
        let trimmed = value.trim().to_ascii_lowercase();
        matches!(trimmed.as_str(), "low" | "medium" | "high" | "xhigh").then_some(trimmed)
    });

    settings
}

fn load_settings_from_app(app: &tauri::AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(app)?;
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<AppSettings>(&content)
            .map(sanitize_settings)
            .map_err(|e| e.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(err) => Err(err.to_string()),
    }
}

fn resolve_project_relative_path(project_root: &str, file_path: &str) -> Result<PathBuf, String> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }

    let root = std::fs::canonicalize(project_root)
        .map_err(|e| format!("项目目录不存在: {}", e))?;
    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err("只允许访问项目内相对路径".to_string());
    }

    let mut candidate = root.clone();
    for component in rel.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => candidate.push(segment),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("非法项目内路径".to_string());
            }
        }
    }

    let resolved = std::fs::canonicalize(&candidate)
        .map_err(|e| format!("目标文件不存在: {}", e))?;
    if !resolved.starts_with(&root) {
        return Err("路径超出项目目录".to_string());
    }

    Ok(resolved)
}

fn emit_progress(
    app: &tauri::AppHandle,
    phase: &str,
    step: &str,
    current: u32,
    total: u32,
    message: &str,
) {
    let _ = app.emit(
        "analysis_progress",
        AnalysisProgress {
            phase: phase.to_string(),
            step: step.to_string(),
            current,
            total,
            message: message.to_string(),
        },
    );
}

fn emit_ai_log(app: &tauri::AppHandle, line: &str) {
    let _ = app.emit("ai_log", line.to_string());
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CliStatus {
    pub available: bool,
    pub resolved_path: Option<String>,
}

#[derive(Debug, Clone)]
struct CliInvocation {
    args: Vec<String>,
    stdin_payload: Option<String>,
}

fn windows_cli_path() -> Option<String> {
    let current_path = std::env::var("PATH").ok()?;
    let mut extra_dirs = Vec::new();

    if let Ok(appdata) = std::env::var("APPDATA") {
        extra_dirs.push(format!("{}\\npm", appdata));
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        extra_dirs.push(format!("{}\\.cargo\\bin", home));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        extra_dirs.push(format!("{}\\pnpm", local));
    }
    if let Ok(nvm) = std::env::var("NVM_SYMLINK") {
        extra_dirs.push(nvm);
    }

    if extra_dirs.is_empty() {
        Some(current_path)
    } else {
        Some(format!("{};{}", extra_dirs.join(";"), current_path))
    }
}

/// Build a TokioCommand with the same PATH augmentation as cli_command, for async usage.
fn async_cli_command(name: &str) -> TokioCommand {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = TokioCommand::new("cmd.exe");
        c.args(["/C", name]);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = TokioCommand::new(name);

    #[cfg(target_os = "windows")]
    {
        if let Some(new_path) = windows_cli_path() {
            cmd.env("PATH", new_path);
        }
    }

    cmd
}

fn build_cli_invocation(
    cli_name: &str,
    prompt: &str,
    model: &Option<String>,
    thinking: &Option<String>,
    codex_cd: Option<&str>,
) -> Result<CliInvocation, String> {
    match cli_name {
        "claude" => {
            let mut args = vec![
                "-p".to_string(),
                "请严格基于 stdin 中提供的完整分析上下文完成任务，不要读取额外文件，不要调用工具。"
                    .to_string(),
                "--output-format".to_string(),
                "text".to_string(),
                "--permission-mode".to_string(),
                "plan".to_string(),
            ];
            if let Some(m) = model {
                if !m.is_empty() {
                    args.push("--model".to_string());
                    args.push(m.clone());
                }
            }
            Ok(CliInvocation {
                args,
                stdin_payload: Some(prompt.to_string()),
            })
        }
        "codex" => {
            let mut args = vec!["exec".to_string(), "--skip-git-repo-check".to_string()];
            args.push("--sandbox".to_string());
            args.push("read-only".to_string());
            if let Some(dir) = codex_cd {
                if !dir.is_empty() {
                    args.push("--cd".to_string());
                    args.push(dir.to_string());
                }
            }
            if let Some(m) = model {
                if !m.is_empty() {
                    args.push("--model".to_string());
                    args.push(m.clone());
                }
            }
            if let Some(t) = thinking {
                if !t.is_empty() {
                    args.push("-c".to_string());
                    args.push(format!("model_reasoning_effort={}", t));
                }
            }
            // Read the full prompt from stdin to avoid Windows command-line length limits.
            args.push("-".to_string());
            Ok(CliInvocation {
                args,
                stdin_payload: Some(prompt.to_string()),
            })
        }
        "gemini" => {
            let mut args = vec![
                "--prompt".to_string(),
                "请严格基于 stdin 中提供的完整分析上下文完成任务，不要扫描或读取其他文件。"
                    .to_string(),
                "--output-format".to_string(),
                "text".to_string(),
                "--approval-mode".to_string(),
                "plan".to_string(),
            ];
            if let Some(m) = model {
                if !m.is_empty() {
                    args.push("--model".to_string());
                    args.push(m.clone());
                }
            }
            Ok(CliInvocation {
                args,
                stdin_payload: Some(prompt.to_string()),
            })
        }
        "copilot" => {
            let mut args = vec![
                "-s".to_string(),
                "--no-ask-user".to_string(),
                "--no-color".to_string(),
            ];
            if let Some(m) = model {
                if !m.is_empty() {
                    args.push("--model".to_string());
                    args.push(m.clone());
                }
            }
            Ok(CliInvocation {
                args,
                stdin_payload: Some(prompt.to_string()),
            })
        }
        _ => Err(format!("不支持的 AI CLI: {}", cli_name)),
    }
}

#[tauri::command]
pub fn detect_ai_clis() -> Result<HashMap<String, CliStatus>, String> {
    let cli_names = ["claude", "codex", "gemini", "copilot"];
    let mut result = HashMap::new();

    #[cfg(target_os = "windows")]
    {
        for cli_name in cli_names {
            let mut cmd = StdCommand::new("where.exe");
            if let Some(path) = windows_cli_path() {
                cmd.env("PATH", path);
            }
            let output = cmd.arg(cli_name).output();
            match output {
                Ok(out) if out.status.success() => {
                    let resolved = String::from_utf8_lossy(&out.stdout)
                        .lines()
                        .map(str::trim)
                        .find(|line| !line.is_empty())
                        .map(|line| line.to_string());
                    result.insert(
                        cli_name.to_string(),
                        CliStatus {
                            available: resolved.is_some(),
                            resolved_path: resolved,
                        },
                    );
                }
                Ok(_) | Err(_) => {
                    result.insert(
                        cli_name.to_string(),
                        CliStatus {
                            available: false,
                            resolved_path: None,
                        },
                    );
                }
            }
        }
        return Ok(result);
    }

    #[cfg(not(target_os = "windows"))]
    {
        for cli_name in cli_names {
            result.insert(
                cli_name.to_string(),
                CliStatus {
                    available: false,
                    resolved_path: None,
                },
            );
        }
        Ok(result)
    }
}

/// Select and scan a project directory
#[tauri::command]
pub fn select_project(
    path: String,
    state: State<'_, AppState>,
) -> Result<workspace::ProjectInfo, String> {
    let project_path = PathBuf::from(&path);
    let info = workspace::scan_project(&project_path)?;

    {
        let mut project = state.project.lock().map_err(|e| e.to_string())?;
        *project = Some(info.clone());
    }

    // Try to load cached analysis, otherwise reset graph
    let cache_path = project_path.join(".analytics").join("cache.json");
    let (next_graph, loaded_from_cache) = if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<GraphStore>(&data) {
                log::info!("Loaded cached analysis from {}", cache_path.display());
                (cached, true)
            } else {
                (GraphStore::new(), false)
            }
        } else {
            (GraphStore::new(), false)
        }
    } else {
        (GraphStore::new(), false)
    };

    let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
    *graph = next_graph;
    if loaded_from_cache {
        return Ok(info);
    }

    Ok(info)
}

/// Check if a cached analysis exists for the current project
#[tauri::command]
pub fn has_analysis_cache(state: State<'_, AppState>) -> Result<bool, String> {
    let project = state.project.lock().map_err(|e| e.to_string())?;
    if let Some(info) = project.as_ref() {
        let cache_path = PathBuf::from(&info.path)
            .join(".analytics")
            .join("cache.json");
        if !cache_path.is_file() {
            return Ok(false);
        }
        let is_valid = std::fs::read_to_string(&cache_path)
            .ok()
            .and_then(|data| serde_json::from_str::<GraphStore>(&data).ok())
            .is_some();
        Ok(is_valid)
    } else {
        Ok(false)
    }
}

/// Save current analysis state to cache
#[tauri::command]
pub fn save_analysis_cache(state: State<'_, AppState>) -> Result<(), String> {
    let project_path = require_project_path(&state)?;
    let cache_json = {
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        serde_json::to_string(&*graph).map_err(|e| e.to_string())?
    };

    let cache_dir = PathBuf::from(&project_path).join(".analytics");
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("创建缓存目录失败: {}", e))?;
    std::fs::write(cache_dir.join("cache.json"), &cache_json).map_err(|e| e.to_string())?;

    Ok(())
}

fn resolve_analysis_target_node_id(requested_node_id: &str, graph: &GraphStore) -> Option<String> {
    let node = graph.get_node(requested_node_id)?;
    match node.node_type {
        NodeType::CodeFile | NodeType::Asset | NodeType::SceneObject => Some(node.id.clone()),
        _ => {
            if let Some(file_path) = node.file_path.as_deref() {
                if graph.get_node(file_path).is_some() {
                    return Some(file_path.to_string());
                }
            }
            requested_node_id
                .split("::")
                .next()
                .filter(|id| graph.get_node(id).is_some())
                .map(|id| id.to_string())
        }
    }
}

fn build_ai_runtime_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("gamescript-analytics-ai");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 AI 工作目录失败: {}", e))?;
    Ok(dir)
}

fn to_project_file_path(project_root: &str, file_rel: &str) -> PathBuf {
    PathBuf::from(project_root).join(file_rel.replace('/', std::path::MAIN_SEPARATOR_STR))
}

fn try_load_text_source(project_root: &str, file_rel: &str) -> Result<String, String> {
    std::fs::read_to_string(to_project_file_path(project_root, file_rel))
        .map_err(|e| format!("无法读取文件源码: {}", e))
}

fn format_source_with_line_numbers(source: &str, max_chars: usize) -> String {
    let numbered: Vec<String> = source
        .lines()
        .enumerate()
        .map(|(idx, line)| format!("{:>4} | {}", idx + 1, line))
        .collect();

    let full = numbered.join("\n");
    if full.chars().count() <= max_chars {
        return full;
    }

    let total_lines = numbered.len();
    let mut current = 0usize;
    let mut kept = Vec::new();

    for line in &numbered {
        let line_len = line.chars().count() + 1;
        if current + line_len > max_chars {
            break;
        }
        kept.push(line.clone());
        current += line_len;
    }

    kept.push(format!(
        "... [已截断，原文件共 {} 行；为控制 prompt 长度仅保留前 {} 行]",
        total_lines,
        kept.len()
    ));

    kept.join("\n")
}

fn relative_symbol_name(file_id: &str, node: &GraphNode) -> String {
    let prefix = format!("{}::", file_id);
    let mut name = node
        .id
        .strip_prefix(&prefix)
        .unwrap_or(node.name.as_str())
        .replace("::", ".");
    if node.node_type == NodeType::Method && !name.ends_with("()") {
        name.push_str("()");
    }
    name
}

fn build_file_structure_summary(file_id: &str, graph: &GraphStore) -> String {
    let mut classes: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| {
            n.file_path.as_deref() == Some(file_id)
                && matches!(
                    n.node_type,
                    NodeType::Class | NodeType::Interface | NodeType::Module
                )
        })
        .collect();
    let mut methods: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| n.file_path.as_deref() == Some(file_id) && n.node_type == NodeType::Method)
        .collect();
    let mut members: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| {
            n.file_path.as_deref() == Some(file_id) && n.node_type == NodeType::MemberVariable
        })
        .collect();

    let sort_key = |node: &&GraphNode| (node.line_number.unwrap_or(u32::MAX), node.id.clone());
    classes.sort_by_key(sort_key);
    methods.sort_by_key(sort_key);
    members.sort_by_key(sort_key);

    let format_nodes = |items: &[&GraphNode]| -> String {
        if items.is_empty() {
            return "  无".to_string();
        }

        items
            .iter()
            .map(|node| {
                let line = node
                    .line_number
                    .map(|l| format!("L{}", l))
                    .unwrap_or_else(|| "行号未知".to_string());
                format!(
                    "  - {} | {:?} | {}",
                    relative_symbol_name(file_id, node),
                    node.node_type,
                    line
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "类/接口:\n{}\n\n函数:\n{}\n\n成员变量:\n{}",
        format_nodes(&classes),
        format_nodes(&methods),
        format_nodes(&members)
    )
}

fn build_file_reference_summary(
    file_id: &str,
    graph: &GraphStore,
    incoming: bool,
    limit: Option<usize>,
) -> Vec<String> {
    let mut edges: Vec<&GraphEdge> = graph
        .edges
        .iter()
        .filter(|e| {
            if matches!(e.edge_type, EdgeType::Contains | EdgeType::Declares) {
                return false;
            }
            if incoming {
                e.target == file_id
            } else {
                e.source == file_id
            }
        })
        .collect();

    edges.sort_by_key(|edge| {
        let other_id = if incoming { &edge.source } else { &edge.target };
        (other_id.clone(), format!("{:?}", edge.edge_type))
    });

    if let Some(max) = limit {
        edges.truncate(max);
    }

    edges
        .into_iter()
        .map(|edge| {
            let other_id = if incoming { &edge.source } else { &edge.target };
            let other_name = graph
                .get_node(other_id)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| other_id.rsplit('/').next().unwrap_or(other_id).to_string());
            let label = edge
                .label
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!(" | 线索: {}", s))
                .unwrap_or_default();
            let evidence = edge
                .evidence
                .as_ref()
                .map(|ev| {
                    let line = ev
                        .source_line
                        .map(|l| format!("L{}", l))
                        .unwrap_or_else(|| "-".to_string());
                    let rule = ev.rule.as_deref().unwrap_or("-");
                    format!(" | 证据: {} {} {}", ev.parser_type, line, rule)
                })
                .unwrap_or_default();

            format!(
                "  - {} ({}) | {:?}/{:?}{}{}",
                other_name, other_id, edge.edge_type, edge.reference_class, label, evidence
            )
        })
        .collect()
}

fn build_suspected_summary(file_id: &str, graph: &GraphStore, limit: Option<usize>) -> Vec<String> {
    let mut items: Vec<&SuspectedReference> = graph
        .suspected_refs
        .iter()
        .filter(|sr| sr.code_location == file_id || sr.resource_path == file_id)
        .collect();
    items.sort_by_key(|sr| (sr.code_line.unwrap_or(u32::MAX), sr.resource_path.clone()));
    if let Some(max) = limit {
        items.truncate(max);
    }

    items
        .into_iter()
        .map(|sr| {
            let line = sr
                .code_line
                .map(|l| format!("L{}", l))
                .unwrap_or_else(|| "-".to_string());
            let excerpt = sr.code_excerpt.as_deref().unwrap_or("");
            format!(
                "  - {} -> {} | {} | {:.0}% | {} | {}",
                sr.code_location,
                sr.resource_path,
                sr.load_method,
                sr.confidence * 100.0,
                line,
                excerpt
            )
        })
        .collect()
}

fn build_hardcode_summary(file_id: &str, graph: &GraphStore, limit: Option<usize>) -> Vec<String> {
    let mut items: Vec<&HardcodeFinding> = graph
        .hardcode_findings
        .iter()
        .filter(|h| h.file_path == file_id)
        .collect();
    items.sort_by_key(|h| h.line_number);
    if let Some(max) = limit {
        items.truncate(max);
    }

    items
        .into_iter()
        .map(|h| {
            format!(
                "  - L{} | {:?}/{:?} | \"{}\" | {}",
                h.line_number, h.category, h.severity, h.value, h.code_excerpt
            )
        })
        .collect()
}

fn build_method_static_flow_summary(file_id: &str, source: &str, graph: &GraphStore) -> String {
    #[derive(Clone)]
    struct MethodView {
        display: String,
        name: String,
        line: u32,
        end_line: u32,
    }

    let mut methods: Vec<MethodView> = graph
        .nodes
        .values()
        .filter(|n| n.file_path.as_deref() == Some(file_id) && n.node_type == NodeType::Method)
        .map(|node| MethodView {
            display: relative_symbol_name(file_id, node),
            name: node.name.clone(),
            line: node.line_number.unwrap_or(1),
            end_line: 0,
        })
        .collect();

    if methods.is_empty() {
        return "  无可用的方法级静态线索".to_string();
    }

    methods.sort_by_key(|m| (m.line, m.display.clone()));

    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return "  文件内容为空，无法提取方法级静态线索".to_string();
    }

    for idx in 0..methods.len() {
        let next_line = methods
            .get(idx + 1)
            .map(|m| m.line.saturating_sub(1))
            .unwrap_or(total_lines);
        methods[idx].end_line = next_line.max(methods[idx].line);
    }

    let outgoing_file_edges: Vec<&GraphEdge> = graph
        .edges
        .iter()
        .filter(|e| {
            e.source == file_id && !matches!(e.edge_type, EdgeType::Contains | EdgeType::Declares)
        })
        .collect();

    let mut outgoing_calls: Vec<Vec<usize>> = vec![Vec::new(); methods.len()];
    for (idx, method) in methods.iter().enumerate() {
        let start = method.line.saturating_sub(1) as usize;
        let end = method.end_line.min(total_lines) as usize;
        let body = if start < end {
            lines[start..end].join("\n")
        } else {
            String::new()
        };

        let mut seen_targets = HashSet::new();
        for (target_idx, target) in methods.iter().enumerate() {
            if idx == target_idx {
                continue;
            }
            let Ok(pattern) = Regex::new(&format!(r"\b{}\s*\(", regex::escape(&target.name)))
            else {
                continue;
            };
            if pattern.is_match(&body) {
                seen_targets.insert(target_idx);
            }
        }

        let mut ordered_targets: Vec<usize> = seen_targets.into_iter().collect();
        ordered_targets.sort_unstable();
        outgoing_calls[idx] = ordered_targets;
    }

    let mut incoming_calls: Vec<Vec<usize>> = vec![Vec::new(); methods.len()];
    for (caller_idx, targets) in outgoing_calls.iter().enumerate() {
        for &target_idx in targets {
            incoming_calls[target_idx].push(caller_idx);
        }
    }

    methods
        .iter()
        .enumerate()
        .map(|(idx, method)| {
            let start = method.line.saturating_sub(1) as usize;
            let end = method.end_line.min(total_lines) as usize;
            let body = if start < end {
                lines[start..end].join("\n")
            } else {
                String::new()
            };

            let mut parts = Vec::new();

            if !incoming_calls[idx].is_empty() {
                let names = incoming_calls[idx]
                    .iter()
                    .map(|caller_idx| methods[*caller_idx].display.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                parts.push(format!("被当前文件内调用: [{}]", names));
            }

            if !outgoing_calls[idx].is_empty() {
                let names = outgoing_calls[idx]
                    .iter()
                    .map(|target_idx| methods[*target_idx].display.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                parts.push(format!("调用: [{}]", names));
            }

            let mut external_hints = Vec::new();
            let mut seen_external = HashSet::new();
            for edge in &outgoing_file_edges {
                let label = edge.label.as_deref().unwrap_or("").trim();
                if label.is_empty() || !body.contains(label) {
                    continue;
                }
                let target = edge.target.rsplit('/').next().unwrap_or(&edge.target);
                let hint = format!("{} -> {}", label, target);
                if seen_external.insert(hint.clone()) {
                    external_hints.push(hint);
                }
            }
            if !external_hints.is_empty() {
                parts.push(format!("外部依赖线索: [{}]", external_hints.join(", ")));
            }

            let hardcodes = graph
                .hardcode_findings
                .iter()
                .filter(|h| {
                    h.file_path == file_id
                        && h.line_number >= method.line
                        && h.line_number <= method.end_line
                })
                .map(|h| format!("L{} {:?}", h.line_number, h.category))
                .collect::<Vec<_>>();
            if !hardcodes.is_empty() {
                parts.push(format!("硬编码: [{}]", hardcodes.join(", ")));
            }

            let dynamic_loads = graph
                .suspected_refs
                .iter()
                .filter(|sr| {
                    sr.code_location == file_id
                        && sr
                            .code_line
                            .map(|line| line >= method.line && line <= method.end_line)
                            .unwrap_or(false)
                })
                .map(|sr| format!("{} -> {}", sr.load_method, sr.resource_path))
                .collect::<Vec<_>>();
            if !dynamic_loads.is_empty() {
                parts.push(format!("动态加载: [{}]", dynamic_loads.join(", ")));
            }

            if parts.is_empty() {
                parts.push("未发现明显的静态调用线索".to_string());
            }

            format!(
                "  - {} [L{}-L{}]: {}",
                method.display,
                method.line,
                method.end_line,
                parts.join("; ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Update a node's AI summary in the graph store, merging with existing summary if present
#[tauri::command]
pub fn update_node_ai_summary(
    node_id: String,
    summary: String,
    analysis_type: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
    let target_node_id = resolve_analysis_target_node_id(&node_id, &graph).unwrap_or(node_id);
    if let Some(node) = graph.nodes.get_mut(&target_node_id) {
        let existing = node
            .metadata
            .get("ai_summary")
            .filter(|s| !s.is_empty() && *s != "未分析")
            .cloned();

        let new_summary = if let Some(existing_text) = existing {
            // Merge: check if the existing summary already contains both sections
            if analysis_type == "quick" && existing_text.contains("【深度分析】") {
                // Had deep, now adding quick — put quick first
                format!("【快速分析】\n{}\n\n{}\n", summary, existing_text)
            } else if analysis_type == "deep" && existing_text.contains("【快速分析】") {
                // Had quick, now adding deep — put deep after
                // Replace any existing deep section if present
                if existing_text.contains("【深度分析】") {
                    // Replace the deep section
                    let parts: Vec<&str> = existing_text.splitn(2, "【深度分析】").collect();
                    format!("{}【深度分析】\n{}", parts[0], summary)
                } else {
                    format!("{}\n【深度分析】\n{}", existing_text, summary)
                }
            } else if analysis_type == "deep" {
                // Had some other text (maybe old deep), replace entirely with new deep
                format!("【深度分析】\n{}", summary)
            } else {
                // Had some other text (maybe old quick), replace entirely with new quick
                format!("【快速分析】\n{}", summary)
            }
        } else {
            // No existing summary — tag it
            if analysis_type == "deep" {
                format!("【深度分析】\n{}", summary)
            } else {
                format!("【快速分析】\n{}", summary)
            }
        };

        node.metadata.insert("ai_summary".to_string(), new_summary);
        Ok(())
    } else {
        Err(format!("节点未找到: {}", target_node_id))
    }
}

/// Get currently loaded project info
#[tauri::command]
pub fn get_project_info(
    state: State<'_, AppState>,
) -> Result<Option<workspace::ProjectInfo>, String> {
    let project = state.project.lock().map_err(|e| e.to_string())?;
    Ok(project.clone())
}

/// Run analysis on the current project (async to avoid freezing UI on large projects)
#[tauri::command]
pub async fn run_analysis(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AnalysisStats, String> {
    let settings = load_settings_from_app(&app)?;
    let (project_path, engine) = {
        let project = state.project.lock().map_err(|e| e.to_string())?;
        let project_info = project.as_ref().ok_or("No project selected")?;
        (
            PathBuf::from(&project_info.path),
            project_info.engine.clone(),
        )
    };

    emit_progress(&app, "scan", "list_files", 0, 8, "正在扫描项目文件...");

    // List files on a blocking thread so we don't block the async runtime
    let pp = project_path.clone();
    let eng = engine.clone();
    let files = tokio::task::spawn_blocking(move || workspace::list_project_files(&pp, &eng))
        .await
        .map_err(|e| e.to_string())?;

    let total_files = files.len();
    emit_progress(
        &app,
        "scan",
        "create_nodes",
        1,
        8,
        &format!("正在创建文件节点 ({} 个文件)...", total_files),
    );

    // Create file nodes — fast, no I/O
    let mut graph = GraphStore::new();
    for file_path in &files {
        let name = file_path
            .rsplit('/')
            .next()
            .unwrap_or(file_path)
            .to_string();

        let ext = file_path.rsplit('.').next().unwrap_or("").to_lowercase();

        let (node_type, asset_kind) = classify_file(&ext);

        let node = GraphNode {
            id: file_path.clone(),
            name,
            node_type,
            asset_kind,
            file_path: Some(file_path.clone()),
            line_number: None,
            metadata: std::collections::HashMap::new(),
        };
        graph.add_node(node);
    }

    // Engine-specific analysis (heavy I/O — run on blocking thread)
    match engine {
        EngineType::Unity => {
            emit_progress(
                &app,
                "scan",
                "guid_map",
                2,
                8,
                "正在构建 Unity GUID 映射...",
            );
            let pp = project_path.clone();
            let guid_map = tokio::task::spawn_blocking(move || analysis::build_unity_guid_map(&pp))
                .await
                .map_err(|e| e.to_string())?;

            let unity_files: Vec<String> = files
                .iter()
                .filter(|f| {
                    let ext = f.rsplit('.').next().unwrap_or("").to_lowercase();
                    matches!(
                        ext.as_str(),
                        "prefab"
                            | "unity"
                            | "mat"
                            | "asset"
                            | "controller"
                            | "overridecontroller"
                            | "anim"
                    )
                })
                .cloned()
                .collect();
            let unity_total = unity_files.len();
            emit_progress(
                &app,
                "scan",
                "unity_refs",
                3,
                8,
                &format!("正在分析 Unity 引用关系 ({} 个文件)...", unity_total),
            );

            let node_ids: HashSet<String> = graph.nodes.keys().cloned().collect();
            let pp = project_path.clone();
            let edges = tokio::task::spawn_blocking(move || {
                analysis::analyze_unity_references(&pp, &unity_files, &guid_map, &node_ids)
            })
            .await
            .map_err(|e| e.to_string())?;

            for edge in edges {
                graph.add_edge(edge);
            }
        }
        EngineType::Godot => {
            emit_progress(
                &app,
                "scan",
                "godot_refs",
                2,
                8,
                "正在分析 Godot 引用关系...",
            );
        }
        _ => {
            emit_progress(&app, "scan", "generic", 2, 8, "正在分析引用关系...");
        }
    }

    // Code cross-reference analysis — the heaviest step, process in batches
    let code_files: Vec<String> = files
        .iter()
        .filter(|f| f.ends_with(".cs") || f.ends_with(".gd"))
        .cloned()
        .collect();
    let code_total = code_files.len();
    emit_progress(
        &app,
        "scan",
        "code_refs",
        4,
        8,
        &format!("正在分析代码交叉引用 ({} 个文件)...", code_total),
    );

    {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let class_map = tokio::task::spawn_blocking(move || analysis::build_class_map(&pp, &cf))
            .await
            .map_err(|e| e.to_string())?;

        emit_progress(
            &app,
            "scan",
            "code_refs",
            4,
            8,
            &format!(
                "正在匹配代码引用 — {} 个类, {} 个文件...",
                class_map.len(),
                code_total
            ),
        );

        let pp = project_path.clone();
        let cf = code_files.clone();
        let edges = tokio::task::spawn_blocking(move || {
            analysis::analyze_code_references_batch(&pp, &cf, &class_map)
        })
        .await
        .map_err(|e| e.to_string())?;

        for edge in edges {
            graph.add_edge(edge);
        }
    }

    // Parse code structure
    emit_progress(
        &app,
        "scan",
        "code_structure",
        5,
        8,
        &format!("正在解析代码结构 ({} 个文件)...", code_total),
    );
    {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let (new_nodes, new_edges) =
            tokio::task::spawn_blocking(move || analysis::parse_code_structure(&pp, &cf))
                .await
                .map_err(|e| e.to_string())?;

        for (id, node) in new_nodes {
            if !graph.nodes.contains_key(&id) {
                graph.add_node(node);
            }
        }
        for edge in new_edges {
            graph.add_edge(edge);
        }
    }

    // Hardcode detection
    emit_progress(
        &app,
        "scan",
        "hardcodes",
        6,
        8,
        &format!("正在检测硬编码 ({} 个文件)...", code_total),
    );
    if settings.hardcode_enabled {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let findings = tokio::task::spawn_blocking(move || analysis::detect_hardcodes(&pp, &cf))
            .await
            .map_err(|e| e.to_string())?;

        for f in findings {
            graph.add_hardcode_finding(f);
        }
    } else {
        emit_progress(
            &app,
            "scan",
            "hardcodes",
            6,
            8,
            "已跳过硬编码检测（设置关闭）",
        );
    }

    // Suspected dynamic references
    emit_progress(
        &app,
        "scan",
        "suspected",
        7,
        8,
        &format!("正在检测疑似引用 ({} 个文件)...", code_total),
    );
    if settings.suspected_enabled {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let refs =
            tokio::task::spawn_blocking(move || analysis::detect_suspected_references(&pp, &cf))
                .await
                .map_err(|e| e.to_string())?;

        for r in refs {
            graph.add_suspected_ref(r);
        }
    } else {
        emit_progress(
            &app,
            "scan",
            "suspected",
            7,
            8,
            "已跳过疑似引用检测（设置关闭）",
        );
    }

    graph.recalculate_stats();
    let stats = graph.stats.clone();

    // Store the completed graph into state
    {
        let mut state_graph = state.graph.lock().map_err(|e| e.to_string())?;
        *state_graph = graph;
    }

    emit_progress(&app, "scan", "done", 8, 8, "静态扫描完成");
    Ok(stats)
}

/// Get the asset graph data for frontend
#[tauri::command]
pub fn get_asset_graph(state: State<'_, AppState>) -> Result<FrontendGraph, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    let full = graph.to_frontend_graph();

    let node_type_map: HashMap<String, NodeType> = full
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.node_type.clone()))
        .collect();

    let is_asset_side = |node_type: &NodeType| {
        matches!(
            node_type,
            NodeType::Asset | NodeType::Directory | NodeType::SceneObject
        )
    };
    let is_script_side = |node_type: &NodeType| matches!(node_type, NodeType::CodeFile);

    let asset_ids: HashSet<String> = full
        .nodes
        .iter()
        .filter(|n| is_asset_side(&n.node_type))
        .map(|n| n.id.clone())
        .collect();

    let mut included_ids = asset_ids.clone();

    for edge in &full.edges {
        let source_type = node_type_map.get(&edge.source);
        let target_type = node_type_map.get(&edge.target);
        let is_confirmed_dynamic = edge.edge_type == EdgeType::DynamicLoad
            && edge.reference_class == ReferenceClass::UserConfirmed;

        if is_confirmed_dynamic
            && source_type.map(|ty| is_asset_side(ty)).unwrap_or(false)
            && target_type.map(|ty| is_script_side(ty)).unwrap_or(false)
        {
            included_ids.insert(edge.target.clone());
        }
        if is_confirmed_dynamic
            && target_type.map(|ty| is_asset_side(ty)).unwrap_or(false)
            && source_type.map(|ty| is_script_side(ty)).unwrap_or(false)
        {
            included_ids.insert(edge.source.clone());
        }
    }

    let nodes: Vec<_> = full
        .nodes
        .into_iter()
        .filter(|n| included_ids.contains(&n.id))
        .collect();
    let edges: Vec<_> = full
        .edges
        .into_iter()
        .filter(|e| {
            if asset_ids.contains(&e.source) && asset_ids.contains(&e.target) {
                return true;
            }

            e.edge_type == EdgeType::DynamicLoad
                && e.reference_class == ReferenceClass::UserConfirmed
                && included_ids.contains(&e.source)
                && included_ids.contains(&e.target)
        })
        .collect();

    Ok(FrontendGraph { nodes, edges })
}

/// Get the code graph data for frontend
#[tauri::command]
pub fn get_code_graph(state: State<'_, AppState>) -> Result<FrontendGraph, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    let full = graph.to_frontend_graph();

    // Filter to code-related nodes only
    let code_types = [
        NodeType::CodeFile,
        NodeType::Class,
        NodeType::Method,
        NodeType::MemberVariable,
        NodeType::Module,
        NodeType::Interface,
    ];
    let nodes: Vec<_> = full
        .nodes
        .into_iter()
        .filter(|n| code_types.contains(&n.node_type))
        .collect();
    let node_ids: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let edges: Vec<_> = full
        .edges
        .into_iter()
        .filter(|e| node_ids.contains(e.source.as_str()) && node_ids.contains(e.target.as_str()))
        .collect();

    Ok(FrontendGraph { nodes, edges })
}

/// Get analysis statistics
#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Result<AnalysisStats, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    Ok(graph.stats.clone())
}

/// Get suspected references
#[tauri::command]
pub fn get_suspected_refs(state: State<'_, AppState>) -> Result<Vec<SuspectedReference>, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    Ok(graph.suspected_refs.clone())
}

/// Promote a suspected reference to official
#[tauri::command]
pub fn promote_suspected_ref(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
    let changed = graph.promote_suspected(&id);
    if changed {
        graph.recalculate_stats();
    }
    Ok(changed)
}

/// Ignore a suspected reference
#[tauri::command]
pub fn ignore_suspected_ref(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
    let changed = graph.ignore_suspected(&id);
    if changed {
        graph.recalculate_stats();
    }
    Ok(changed)
}

/// Get hardcode findings
#[tauri::command]
pub fn get_hardcode_findings(state: State<'_, AppState>) -> Result<Vec<HardcodeFinding>, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    Ok(graph.hardcode_findings.clone())
}

fn classify_file(ext: &str) -> (NodeType, Option<AssetKind>) {
    match ext {
        "cs" | "gd" => (NodeType::CodeFile, Some(AssetKind::Script)),
        "unity" | "tscn" => (NodeType::Asset, Some(AssetKind::Scene)),
        "prefab" | "tres" => (NodeType::Asset, Some(AssetKind::Prefab)),
        "mat" => (NodeType::Asset, Some(AssetKind::Material)),
        "shader" | "cginc" | "gdshader" => (NodeType::Asset, Some(AssetKind::Shader)),
        "png" | "jpg" | "jpeg" | "tga" | "psd" | "tif" | "svg" => {
            (NodeType::Asset, Some(AssetKind::Texture))
        }
        "wav" | "mp3" | "ogg" | "aiff" => (NodeType::Asset, Some(AssetKind::Audio)),
        "anim" | "controller" | "overridecontroller" => {
            (NodeType::Asset, Some(AssetKind::Animation))
        }
        "fbx" | "obj" | "blend" | "glb" | "gltf" => (NodeType::Asset, Some(AssetKind::Data)),
        _ => (NodeType::Asset, Some(AssetKind::Other)),
    }
}

/// Save application settings
#[tauri::command]
pub fn save_settings(settings: AppSettings, app: tauri::AppHandle) -> Result<AppSettings, String> {
    let settings = sanitize_settings(settings);
    let path = settings_path(&app)?;
    let dir = path.parent().ok_or("无法确定设置目录".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(settings)
}

/// Load application settings
#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    load_settings_from_app(&app)
}

/// Open file location in system file explorer
#[tauri::command]
pub fn open_file_location(file_path: String, project_path: String) -> Result<(), String> {
    let full = resolve_project_relative_path(&project_path, &file_path)?;
    let full_str = full.to_string_lossy().to_string().replace('/', "\\");

    #[cfg(target_os = "windows")]
    {
        StdCommand::new("explorer")
            .arg(format!("/select,{}", full_str))
            .spawn()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        StdCommand::new("open")
            .args(["-R", &full.to_string_lossy()])
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = full.parent() {
            StdCommand::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to open file manager: {}", e))?;
        }
    }

    Ok(())
}

/// Read an image file and return its contents as a base64 data URL
#[tauri::command]
pub fn read_image_base64(file_path: String, project_path: String) -> Result<String, String> {
    let full = resolve_project_relative_path(&project_path, &file_path)?;
    // Limit to 10MB to avoid memory issues
    let metadata = std::fs::metadata(&full).map_err(|e| e.to_string())?;
    if metadata.len() > 10 * 1024 * 1024 {
        return Err("File too large for preview".to_string());
    }
    let ext = full
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "tga" => "image/tga",
        "tiff" | "tif" => "image/tiff",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };
    let data = std::fs::read(&full).map_err(|e| e.to_string())?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Run AI CLI analysis on a specific graph node (async with streaming)
#[tauri::command]
pub async fn run_ai_analysis(
    node_id: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (prompt, target_node_id, runtime_dir) = {
        let project_path = cloned_project_info(&state)?
            .map(|p| p.path)
            .unwrap_or_default();
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        let target_id = resolve_analysis_target_node_id(&node_id, &graph)
            .ok_or_else(|| format!("节点未找到: {}", node_id))?;
        let workdir = build_ai_runtime_dir()?;
        (
            build_ai_prompt(&project_path, &node_id, &target_id, &graph),
            target_id,
            workdir,
        )
    };

    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" {
        Some(runtime_dir_str.as_str())
    } else {
        None
    };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    emit_ai_log(&app, &format!("[快速分析] 请求节点: {}", node_id));
    emit_ai_log(&app, &format!("  实际分析文件: {}", target_node_id));
    emit_ai_log(
        &app,
        &format!("  CLI: {} | prompt长度: {} 字符", cli_name, prompt.len()),
    );

    let mut cmd = async_cli_command(&cli_name);
    cmd.current_dir(&runtime_dir);
    cmd.args(&invocation.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "无法运行 {}: {}。请确保已安装该 CLI 工具并加入 PATH。",
            cli_name, e
        )
    })?;

    if let Some(stdin_payload) = invocation.stdin_payload {
        if let Some(mut stdin_pipe) = child.stdin.take() {
            stdin_pipe
                .write_all(stdin_payload.as_bytes())
                .await
                .map_err(|e| format!("写入 stdin 失败: {}", e))?;
            drop(stdin_pipe); // close stdin to signal EOF
        }
    }

    emit_ai_log(&app, "  进程已启动，等待输出...");

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Read stdout and stderr CONCURRENTLY to avoid pipe deadlock
    let result_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));

    let stdout_handle = {
        let rl = result_lines.clone();
        let app2 = app.clone();
        tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    emit_ai_log(&app2, &line);
                    rl.lock().unwrap().push(line);
                }
            }
        })
    };

    let stderr_handle = {
        let sb = stderr_buf.clone();
        let app2 = app.clone();
        tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    emit_ai_log(&app2, &format!("  [stderr] {}", line));
                    let mut buf = sb.lock().unwrap();
                    buf.push_str(&line);
                    buf.push('\n');
                }
            }
        })
    };

    // Wait with timeout (5 minutes)
    let timeout = tokio::time::Duration::from_secs(300);
    match tokio::time::timeout(timeout, async {
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;
        child.wait().await
    })
    .await
    {
        Ok(Ok(status)) => {
            if status.success() {
                let result = result_lines.lock().unwrap().join("\n").trim().to_string();
                if result.is_empty() {
                    Err("AI CLI 返回了空结果".to_string())
                } else {
                    emit_ai_log(&app, "[快速分析] 分析完成");
                    Ok(result)
                }
            } else {
                let stderr_text = stderr_buf.lock().unwrap().clone();
                Err(format!("AI CLI 错误: {}", stderr_text.trim()))
            }
        }
        Ok(Err(e)) => Err(format!("等待进程结束失败: {}", e)),
        Err(_) => {
            let _ = child.kill().await;
            Err("AI CLI 超时（5分钟），已终止进程".to_string())
        }
    }
}

/// Run deep AI CLI analysis on a specific graph node (more detailed prompt with full context)
#[tauri::command]
pub async fn run_deep_ai_analysis(
    node_id: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let (prompt, target_node_id, runtime_dir) = {
        let project_path = cloned_project_info(&state)?
            .map(|p| p.path)
            .unwrap_or_default();
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        let target_id = resolve_analysis_target_node_id(&node_id, &graph)
            .ok_or_else(|| format!("节点未找到: {}", node_id))?;
        let workdir = build_ai_runtime_dir()?;
        (
            build_deep_ai_prompt(&project_path, &node_id, &target_id, &graph),
            target_id,
            workdir,
        )
    };

    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" {
        Some(runtime_dir_str.as_str())
    } else {
        None
    };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    emit_ai_log(&app, &format!("[深度分析] 请求节点: {}", node_id));
    emit_ai_log(&app, &format!("  实际分析文件: {}", target_node_id));
    emit_ai_log(
        &app,
        &format!("  CLI: {} | prompt长度: {} 字符", cli_name, prompt.len()),
    );

    let mut cmd = async_cli_command(&cli_name);
    cmd.current_dir(&runtime_dir);
    cmd.args(&invocation.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "无法运行 {}: {}。请确保已安装该 CLI 工具并加入 PATH。",
            cli_name, e
        )
    })?;

    if let Some(stdin_payload) = invocation.stdin_payload {
        if let Some(mut stdin_pipe) = child.stdin.take() {
            stdin_pipe
                .write_all(stdin_payload.as_bytes())
                .await
                .map_err(|e| format!("写入 stdin 失败: {}", e))?;
            drop(stdin_pipe);
        }
    }

    emit_ai_log(&app, "  进程已启动，等待输出...");

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Read stdout and stderr CONCURRENTLY to avoid pipe deadlock
    let result_lines = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));

    let stdout_handle = {
        let rl = result_lines.clone();
        let app2 = app.clone();
        tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    emit_ai_log(&app2, &line);
                    rl.lock().unwrap().push(line);
                }
            }
        })
    };

    let stderr_handle = {
        let sb = stderr_buf.clone();
        let app2 = app.clone();
        tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    emit_ai_log(&app2, &format!("  [stderr] {}", line));
                    let mut buf = sb.lock().unwrap();
                    buf.push_str(&line);
                    buf.push('\n');
                }
            }
        })
    };

    // Wait with timeout (10 minutes for deep analysis)
    let timeout = tokio::time::Duration::from_secs(600);
    match tokio::time::timeout(timeout, async {
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;
        child.wait().await
    })
    .await
    {
        Ok(Ok(status)) => {
            if status.success() {
                let result = result_lines.lock().unwrap().join("\n").trim().to_string();
                if result.is_empty() {
                    Err("AI CLI 返回了空结果".to_string())
                } else {
                    emit_ai_log(&app, "[深度分析] 分析完成");
                    Ok(result)
                }
            } else {
                let stderr_text = stderr_buf.lock().unwrap().clone();
                Err(format!("AI CLI 错误: {}", stderr_text.trim()))
            }
        }
        Ok(Err(e)) => Err(format!("等待进程结束失败: {}", e)),
        Err(_) => {
            let _ = child.kill().await;
            Err("AI CLI 超时（10分钟），已终止进程".to_string())
        }
    }
}

/// Check if a node is a code or resource file (skip .meta, directories, etc.)
fn is_analyzable_node(node: &GraphNode) -> bool {
    matches!(
        node.node_type,
        NodeType::CodeFile
            | NodeType::Asset
            | NodeType::Class
            | NodeType::Method
            | NodeType::Interface
            | NodeType::Module
            | NodeType::SceneObject
    )
}

fn build_ai_prompt(
    project_root: &str,
    requested_node_id: &str,
    target_node_id: &str,
    graph: &GraphStore,
) -> String {
    let file_node = match graph.get_node(target_node_id) {
        Some(n) => n,
        None => return format!("Analyze node: {}", target_node_id),
    };
    let requested_node = graph.get_node(requested_node_id).unwrap_or(file_node);

    let structure = build_file_structure_summary(target_node_id, graph);
    let method_flow = match try_load_text_source(project_root, target_node_id) {
        Ok(source) => build_method_static_flow_summary(target_node_id, &source, graph),
        Err(_) => "  文件内容不可读，无法提取方法级静态线索".to_string(),
    };
    let source_block = match try_load_text_source(project_root, target_node_id) {
        Ok(source) => format!(
            "```text\n{}\n```",
            format_source_with_line_numbers(&source, 35_000)
        ),
        Err(err) => format!("该文件不是可直接阅读的文本文件，或读取失败：{}", err),
    };

    let incoming = build_file_reference_summary(target_node_id, graph, true, Some(20));
    let outgoing = build_file_reference_summary(target_node_id, graph, false, Some(20));
    let suspected = build_suspected_summary(target_node_id, graph, Some(10));
    let hardcodes = build_hardcode_summary(target_node_id, graph, Some(10));
    let normalized_note = if requested_node_id != target_node_id {
        format!(
            "原始点击节点: {} ({:?})\n已自动收束为所属文件分析，禁止扩展到其他文件。\n",
            requested_node.id, requested_node.node_type
        )
    } else {
        String::new()
    };

    format!(
        "你是一个游戏项目单文件分析专家。你只能分析下面提供的这一个文件，不能读取其他文件，不能扫描目录，不能补充任何未提供的项目上下文。\n\
         重要约束:\n\
         - 只允许基于这里给出的单文件源码、文件结构摘要、静态引用摘要回答\n\
         - 如果某个函数、变量或外部调用用途无法从当前文件直接判断，明确写“静态数据不足”\n\
         - 输出必须始终围绕这个文件，不要写项目级总览\n\
         - 这是快速分析模式，请用简洁要点回答，每项控制在 1-3 句\n\n\
         === 分析范围 ===\n\
         请求节点: {} ({:?})\n\
         实际分析文件: {}\n\
         文件名: {}\n\
         文件类型: {:?}\n\
         资源类型: {}\n\
         {}\n\
         === 文件结构摘要 ===\n\
         {}\n\n\
         === 方法级静态线索（仅基于当前文件源码） ===\n\
         {}\n\n\
         === 文件级静态引用：谁引用这个文件 ===\n\
         {}\n\n\
         === 文件级静态引用：这个文件引用了谁 ===\n\
         {}\n\n\
         === 疑似动态引用 ===\n\
         {}\n\n\
         === 硬编码 ===\n\
         {}\n\n\
         === 单文件源码（仅此文件） ===\n\
         {}\n\n\
         请用中文完成快速分析，输出这 4 项：\n\
         1. 【文件职责】这个文件主要负责什么功能，属于什么模块\n\
         2. 【函数与变量概览】概括主要类、函数、成员变量分别在做什么，按重要度归纳，不要逐行解释\n\
         3. 【关键依赖】说明这个文件的主要输入、输出、依赖对象，以及最关键的静态引用关系\n\
         4. 【问题与建议】指出最值得注意的风险、歧义点或可落地优化项",
        requested_node.id,
        requested_node.node_type,
        target_node_id,
        file_node.name,
        file_node.node_type,
        file_node
            .asset_kind
            .as_ref()
            .map(|k| format!("{:?}", k))
            .unwrap_or_else(|| "无".to_string()),
        normalized_note,
        structure,
        method_flow,
        if incoming.is_empty() { "  无".to_string() } else { incoming.join("\n") },
        if outgoing.is_empty() { "  无".to_string() } else { outgoing.join("\n") },
        if suspected.is_empty() { "  无".to_string() } else { suspected.join("\n") },
        if hardcodes.is_empty() { "  无".to_string() } else { hardcodes.join("\n") },
        source_block
    )
}

/// Build a context prompt for a batch of files in a directory
fn build_batch_prompt(dir: &str, nodes_in_dir: &[&GraphNode], graph: &GraphStore) -> String {
    let mut prompt = format!(
        "你是一个游戏项目代码与资源分析专家。请只针对代码逻辑和资源引用关系进行快速概要分析，忽略 .meta 等引擎元数据文件。\n\
         ⚠ 重要约束：\n\
         - 绝对禁止使用文件系统命令（ls, find, cat, tree, read_file 等）扫描目录或读取其他文件\n\
         - 只分析下方提供的文件列表，不要扩展到其他文件或目录\n\
         - 所有分析所需数据已在下方完整提供，直接基于这些数据回答即可\n\
         ⚡ 这是快速批量分析模式，每个文件只需1-2句话概要，重点标注问题项。\n\n\
         === 分析目录: {} ===\n\n",
        dir
    );

    let mut word_count = 0usize;
    let max_words = 6000;

    for node in nodes_in_dir {
        if word_count > max_words {
            break;
        }

        // Per-node edges
        let incoming: Vec<String> = graph
            .edges
            .iter()
            .filter(|e| e.target == node.id)
            .take(10)
            .map(|e| {
                let src_name = e.source.rsplit('/').next().unwrap_or(&e.source);
                format!("    {} --[{:?}]--> {}", src_name, e.edge_type, node.name)
            })
            .collect();

        let outgoing: Vec<String> = graph
            .edges
            .iter()
            .filter(|e| e.source == node.id)
            .take(10)
            .map(|e| {
                let tgt_name = e.target.rsplit('/').next().unwrap_or(&e.target);
                format!("    {} --[{:?}]--> {}", node.name, e.edge_type, tgt_name)
            })
            .collect();

        let mut entry = format!(
            "【{}】类型: {:?}, 资源: {}\n",
            node.name,
            node.node_type,
            node.asset_kind
                .as_ref()
                .map(|k| format!("{:?}", k))
                .unwrap_or("无".to_string()),
        );

        if !incoming.is_empty() {
            entry.push_str(&format!(
                "  被引用({}):\n{}\n",
                incoming.len(),
                incoming.join("\n")
            ));
        }
        if !outgoing.is_empty() {
            entry.push_str(&format!(
                "  引用({}):\n{}\n",
                outgoing.len(),
                outgoing.join("\n")
            ));
        }

        // Suspected refs for this node
        let suspected: Vec<String> = graph
            .suspected_refs
            .iter()
            .filter(|sr| sr.code_location == node.id || sr.resource_path == node.id)
            .take(5)
            .map(|sr| {
                format!(
                    "    动态加载: {} → {} ({})",
                    sr.code_location
                        .rsplit('/')
                        .next()
                        .unwrap_or(&sr.code_location),
                    sr.resource_path,
                    sr.load_method
                )
            })
            .collect();
        if !suspected.is_empty() {
            entry.push_str(&format!(
                "  疑似引用({}):\n{}\n",
                suspected.len(),
                suspected.join("\n")
            ));
        }

        // Hardcodes for this node
        let hardcodes: Vec<String> = graph
            .hardcode_findings
            .iter()
            .filter(|h| h.file_path == node.id)
            .take(5)
            .map(|h| format!("    L{}: {:?} \"{}\"", h.line_number, h.category, h.value))
            .collect();
        if !hardcodes.is_empty() {
            entry.push_str(&format!(
                "  硬编码({}):\n{}\n",
                hardcodes.len(),
                hardcodes.join("\n")
            ));
        }

        word_count += entry.len() / 2;
        prompt.push_str(&entry);
        prompt.push('\n');
    }

    prompt.push_str("请用中文对每个文件逐一快速概要分析（每个文件1-2句话），包含：\n\
        1. 用途和职责\n\
        2. 标注有问题的项（未使用资源、循环依赖、硬编码等）\n\
        3. 关键优化建议\n\n\
        ⚠ 输出格式要求：必须严格按以下格式，每个文件用分隔符包裹，文件名必须与上面给出的【】中的名称完全一致：\n\
        ===【文件名1】===\n\
        分析内容...\n\
        ===【文件名2】===\n\
        分析内容...\n");

    prompt
}

/// Parse batch AI result into per-node summaries using ===【name】=== delimiters.
/// Returns a map from node name to its individual summary text.
fn parse_batch_result(result: &str, node_names: &[(&str, &str)]) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();

    // Try to split by ===【...】=== markers
    let parts: Vec<&str> = result.split("===【").collect();

    if parts.len() > 1 {
        // Successfully found delimiters
        for part in parts.iter().skip(1) {
            if let Some(close_idx) = part.find("】===") {
                let name = part[..close_idx].trim();
                let content = part[close_idx + "】===".len()..].trim();
                // Match against known node names
                for &(node_id, node_name) in node_names {
                    if node_name == name {
                        map.insert(node_id.to_string(), content.to_string());
                        break;
                    }
                }
            }
        }
    }

    // If parsing found nothing, store the whole result for all nodes as fallback
    if map.is_empty() {
        for &(node_id, _) in node_names {
            map.insert(node_id.to_string(), result.to_string());
        }
    }

    map
}

/// Build a deep analysis prompt for a specific node — more detailed than the quick version
fn build_deep_ai_prompt(
    project_root: &str,
    requested_node_id: &str,
    target_node_id: &str,
    graph: &GraphStore,
) -> String {
    let file_node = match graph.get_node(target_node_id) {
        Some(n) => n,
        None => return format!("Analyze node: {}", target_node_id),
    };
    let requested_node = graph.get_node(requested_node_id).unwrap_or(file_node);

    let structure = build_file_structure_summary(target_node_id, graph);
    let source_result = try_load_text_source(project_root, target_node_id);
    let method_flow = match &source_result {
        Ok(source) => build_method_static_flow_summary(target_node_id, source, graph),
        Err(_) => "  文件内容不可读，无法提取方法级静态线索".to_string(),
    };
    let source_block = match source_result {
        Ok(source) => format!(
            "```text\n{}\n```",
            format_source_with_line_numbers(&source, 70_000)
        ),
        Err(err) => format!("该文件不是可直接阅读的文本文件，或读取失败：{}", err),
    };

    let incoming = build_file_reference_summary(target_node_id, graph, true, None);
    let outgoing = build_file_reference_summary(target_node_id, graph, false, None);
    let suspected = build_suspected_summary(target_node_id, graph, None);
    let hardcodes = build_hardcode_summary(target_node_id, graph, None);
    let normalized_note = if requested_node_id != target_node_id {
        format!(
            "原始点击节点: {} ({:?})\n已自动收束为所属文件分析，禁止扩展到其他文件。\n",
            requested_node.id, requested_node.node_type
        )
    } else {
        String::new()
    };

    format!(
        "你是一个资深游戏项目单文件架构分析专家。你只能分析下面给出的这个文件，不能读取其他文件，不能扫描目录，不能把回答扩展成全项目总结。\n\
         重要约束:\n\
         - 所有结论必须建立在当前文件源码、当前文件的静态结构、以及当前文件的静态引用链摘要上\n\
         - 如果跨文件的方法级信息无法由现有静态数据确定，必须明确写“静态数据不足”\n\
         - 允许引用其他文件名作为依赖链的一部分，但不允许假设那些文件的具体实现\n\n\
         === 深度分析范围 ===\n\
         请求节点: {} ({:?})\n\
         实际分析文件: {}\n\
         文件名: {}\n\
         文件类型: {:?}\n\
         资源类型: {}\n\
         {}\n\
         === 文件结构摘要 ===\n\
         {}\n\n\
         === 方法级静态线索（仅基于当前文件源码） ===\n\
         {}\n\n\
         === 文件级引用链：谁引用这个文件 ===\n\
         {}\n\n\
         === 文件级引用链：这个文件引用了谁 ===\n\
         {}\n\n\
         === 疑似动态引用 ===\n\
         {}\n\n\
         === 硬编码 ===\n\
         {}\n\n\
         === 单文件源码（仅此文件） ===\n\
         {}\n\n\
         请用中文进行深度分析，按以下 6 项输出：\n\
         1. 【文件定位】这个文件在模块中的职责、边界、核心功能\n\
         2. 【引用链分析】谁依赖它、它依赖谁，这些关系各自服务什么目的，是否存在耦合过重、边界不清或方向异常\n\
         3. 【逐函数分析】按主要函数逐个说明: 做什么、当前文件内被谁调用、调用了谁、关联哪些外部依赖线索、主要服务什么业务流程；无法确定的部分写“静态数据不足”\n\
         4. 【关键状态与变量】总结重要成员变量、配置值、缓存状态或资源句柄分别承担什么责任\n\
         5. 【动态引用与硬编码】评估动态加载和硬编码是否合理，哪些值得继续核实或抽取\n\
         6. 【影响面与建议】如果修改这个文件，最可能影响哪些调用链或功能区域，并给出具体可执行的重构建议",
        requested_node.id,
        requested_node.node_type,
        target_node_id,
        file_node.name,
        file_node.node_type,
        file_node
            .asset_kind
            .as_ref()
            .map(|k| format!("{:?}", k))
            .unwrap_or_else(|| "无".to_string()),
        normalized_note,
        structure,
        method_flow,
        if incoming.is_empty() { "  无".to_string() } else { incoming.join("\n") },
        if outgoing.is_empty() { "  无".to_string() } else { outgoing.join("\n") },
        if suspected.is_empty() { "  无".to_string() } else { suspected.join("\n") },
        if hardcodes.is_empty() { "  无".to_string() } else { hardcodes.join("\n") },
        source_block
    )
}

/// Run AI batch analysis on all scanned nodes grouped by directory (async with streaming)
#[tauri::command]
pub async fn run_ai_batch_analysis(
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let project_path = {
        let project = state.project.lock().map_err(|e| e.to_string())?;
        project.as_ref().map(|p| p.path.clone()).unwrap_or_default()
    };

    // Collect directory groups from graph, only including analyzable nodes (code + resources)
    let dir_batches: Vec<(String, Vec<String>)> = {
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        let mut dir_map: HashMap<String, Vec<String>> = HashMap::new();

        for node in graph.nodes.values() {
            if !is_analyzable_node(node) {
                continue;
            }
            let dir = node
                .id
                .rfind('/')
                .map(|i| node.id[..i].to_string())
                .unwrap_or_else(|| "root".to_string());
            dir_map.entry(dir).or_default().push(node.id.clone());
        }

        dir_map.into_iter().collect()
    };

    let total_batches = dir_batches.len() as u32;
    let mut analyzed = 0u32;
    // Windows cmd.exe has ~8191 char command line limit; cap nodes per CLI call
    const MAX_NODES_PER_BATCH: usize = 20;

    emit_ai_log(
        &app,
        &format!("开始 AI 批量分析，共 {} 个目录", total_batches),
    );

    for (idx, (dir, node_ids)) in dir_batches.iter().enumerate() {
        emit_progress(
            &app,
            "ai",
            &format!("batch_{}", idx),
            idx as u32,
            total_batches,
            &format!("AI 分析目录: {} ({}/{})", dir, idx + 1, total_batches),
        );
        emit_ai_log(
            &app,
            &format!(
                "\n━━━ [{}/{}] 分析目录: {} ━━━",
                idx + 1,
                total_batches,
                dir
            ),
        );

        // Split large directories into sub-batches to keep prompts within CLI limits
        let chunks: Vec<&[String]> = node_ids.chunks(MAX_NODES_PER_BATCH).collect();
        let num_chunks = chunks.len();
        if num_chunks > 1 {
            emit_ai_log(
                &app,
                &format!(
                    "  目录包含 {} 个节点，拆分为 {} 批",
                    node_ids.len(),
                    num_chunks
                ),
            );
        }

        let mut dir_success = false;

        for (chunk_idx, chunk_node_ids) in chunks.iter().enumerate() {
            // Build prompt under lock, then release lock for CLI invocation
            let (prompt, node_name_map) = {
                let graph = state.graph.lock().map_err(|e| e.to_string())?;
                let nodes_in_dir: Vec<&GraphNode> = chunk_node_ids
                    .iter()
                    .filter_map(|id| graph.get_node(id))
                    .collect();
                if num_chunks > 1 {
                    emit_ai_log(
                        &app,
                        &format!(
                            "  [批次 {}/{}] 处理 {} 个节点...",
                            chunk_idx + 1,
                            num_chunks,
                            nodes_in_dir.len()
                        ),
                    );
                } else {
                    emit_ai_log(&app, &format!("  目录包含 {} 个节点", nodes_in_dir.len()));
                }
                let p = build_batch_prompt(dir, &nodes_in_dir, &graph);
                let names: Vec<(String, String)> = nodes_in_dir
                    .iter()
                    .map(|n| (n.id.clone(), n.name.clone()))
                    .collect();
                (p, names)
            };

            let codex_abs_dir = if cli_name == "codex" {
                PathBuf::from(&project_path)
                    .join(dir)
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };
            let codex_cd = if !codex_abs_dir.is_empty() {
                Some(codex_abs_dir.as_str())
            } else {
                None
            };
            let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

            emit_ai_log(
                &app,
                &format!("  调用 {} CLI... (prompt: {} 字符)", cli_name, prompt.len()),
            );

            let result = match run_cli_streaming(&cli_name, &invocation, &app, &project_path).await
            {
                Ok(text) if !text.is_empty() => text,
                Ok(_) => {
                    emit_ai_log(&app, "  ⚠ CLI 返回空结果");
                    "未分析".to_string()
                }
                Err(e) => {
                    emit_ai_log(&app, &format!("  ✗ 失败: {}", e));
                    log::warn!("AI CLI failed for {}: {}", dir, e);
                    "未分析".to_string()
                }
            };

            // Parse per-node results and store in metadata
            {
                let name_refs: Vec<(&str, &str)> = node_name_map
                    .iter()
                    .map(|(id, name)| (id.as_str(), name.as_str()))
                    .collect();
                let per_node = parse_batch_result(&result, &name_refs);

                let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
                for node_id in *chunk_node_ids {
                    let summary = per_node
                        .get(node_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| "未分析".to_string());
                    if let Some(node) = graph.nodes.get_mut(node_id) {
                        node.metadata.insert("ai_summary".to_string(), summary);
                    }
                }
            }

            if result != "未分析" {
                dir_success = true;
                if num_chunks > 1 {
                    emit_ai_log(&app, &format!("  ✓ 批次 {} 完成", chunk_idx + 1));
                }
            }
        }

        if dir_success {
            analyzed += 1;
            emit_ai_log(&app, &format!("  ✓ 目录分析完成"));
        }
    }

    emit_ai_log(
        &app,
        &format!(
            "\n━━━ AI 分析完成，成功 {}/{} 个目录 ━━━",
            analyzed, total_batches
        ),
    );
    emit_progress(
        &app,
        "ai",
        "done",
        total_batches,
        total_batches,
        "AI 分析完成",
    );
    Ok(analyzed)
}

/// Run a CLI command asynchronously, streaming stdout/stderr lines as ai_log events.
async fn run_cli_streaming(
    cli_name: &str,
    invocation: &CliInvocation,
    app: &tauri::AppHandle,
    project_dir: &str,
) -> Result<String, String> {
    let mut cmd = async_cli_command(cli_name);
    if !project_dir.is_empty() {
        cmd.current_dir(project_dir);
    }
    cmd.args(&invocation.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "无法运行 {}: {}。请确保已安装该 CLI 工具并加入 PATH。",
            cli_name, e
        )
    })?;

    if let Some(stdin_payload) = invocation.stdin_payload.as_ref() {
        if let Some(mut stdin_pipe) = child.stdin.take() {
            stdin_pipe
                .write_all(stdin_payload.as_bytes())
                .await
                .map_err(|e| format!("写入 stdin 失败: {}", e))?;
            drop(stdin_pipe);
        }
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Collect stdout in a shared buffer while streaming lines
    let result_buf = Arc::new(Mutex::new(Vec::<String>::new()));

    // Spawn stdout reader task
    let stdout_handle = if let Some(out) = stdout {
        let app_clone = app.clone();
        let buf = Arc::clone(&result_buf);
        Some(tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                emit_ai_log(&app_clone, &format!("  {}", line));
                if let Ok(mut b) = buf.lock() {
                    b.push(line);
                }
            }
        }))
    } else {
        None
    };

    // Spawn stderr reader task
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let stderr_handle = if let Some(err) = stderr {
        let app_clone = app.clone();
        let buf = Arc::clone(&stderr_buf);
        Some(tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                emit_ai_log(&app_clone, &format!("  {}", line));
                if let Ok(mut b) = buf.lock() {
                    b.push_str(&line);
                    b.push('\n');
                }
            }
        }))
    } else {
        None
    };

    // Wait for both readers AND the child process
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等待进程结束失败: {}", e))?;

    if status.success() {
        let lines = result_buf.lock().map_err(|e| e.to_string())?;
        Ok(lines.join("\n").trim().to_string())
    } else {
        let err = stderr_buf.lock().map_err(|e| e.to_string())?;
        Err(format!(
            "CLI 返回错误 (exit {}): {}",
            status.code().unwrap_or(-1),
            err.trim()
        ))
    }
}

/// Export analysis results to .analytics/ directory
#[tauri::command]
pub fn export_analysis(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project_info = require_project_info(&state)?;
    let graph_snapshot = state.graph.lock().map_err(|e| e.to_string())?.clone();
    let frontend_graph = graph_snapshot.to_frontend_graph();
    let stats = graph_snapshot.stats.clone();
    let suspected_refs = graph_snapshot.suspected_refs.clone();
    let hardcode_findings = graph_snapshot.hardcode_findings.clone();

    let export_dir = PathBuf::from(&project_info.path).join(".analytics");
    std::fs::create_dir_all(&export_dir).map_err(|e| format!("创建导出目录失败: {}", e))?;

    emit_progress(&app, "export", "graph", 0, 4, "正在导出图谱数据...");

    // Export graph.json (full graph with ai_summary metadata)
    let graph_json = serde_json::to_string_pretty(&frontend_graph).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("graph.json"), &graph_json).map_err(|e| e.to_string())?;

    emit_progress(&app, "export", "stats", 1, 4, "正在导出统计数据...");

    // Export stats.json
    #[derive(serde::Serialize)]
    struct ExportStats {
        engine: String,
        project_name: String,
        timestamp: String,
        stats: AnalysisStats,
    }
    let export_stats = ExportStats {
        engine: format!("{:?}", project_info.engine),
        project_name: project_info.name.clone(),
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        stats,
    };
    let stats_json = serde_json::to_string_pretty(&export_stats).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("stats.json"), &stats_json).map_err(|e| e.to_string())?;

    emit_progress(&app, "export", "findings", 2, 4, "正在导出检测结果...");

    // Export suspected-refs.json
    let suspected_json = serde_json::to_string_pretty(&suspected_refs).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("suspected-refs.json"), &suspected_json)
        .map_err(|e| e.to_string())?;

    // Export hardcode-findings.json
    let hardcode_json =
        serde_json::to_string_pretty(&hardcode_findings).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("hardcode-findings.json"), &hardcode_json)
        .map_err(|e| e.to_string())?;

    emit_progress(&app, "export", "summary", 3, 4, "正在生成 Markdown 摘要...");

    // Export summary.md
    let mut md = String::new();
    md.push_str(&format!("# {} 项目分析报告\n\n", project_info.name));
    md.push_str(&format!("- **引擎**: {:?}\n", project_info.engine));
    md.push_str(&format!(
        "- **生成时间**: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    md.push_str(&format!("- **文件总数**: {}\n", graph_snapshot.stats.total_files));
    md.push_str(&format!("- **资源文件**: {}\n", graph_snapshot.stats.asset_count));
    md.push_str(&format!("- **脚本文件**: {}\n", graph_snapshot.stats.script_count));
    md.push_str(&format!("- **类**: {}\n", graph_snapshot.stats.class_count));
    md.push_str(&format!("- **方法**: {}\n", graph_snapshot.stats.method_count));
    md.push_str(&format!(
        "- **正式引用边**: {}\n",
        graph_snapshot.stats.official_edges
    ));
    md.push_str(&format!(
        "- **疑似引用**: {}\n",
        graph_snapshot.stats.suspected_count
    ));
    md.push_str(&format!(
        "- **硬编码检出**: {}\n\n",
        graph_snapshot.stats.hardcode_count
    ));

    // Collect AI summaries grouped by directory
    let mut dir_summaries: HashMap<String, Vec<String>> = HashMap::new();
    for node in graph_snapshot.nodes.values() {
        if let Some(summary) = node.metadata.get("ai_summary") {
            if summary != "未分析" {
                let dir = node
                    .id
                    .rfind('/')
                    .map(|i| node.id[..i].to_string())
                    .unwrap_or_else(|| "root".to_string());
                dir_summaries
                    .entry(dir)
                    .or_default()
                    .push(format!("### {}\n{}\n", node.name, summary));
            }
        }
    }

    if !dir_summaries.is_empty() {
        md.push_str("## AI 分析摘要\n\n");
        let mut dirs: Vec<_> = dir_summaries.keys().cloned().collect();
        dirs.sort();
        for dir in dirs {
            md.push_str(&format!("## 目录: {}\n\n", dir));
            // Only include first unique summary per directory (batches share the same result)
            if let Some(summaries) = dir_summaries.get(&dir) {
                if let Some(first) = summaries.first() {
                    md.push_str(first);
                    md.push('\n');
                }
            }
        }
    }

    // Unused resources (nodes with zero incoming edges, type Asset)
    let asset_nodes: Vec<&GraphNode> = graph_snapshot
        .nodes
        .values()
        .filter(|n| n.node_type == NodeType::Asset)
        .collect();
    let referenced_targets: std::collections::HashSet<&str> =
        graph_snapshot.edges.iter().map(|e| e.target.as_str()).collect();
    let unused: Vec<&&GraphNode> = asset_nodes
        .iter()
        .filter(|n| !referenced_targets.contains(n.id.as_str()))
        .collect();

    if !unused.is_empty() {
        md.push_str("## 可能未使用的资源\n\n");
        for node in &unused {
            md.push_str(&format!("- {} ({})\n", node.id, node.name));
        }
        md.push('\n');
    }

    std::fs::write(export_dir.join("summary.md"), &md).map_err(|e| e.to_string())?;

    emit_progress(&app, "export", "done", 4, 4, "导出完成");

    let export_path = export_dir.to_string_lossy().to_string();
    Ok(export_path)
}

// ======================== V2: Redundancy Commands ========================

#[tauri::command]
pub fn get_orphan_nodes(
    state: State<'_, AppState>,
) -> Result<Vec<OrphanReport>, String> {
    let project_path = PathBuf::from(require_project_path(&state)?);
    let graph = state.graph.lock().map_err(|e| e.to_string())?;

    Ok(analysis::detect_orphan_nodes(&graph, &project_path))
}

#[tauri::command]
pub fn get_duplicate_resources(
    state: State<'_, AppState>,
) -> Result<Vec<DuplicateGroup>, String> {
    let project_path = PathBuf::from(require_project_path(&state)?);
    let graph = state.graph.lock().map_err(|e| e.to_string())?;

    Ok(analysis::detect_duplicates(&graph, &project_path))
}

#[tauri::command]
pub fn get_hotspots(
    threshold: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<HotspotReport>, String> {
    let graph = state.graph.lock().map_err(|e| e.to_string())?;
    let t = threshold.unwrap_or(5);
    Ok(analysis::detect_hotspots(&graph, t))
}

// ======================== V2: Asset Metrics Commands ========================

#[tauri::command]
pub fn get_asset_metrics(
    state: State<'_, AppState>,
) -> Result<Vec<AssetMetrics>, String> {
    let project_path = PathBuf::from(require_project_path(&state)?);
    let graph = state.graph.lock().map_err(|e| e.to_string())?;

    Ok(asset_metrics::collect_asset_metrics(&graph, &project_path))
}

// ======================== V2: AI Review Commands ========================

fn parse_review_type(review_type: &str) -> Result<ReviewType, String> {
    match review_type {
        "line" => Ok(ReviewType::Line),
        "architecture" => Ok(ReviewType::Architecture),
        "performance" => Ok(ReviewType::Performance),
        _ => Err(format!("不支持的审查类型: {}", review_type)),
    }
}

fn build_code_review_request(
    graph: &GraphStore,
    project_path: &str,
    node_id: &str,
    review_type: &str,
    response_language: &str,
) -> Result<(String, String, String, ReviewType), String> {
    let node = graph
        .nodes
        .get(node_id)
        .ok_or_else(|| format!("节点未找到: {}", node_id))?;
    let fp = node.file_path.clone().ok_or("节点没有文件路径")?;

    let full_path =
        std::path::Path::new(project_path).join(fp.replace('/', std::path::MAIN_SEPARATOR_STR));
    let content = std::fs::read_to_string(&full_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let lang = ai_review::detect_language(&fp);
    let rt = parse_review_type(review_type)?;

    let prompt = match rt {
        ReviewType::Line => {
            ai_review::build_line_review_prompt(&fp, &content, lang, response_language)
        }
        ReviewType::Architecture => {
            let upstream: Vec<String> = graph
                .get_upstream(node_id)
                .iter()
                .filter_map(|e| graph.nodes.get(&e.source))
                .filter_map(|n| n.file_path.clone())
                .collect();
            let downstream: Vec<String> = graph
                .get_downstream(node_id)
                .iter()
                .filter_map(|e| graph.nodes.get(&e.target))
                .filter_map(|n| n.file_path.clone())
                .collect();
            ai_review::build_arch_review_prompt(
                &fp,
                &content,
                &upstream,
                &downstream,
                lang,
                response_language,
            )
        }
        ReviewType::Performance => {
            ai_review::build_perf_review_prompt(&fp, &content, lang, response_language)
        }
    };

    Ok((prompt, fp, node_id.to_string(), rt))
}

async fn execute_code_review_request(
    cli_name: &str,
    model: &Option<String>,
    thinking: &Option<String>,
    prompt: String,
    file_path: String,
    target_node_id: String,
    review_type: ReviewType,
    response_language: &str,
    app: &tauri::AppHandle,
) -> Result<ReviewResult, String> {
    let runtime_dir = build_ai_runtime_dir()?;
    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" {
        Some(runtime_dir_str.as_str())
    } else {
        None
    };
    let invocation = build_cli_invocation(cli_name, &prompt, model, thinking, codex_cd)?;

    emit_ai_log(
        app,
        &format!(
            "[代码审查] 类型: {:?} | 节点: {}",
            review_type, target_node_id
        ),
    );
    emit_ai_log(
        app,
        &format!("  CLI: {} | prompt长度: {} 字符", cli_name, prompt.len()),
    );

    let raw = run_cli_streaming(cli_name, &invocation, app, &runtime_dir.to_string_lossy())
        .await
        .map_err(|e| {
            emit_ai_log(app, &format!("[审查错误] {}", e));
            e
        })?;

    if raw.trim().is_empty() {
        let err = "AI CLI 返回了空结果".to_string();
        emit_ai_log(app, &format!("[审查错误] {}", err));
        return Err(err);
    }

    emit_ai_log(app, &format!("[审查完成] 响应长度: {} 字符", raw.len()));

    Ok(ai_review::parse_review_response(
        &raw,
        review_type,
        &file_path,
        &target_node_id,
        response_language,
    ))
}

#[tauri::command]
pub async fn run_ai_code_review(
    node_id: String,
    review_type: String,
    response_language: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ReviewResult, String> {
    let (prompt, file_path, target_node_id, rt) = {
        let project_path = cloned_project_info(&state)?
            .map(|p| p.path)
            .unwrap_or_default();
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        build_code_review_request(
            &graph,
            &project_path,
            &node_id,
            &review_type,
            &response_language,
        )?
    };

    execute_code_review_request(
        &cli_name,
        &model,
        &thinking,
        prompt,
        file_path,
        target_node_id,
        rt,
        &response_language,
        &app,
    )
    .await
}

#[tauri::command]
pub async fn run_ai_project_code_review(
    review_type: String,
    response_language: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ReviewResult>, String> {
    let (graph, project_path, mut code_file_ids) = {
        let project_path = require_project_path(&state)?;
        let graph = state.graph.lock().map_err(|e| e.to_string())?.clone();
        let mut code_file_ids: Vec<String> = graph
            .nodes
            .values()
            .filter(|n| n.node_type == NodeType::CodeFile)
            .map(|n| n.id.clone())
            .collect();
        code_file_ids.sort();
        (graph, project_path, code_file_ids)
    };

    if code_file_ids.is_empty() {
        return Err("当前项目没有可审查的代码文件".to_string());
    }

    emit_ai_log(
        &app,
        &format!(
            "[全量代码审查] 类型: {} | 文件数: {}",
            review_type,
            code_file_ids.len()
        ),
    );

    let total = code_file_ids.len();
    let mut results = Vec::new();
    let mut failed = 0usize;

    for (idx, file_node_id) in code_file_ids.drain(..).enumerate() {
        emit_ai_log(
            &app,
            &format!("[全量代码审查] 进度 {}/{}: {}", idx + 1, total, file_node_id),
        );

        let (prompt, file_path, target_node_id, rt) = match build_code_review_request(
            &graph,
            &project_path,
            &file_node_id,
            &review_type,
            &response_language,
        ) {
            Ok(req) => req,
            Err(err) => {
                failed += 1;
                emit_ai_log(&app, &format!("[全量代码审查] 跳过 {}: {}", file_node_id, err));
                continue;
            }
        };

        match execute_code_review_request(
            &cli_name,
            &model,
            &thinking,
            prompt,
            file_path,
            target_node_id,
            rt,
            &response_language,
            &app,
        )
        .await
        {
            Ok(result) => results.push(result),
            Err(err) => {
                failed += 1;
                emit_ai_log(
                    &app,
                    &format!("[全量代码审查] 文件失败 {}: {}", file_node_id, err),
                );
            }
        }
    }

    emit_ai_log(
        &app,
        &format!(
            "[全量代码审查] 完成，成功 {} 个，失败 {} 个",
            results.len(),
            failed
        ),
    );

    if results.is_empty() {
        Err("全量代码审查未生成任何结果，请检查 AI CLI 输出".to_string())
    } else {
        Ok(results)
    }
}

#[tauri::command]
pub async fn run_ai_asset_review(
    response_language: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ReviewResult, String> {
    let (prompt, runtime_dir) = {
        let project_path = PathBuf::from(require_project_path(&state)?);
        let graph = state.graph.lock().map_err(|e| e.to_string())?;

        let metrics = asset_metrics::collect_asset_metrics(&graph, &project_path);
        // Only send metrics with issues (poor/fair rating) to keep prompt size down
        let problem_metrics: Vec<&AssetMetrics> = metrics.iter()
            .filter(|m| {
                matches!(m.performance_rating.as_deref(), Some("poor") | Some("fair"))
            })
            .take(50)
            .collect();

        let metrics_json = serde_json::to_string_pretty(&problem_metrics)
            .map_err(|e| e.to_string())?;
        let prompt = ai_review::build_asset_optimization_prompt(&metrics_json, &response_language);
        let workdir = build_ai_runtime_dir()?;
        (prompt, workdir)
    };

    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" {
        Some(runtime_dir_str.as_str())
    } else {
        None
    };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    emit_ai_log(&app, &format!("[资源优化审查] CLI: {} | prompt长度: {} 字符", cli_name, prompt.len()));

    let raw = run_cli_streaming(
        &cli_name,
        &invocation,
        &app,
        &runtime_dir.to_string_lossy(),
    )
    .await
    .map_err(|e| {
        emit_ai_log(&app, &format!("[资源审查错误] {}", e));
        e
    })?;

    if raw.trim().is_empty() {
        let err = "AI CLI 返回了空结果".to_string();
        emit_ai_log(&app, &format!("[资源审查错误] {}", err));
        return Err(err);
    }

    emit_ai_log(&app, &format!("[资源审查完成] 响应长度: {} 字符", raw.len()));

    Ok(ai_review::parse_review_response(
        &raw,
        ReviewType::Line,
        "",
        "asset_review",
        &response_language,
    ))
}

// ======================== Profiler Commands ========================

#[tauri::command]
pub async fn discover_unity() -> Result<u16, String> {
    unity_connection::discover_unity().await
}

#[tauri::command]
pub async fn connect_unity(
    port: u16,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let alive = unity_connection::check_connection(port).await;
    if alive {
        if let Ok(mut p) = state.unity_port.lock() {
            *p = Some(port);
        }
    }
    Ok(alive)
}

#[tauri::command]
pub async fn get_unity_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let port = {
        state.unity_port.lock().map_err(|e| e.to_string())?.clone()
    };
    match port {
        Some(p) => {
            let connected = unity_connection::check_connection(p).await;
            if !connected {
                if let Ok(mut up) = state.unity_port.lock() {
                    *up = None;
                }
                return Ok(serde_json::json!({
                    "connected": false,
                    "port": null,
                    "editor_state": null,
                    "profiling": false
                }));
            }
            let editor_state = unity_connection::get_editor_state(p).await.ok();
            let profiling = state.profiler.is_active().await;
            Ok(serde_json::json!({
                "connected": true,
                "port": p,
                "editor_state": editor_state,
                "profiling": profiling
            }))
        }
        None => Ok(serde_json::json!({
            "connected": false,
            "port": null,
            "editor_state": null,
            "profiling": false
        })),
    }
}

#[tauri::command]
pub async fn disconnect_unity(
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Ok(mut p) = state.unity_port.lock() {
        *p = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_profiling(
    session_name: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let port = {
        state.unity_port.lock().map_err(|e| e.to_string())?
            .ok_or("未连接 Unity")?
    };

    if !unity_connection::check_connection(port).await {
        if let Ok(mut p) = state.unity_port.lock() {
            *p = None;
        }
        return Err("Unity 连接已断开，请重新连接".to_string());
    }

    let editor_state = unity_connection::get_editor_state(port).await
        .map_err(|e| format!("读取 Unity 编辑器状态失败: {}", e))?;
    if !editor_state.is_playing {
        return Err("请先在 Unity 中进入 Play 模式，再开始采集".to_string());
    }

    state.profiler.start_session(port, session_name, app).await
}

#[tauri::command]
pub async fn stop_profiling(
    state: State<'_, AppState>,
) -> Result<profiler_session::SessionMeta, String> {
    let session = state.profiler.stop_session().await?;

    // Save session to disk
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    profiler_session::save_session(&project_path, &session)?;

    Ok(profiler_session::SessionMeta::from(&session))
}

#[tauri::command]
pub async fn list_profiler_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<profiler_session::SessionMeta>, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    Ok(profiler_session::list_sessions(&project_path))
}

#[tauri::command]
pub async fn get_profiler_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<profiler_session::ProfilerSession, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    profiler_session::load_session(&project_path, &session_id)
}

#[tauri::command]
pub async fn delete_profiler_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    profiler_session::delete_session(&project_path, &session_id)
}

#[tauri::command]
pub async fn rename_profiler_session(
    session_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    profiler_session::rename_session(&project_path, &session_id, &new_name)
}

#[tauri::command]
pub async fn generate_profiler_report(
    session_id: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<profiler_report::ProfilerReport, String> {
    let (session, response_language) = {
        let project_path = require_project_path(&state)?;
        let settings = load_settings_from_app(&app)?;
        let session = profiler_session::load_session(&project_path, &session_id)?;
        (session, settings.language)
    };

    let prompt = profiler_report::build_profiler_prompt(&session, &response_language);
    let runtime_dir = build_ai_runtime_dir()?;
    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" { Some(runtime_dir_str.as_str()) } else { None };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    emit_ai_log(&app, &format!("[性能分析] 会话: {} | CLI: {}", session.name, cli_name));

    let raw = run_cli_streaming(&cli_name, &invocation, &app, &runtime_dir_str).await?;

    if raw.trim().is_empty() {
        return Err("AI CLI 返回了空结果".to_string());
    }

    // Parse JSON response
    let parsed: serde_json::Value = extract_json_from_response(&raw)
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or(serde_json::json!({
            "health_score": 50,
            "summary": raw.clone(),
            "findings": [],
            "optimization_plan": ""
        }));

    let report = profiler_report::ProfilerReport {
        session_id: session_id.clone(),
        health_score: parsed.get("health_score").and_then(|v| v.as_u64()).unwrap_or(50) as u32,
        summary: parsed.get("summary").and_then(|v| v.as_str()).unwrap_or(&raw).to_string(),
        findings: parsed.get("findings")
            .and_then(|v| serde_json::from_value::<Vec<profiler_report::ProfilerFinding>>(v.clone()).ok())
            .unwrap_or_default(),
        optimization_plan: parsed.get("optimization_plan").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        raw_response: raw,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    emit_ai_log(&app, &format!("[性能分析完成] 健康评分: {}/100", report.health_score));
    Ok(report)
}

#[tauri::command]
pub async fn generate_deep_profiler_analysis(
    session_id: String,
    file_paths: Vec<String>,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<profiler_report::DeepAnalysisReport, String> {
    let (session, project_path, response_language) = {
        let project_path = require_project_path(&state)?;
        let settings = load_settings_from_app(&app)?;
        let session = profiler_session::load_session(&project_path, &session_id)?;
        (session, project_path, settings.language)
    };

    // Read source files
    let mut source_files = Vec::new();
    for fp in &file_paths {
        let abs = PathBuf::from(&project_path).join(fp);
        if let Ok(content) = std::fs::read_to_string(&abs) {
            source_files.push((fp.clone(), content));
        }
    }

    if source_files.is_empty() {
        return Err("未找到可分析的源文件".to_string());
    }

    let prompt = profiler_report::build_deep_analysis_prompt(&session, &source_files, &response_language);
    let runtime_dir = build_ai_runtime_dir()?;
    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" { Some(runtime_dir_str.as_str()) } else { None };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    emit_ai_log(&app, &format!("[深度性能分析] 会话: {} | 文件: {} 个", session.name, source_files.len()));

    let raw = run_cli_streaming(&cli_name, &invocation, &app, &runtime_dir_str).await?;

    if raw.trim().is_empty() {
        return Err("AI CLI 返回了空结果".to_string());
    }

    let parsed: serde_json::Value = extract_json_from_response(&raw)
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or(serde_json::json!({
            "summary": raw.clone(),
            "source_findings": []
        }));

    let report = profiler_report::DeepAnalysisReport {
        session_id: session_id.clone(),
        summary: parsed.get("summary").and_then(|v| v.as_str()).unwrap_or(&raw).to_string(),
        source_findings: parsed.get("source_findings")
            .and_then(|v| serde_json::from_value::<Vec<profiler_report::SourceFinding>>(v.clone()).ok())
            .unwrap_or_default(),
        raw_response: raw,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    emit_ai_log(&app, &format!("[深度分析完成] 发现 {} 个问题", report.source_findings.len()));
    Ok(report)
}

#[tauri::command]
pub async fn compare_profiler_sessions(
    session_a_id: String,
    session_b_id: String,
    state: State<'_, AppState>,
) -> Result<profiler_report::ComparisonResult, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    let a = profiler_session::load_session(&project_path, &session_a_id)?;
    let b = profiler_session::load_session(&project_path, &session_b_id)?;
    Ok(profiler_report::compare_sessions(&a, &b))
}

#[tauri::command]
pub async fn export_profiler_report(
    session_id: String,
    report: profiler_report::ProfilerReport,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    let session = profiler_session::load_session(&project_path, &session_id)?;
    let md = profiler_report::export_report_markdown(&report, &session.name);

    let export_dir = PathBuf::from(&project_path).join(".analytics").join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
    let filename = format!("profiler_report_{}.md", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    let export_path = export_dir.join(&filename);
    std::fs::write(&export_path, &md).map_err(|e| e.to_string())?;

    Ok(export_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn export_profiler_comparison(
    result: profiler_report::ComparisonResult,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    let md = profiler_report::export_comparison_markdown(&result);

    let export_dir = PathBuf::from(&project_path).join(".analytics").join("exports");
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
    let filename = format!("profiler_comparison_{}.md", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    let export_path = export_dir.join(&filename);
    std::fs::write(&export_path, &md).map_err(|e| e.to_string())?;

    Ok(export_path.to_string_lossy().to_string())
}

fn extract_json_from_response(raw: &str) -> Option<String> {
    // Try to extract JSON block from markdown code fence
    if let Some(start) = raw.find("```json") {
        let after = &raw[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    // Try to find raw JSON object
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            if end > start {
                return Some(raw[start..=end].to_string());
            }
        }
    }
    None
}

// ======================== Device Profiler Commands ========================

#[tauri::command]
pub async fn discover_devices(port: Option<u16>) -> Result<Vec<device_transfer::DiscoveredDevice>, String> {
    Ok(device_transfer::discover_devices(port).await)
}

#[tauri::command]
pub async fn get_device_status(ip: String, port: u16) -> Result<device_transfer::DeviceStatus, String> {
    device_transfer::get_device_status(&ip, port).await
}

#[tauri::command]
pub async fn list_device_sessions(ip: String, port: u16) -> Result<Vec<device_transfer::RemoteSession>, String> {
    device_transfer::list_device_sessions(&ip, port).await
}

#[tauri::command]
pub async fn download_device_session(
    ip: String,
    port: u16,
    file_name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let save_dir = {
        let project = state.project.lock().map_err(|e| e.to_string())?;
        if let Some(project_info) = project.as_ref() {
            PathBuf::from(&project_info.path)
                .join(".analytics")
                .join("device_profiles")
        } else {
            std::env::temp_dir()
                .join("GameAnalytics")
                .join("device_profiles")
        }
    };
    std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
    device_transfer::download_session(&ip, port, &file_name, &save_dir.to_string_lossy()).await
}

#[tauri::command]
pub async fn remote_start_capture(
    ip: String,
    port: u16,
    session_name: Option<String>,
) -> Result<(), String> {
    device_transfer::remote_start_capture(&ip, port, session_name).await
}

#[tauri::command]
pub async fn remote_stop_capture(
    ip: String,
    port: u16,
) -> Result<device_transfer::RemoteStopCaptureResult, String> {
    device_transfer::remote_stop_capture(&ip, port).await
}

#[tauri::command]
pub async fn import_gaprof_file(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project_path = {
        state.project.lock().map_err(|e| e.to_string())?
            .as_ref().map(|p| p.path.clone())
            .ok_or("未打开项目")?
    };
    let save_dir = PathBuf::from(&project_path)
        .join(".analytics")
        .join("device_profiles");
    std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;

    let src_path = PathBuf::from(&file_path);
    let file_name = src_path.file_name()
        .ok_or("Invalid file path")?
        .to_string_lossy()
        .to_string();
    let dest = save_dir.join(&file_name);
    std::fs::copy(&src_path, &dest).map_err(|e| e.to_string())?;

    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn parse_gaprof_session(file_path: String) -> Result<device_profile::DeviceProfileReport, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let session_name = PathBuf::from(&file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());

    let mut report = device_profile::generate_report(&session, &session_name);
    report.source_file_path = Some(file_path);
    Ok(report)
}

#[tauri::command]
pub async fn generate_device_report(
    file_path: String,
    _app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<device_profile::DeviceProfileReport, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let session_name = PathBuf::from(&file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());

    let mut report = device_profile::generate_report(&session, &session_name);
    report.source_file_path = Some(file_path);
    Ok(report)
}

#[tauri::command]
pub async fn get_device_screenshot(
    file_path: String,
    frame_index: u32,
) -> Result<String, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let screenshot = session.screenshots.iter()
        .find(|s| s.frame_index == frame_index)
        .ok_or("Screenshot not found for this frame")?;

    // Return as base64 encoded JPEG
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &screenshot.jpeg_data))
}

#[tauri::command]
pub async fn export_device_report(
    report: device_profile::DeviceProfileReport,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let md = device_profile::export_device_report_markdown(&report);
    let export_dir = {
        let project_guard = state.project.lock().map_err(|e| e.to_string())?;
        if let Some(project) = project_guard.as_ref() {
            PathBuf::from(&project.path).join(".analytics").join("exports")
        } else if let Some(source_file) = report.source_file_path.as_deref() {
            PathBuf::from(source_file)
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or("无法确定导出目录")?
        } else {
            return Err("未打开项目，且报告没有来源文件路径，无法导出".to_string());
        }
    };
    std::fs::create_dir_all(&export_dir).map_err(|e| e.to_string())?;
    let filename = format!("device_profile_{}.md", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    let export_path = export_dir.join(&filename);
    std::fs::write(&export_path, &md).map_err(|e| e.to_string())?;

    Ok(export_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_frame_functions(
    file_path: String,
    frame_index: u32,
) -> Result<Option<device_profile::PerFrameFunctions>, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let idx = frame_index as usize;
    if idx >= session.function_samples.len() {
        return Ok(None);
    }
    let samples = &session.function_samples[idx];
    if samples.is_empty() {
        return Ok(None);
    }

    let functions: Vec<device_profile::PerFrameFunction> = samples.iter().map(|s| {
        let name = session.string_table.get(s.function_name_index as usize)
            .cloned()
            .unwrap_or_else(|| format!("Function_{}", s.function_name_index));
        device_profile::PerFrameFunction {
            name,
            category: s.category.label().to_string(),
            self_ms: s.self_time_ms,
            total_ms: s.total_time_ms,
            call_count: s.call_count,
            depth: s.depth,
            parent_index: s.parent_index,
        }
    }).collect();

    Ok(Some(device_profile::PerFrameFunctions { frame_index, functions }))
}

#[tauri::command]
pub async fn get_session_logs(
    file_path: String,
    log_type_filter: Option<u8>,
    limit: Option<usize>,
) -> Result<Vec<device_profile::LogEntry>, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let max = limit.unwrap_or(500);
    let entries: Vec<device_profile::LogEntry> = session.log_entries.into_iter()
        .filter(|l| log_type_filter.map_or(true, |t| l.log_type == t))
        .take(max)
        .collect();

    Ok(entries)
}

/// AI-powered device profiling analysis that combines profiler data with code graph
#[tauri::command]
pub async fn run_ai_device_analysis(
    file_path: String,
    cli_name: String,
    model: Option<String>,
    thinking: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DeviceAiAnalysis, String> {
    let data = std::fs::read(&file_path).map_err(|e| format!("Read file: {}", e))?;
    let session = device_profile::parse_gaprof(&data)?;

    let session_name = PathBuf::from(&file_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());

    let report = device_profile::generate_report(&session, &session_name);

    // Build the base prompt from profiler data
    let mut prompt = device_profile::build_ai_prompt(&report);

    // Enrich with code graph context if available
    let graph_context = {
        let project = cloned_project_info(&state)?;
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        if let Some(project) = project.as_ref() {
            build_code_graph_context(&graph, &project.path, &report)
        } else {
            String::new()
        }
    };

    if !graph_context.is_empty() {
        prompt.push_str("\n\n### Code Graph Context (Static Analysis)\n");
        prompt.push_str("The following is the relevant code dependency information from static analysis of the project:\n\n");
        prompt.push_str(&graph_context);
        prompt.push_str("\nPlease cross-reference the profiler bottlenecks with the code graph to identify specific optimization targets.\n");
    }

    emit_ai_log(&app, &format!("[设备性能AI分析] 文件: {} | prompt长度: {} 字符", session_name, prompt.len()));

    let runtime_dir = build_ai_runtime_dir()?;
    let runtime_dir_str = runtime_dir.to_string_lossy().to_string();
    let codex_cd = if cli_name == "codex" { Some(runtime_dir_str.as_str()) } else { None };
    let invocation = build_cli_invocation(&cli_name, &prompt, &model, &thinking, codex_cd)?;

    let raw = run_cli_streaming(&cli_name, &invocation, &app, &runtime_dir.to_string_lossy())
        .await
        .map_err(|e| {
            emit_ai_log(&app, &format!("[设备性能AI分析错误] {}", e));
            e
        })?;

    if raw.trim().is_empty() {
        return Err("AI CLI returned empty result".to_string());
    }

    emit_ai_log(&app, &format!("[设备性能AI分析完成] 响应长度: {} 字符", raw.len()));

    Ok(DeviceAiAnalysis {
        session_name: session_name.clone(),
        overall_grade: report.overall_grade.clone(),
        analysis: raw,
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAiAnalysis {
    pub session_name: String,
    pub overall_grade: String,
    pub analysis: String,
    pub timestamp: String,
}

/// Build code graph context for the AI prompt based on profiler bottlenecks
fn build_code_graph_context(
    graph: &crate::graph::store::GraphStore,
    project_path: &str,
    report: &device_profile::DeviceProfileReport,
) -> String {
    use crate::graph::model::NodeType;
    let mut context = String::new();

    // Find script files that could be related to the bottleneck module
    let bottleneck = &report.module_analysis.bottleneck;
    let code_files: Vec<_> = graph.nodes.values()
        .filter(|n| n.node_type == NodeType::CodeFile)
        .collect();

    if code_files.is_empty() {
        return context;
    }

    context.push_str(&format!("Project: {}\n", project_path));
    context.push_str(&format!("Total scripts analyzed: {}\n", code_files.len()));
    context.push_str(&format!("Bottleneck module: {}\n\n", bottleneck));

    // Find MonoBehaviour classes (they run in Update/LateUpdate/FixedUpdate)
    let mono_classes: Vec<_> = graph.nodes.values()
        .filter(|n| n.node_type == NodeType::Class)
        .filter(|n| {
            // Check if this class has edges indicating MonoBehaviour inheritance
            graph.edges.iter().any(|e| {
                e.target == n.id && matches!(e.edge_type, crate::graph::model::EdgeType::Inherits)
            })
        })
        .collect();

    if !mono_classes.is_empty() {
        context.push_str(&format!("MonoBehaviour-derived classes ({}):\n", mono_classes.len()));
        for cls in mono_classes.iter().take(30) {
            if let Some(path) = cls.file_path.as_deref() {
                context.push_str(&format!("  - {} ({})\n", cls.name, path));
            } else {
                context.push_str(&format!("  - {}\n", cls.name));
            }
        }
        context.push('\n');
    }

    // Find hotspot files (highest in-degree)
    let mut file_degrees: Vec<(&str, usize)> = code_files.iter()
        .map(|n| {
            let degree = graph.edges.iter().filter(|e| e.target == n.id).count();
            (n.name.as_str(), degree)
        })
        .collect();
    file_degrees.sort_by(|a, b| b.1.cmp(&a.1));

    if !file_degrees.is_empty() {
        context.push_str("Top dependency hotspot scripts (most depended upon):\n");
        for (name, degree) in file_degrees.iter().take(15) {
            context.push_str(&format!("  - {} ({} dependents)\n", name, degree));
        }
        context.push('\n');
    }

    // If function analysis exists, try to map function names to code files
    if let Some(fa) = &report.function_analysis {
        let script_functions: Vec<_> = fa.top_functions.iter()
            .filter(|f| f.category == "用户脚本")
            .take(10)
            .collect();

        if !script_functions.is_empty() {
            context.push_str("Top user script functions (from profiler):\n");
            for f in &script_functions {
                context.push_str(&format!("  - {} (self={:.3}ms, calls={:.1}/frame)\n",
                    f.name, f.avg_self_ms, f.avg_call_count));
            }
            context.push('\n');
        }
    }

    context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_settings_normalizes_scope_and_empty_values() {
        let settings = AppSettings {
            ai_cli: "  ".to_string(),
            language: "en-US".to_string(),
            scan_scope: "custom".to_string(),
            hardcode_enabled: false,
            suspected_enabled: true,
            ai_model: Some("  gpt-5.4  ".to_string()),
            ai_thinking: Some(" HIGH ".to_string()),
        };

        let sanitized = sanitize_settings(settings);
        assert_eq!(sanitized.ai_cli, "claude");
        assert_eq!(sanitized.language, "en");
        assert_eq!(sanitized.scan_scope, "full");
        assert_eq!(sanitized.ai_model.as_deref(), Some("gpt-5.4"));
        assert_eq!(sanitized.ai_thinking.as_deref(), Some("high"));
        assert!(!sanitized.hardcode_enabled);
        assert!(sanitized.suspected_enabled);
    }

    #[test]
    fn resolve_project_relative_path_rejects_parent_segments() {
        let root = std::env::temp_dir().join(format!(
            "ga-path-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("Assets")).unwrap();

        let result = resolve_project_relative_path(root.to_str().unwrap(), "../secret.txt");
        assert!(result.is_err());

        std::fs::remove_dir_all(root).unwrap();
    }
}
