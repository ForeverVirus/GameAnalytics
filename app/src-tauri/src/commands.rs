use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;

use crate::analysis;
use crate::graph::model::*;
use crate::graph::store::{FrontendGraph, GraphStore};
use crate::workspace;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Application state shared across commands
pub struct AppState {
    pub project: Mutex<Option<workspace::ProjectInfo>>,
    pub graph: Mutex<GraphStore>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project: Mutex::new(None),
            graph: Mutex::new(GraphStore::new()),
        }
    }
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

    let mut project = state.project.lock().map_err(|e| e.to_string())?;
    *project = Some(info.clone());

    // Try to load cached analysis, otherwise reset graph
    let mut graph = state.graph.lock().map_err(|e| e.to_string())?;
    let cache_path = project_path.join(".analytics").join("cache.json");
    if cache_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str::<GraphStore>(&data) {
                log::info!("Loaded cached analysis from {}", cache_path.display());
                *graph = cached;
                return Ok(info);
            }
        }
    }
    *graph = GraphStore::new();

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
        Ok(cache_path.exists())
    } else {
        Ok(false)
    }
}

/// Save current analysis state to cache
#[tauri::command]
pub fn save_analysis_cache(state: State<'_, AppState>) -> Result<(), String> {
    let project = state.project.lock().map_err(|e| e.to_string())?;
    let project_info = project.as_ref().ok_or("No project selected")?;
    let graph = state.graph.lock().map_err(|e| e.to_string())?;

    let cache_dir = PathBuf::from(&project_info.path).join(".analytics");
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("创建缓存目录失败: {}", e))?;

    let cache_json = serde_json::to_string(&*graph).map_err(|e| e.to_string())?;
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
    {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let findings = tokio::task::spawn_blocking(move || analysis::detect_hardcodes(&pp, &cf))
            .await
            .map_err(|e| e.to_string())?;

        for f in findings {
            graph.add_hardcode_finding(f);
        }
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
    {
        let pp = project_path.clone();
        let cf = code_files.clone();
        let refs =
            tokio::task::spawn_blocking(move || analysis::detect_suspected_references(&pp, &cf))
                .await
                .map_err(|e| e.to_string())?;

        for r in refs {
            graph.add_suspected_ref(r);
        }
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
        "anim" | "controller" | "overrideController" => {
            (NodeType::Asset, Some(AssetKind::Animation))
        }
        "fbx" | "obj" | "blend" | "glb" | "gltf" => (NodeType::Asset, Some(AssetKind::Data)),
        _ => (NodeType::Asset, Some(AssetKind::Other)),
    }
}

/// Save application settings
#[tauri::command]
pub fn save_settings(settings: AppSettings, app: tauri::AppHandle) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("settings.json");
    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Load application settings
#[tauri::command]
pub fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = dir.join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).map_err(|e| e.to_string()),
        Err(_) => Ok(AppSettings::default()),
    }
}

/// Open file location in system file explorer
#[tauri::command]
pub fn open_file_location(file_path: String, project_path: String) -> Result<(), String> {
    let full = PathBuf::from(&project_path).join(&file_path);
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
    let full = PathBuf::from(&project_path).join(&file_path);
    if !full.exists() {
        return Err("File not found".to_string());
    }
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
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        let project = state.project.lock().map_err(|e| e.to_string())?;
        let project_path = project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
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
        let graph = state.graph.lock().map_err(|e| e.to_string())?;
        let project = state.project.lock().map_err(|e| e.to_string())?;
        let project_path = project.as_ref().map(|p| p.path.clone()).unwrap_or_default();
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
    let project = state.project.lock().map_err(|e| e.to_string())?;
    let project_info = project.as_ref().ok_or("No project selected")?;
    let graph = state.graph.lock().map_err(|e| e.to_string())?;

    let export_dir = PathBuf::from(&project_info.path).join(".analytics");
    std::fs::create_dir_all(&export_dir).map_err(|e| format!("创建导出目录失败: {}", e))?;

    emit_progress(&app, "export", "graph", 0, 4, "正在导出图谱数据...");

    // Export graph.json (full graph with ai_summary metadata)
    let frontend_graph = graph.to_frontend_graph();
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
        stats: graph.stats.clone(),
    };
    let stats_json = serde_json::to_string_pretty(&export_stats).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("stats.json"), &stats_json).map_err(|e| e.to_string())?;

    emit_progress(&app, "export", "findings", 2, 4, "正在导出检测结果...");

    // Export suspected-refs.json
    let suspected_json =
        serde_json::to_string_pretty(&graph.suspected_refs).map_err(|e| e.to_string())?;
    std::fs::write(export_dir.join("suspected-refs.json"), &suspected_json)
        .map_err(|e| e.to_string())?;

    // Export hardcode-findings.json
    let hardcode_json =
        serde_json::to_string_pretty(&graph.hardcode_findings).map_err(|e| e.to_string())?;
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
    md.push_str(&format!("- **文件总数**: {}\n", graph.stats.total_files));
    md.push_str(&format!("- **资源文件**: {}\n", graph.stats.asset_count));
    md.push_str(&format!("- **脚本文件**: {}\n", graph.stats.script_count));
    md.push_str(&format!("- **类**: {}\n", graph.stats.class_count));
    md.push_str(&format!("- **方法**: {}\n", graph.stats.method_count));
    md.push_str(&format!(
        "- **正式引用边**: {}\n",
        graph.stats.official_edges
    ));
    md.push_str(&format!(
        "- **疑似引用**: {}\n",
        graph.stats.suspected_count
    ));
    md.push_str(&format!(
        "- **硬编码检出**: {}\n\n",
        graph.stats.hardcode_count
    ));

    // Collect AI summaries grouped by directory
    let mut dir_summaries: HashMap<String, Vec<String>> = HashMap::new();
    for node in graph.nodes.values() {
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
    let asset_nodes: Vec<&GraphNode> = graph
        .nodes
        .values()
        .filter(|n| n.node_type == NodeType::Asset)
        .collect();
    let referenced_targets: std::collections::HashSet<&str> =
        graph.edges.iter().map(|e| e.target.as_str()).collect();
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
