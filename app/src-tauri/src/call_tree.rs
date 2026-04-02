// Call Tree Reconstruction Module
// Builds hierarchical call trees from flat FunctionSample data,
// supporting forward (caller→callees) and reverse (callee→callers) analysis.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::device_profile::{FunctionCategory, FunctionSample, GaprofSession};

// ======================== Data Structures ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeNode {
    pub name: String,
    pub category: String,
    pub avg_self_ms: f32,
    pub total_self_ms: f32,
    pub self_pct: f32,
    pub avg_total_ms: f32,
    pub total_total_ms: f32,
    pub total_pct: f32,
    pub call_count: u64,
    pub avg_call_count: f32,
    pub frames_called: u32,
    pub calls_per_frame: f32,
    pub depth: u8,
    pub children: Vec<CallTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSearchResult {
    pub name: String,
    pub category: String,
    pub avg_self_ms: f32,
    pub total_self_ms: f32,
    pub frames_called: u32,
    pub avg_call_count: f32,
}

// ======================== Call Tree Builder ========================

/// Build an aggregated call tree for a specific module/category across all sampled frames.
/// `direction`: "forward" builds caller→callees tree; "reverse" builds callee→callers tree.
/// `category_filter`: optionally filter to functions of a specific category.
/// `frame_start`/`frame_end`: optional frame range (inclusive).
pub fn build_call_tree(
    session: &GaprofSession,
    category_filter: Option<FunctionCategory>,
    frame_start: Option<u32>,
    frame_end: Option<u32>,
    direction: &str,
    top_n: Option<usize>,
) -> Vec<CallTreeNode> {
    let start = frame_start.unwrap_or(0) as usize;
    let end = frame_end.map(|e| e as usize + 1).unwrap_or(session.function_samples.len());
    let end = end.min(session.function_samples.len());

    if start >= end || session.function_samples.is_empty() {
        return Vec::new();
    }

    let sampled_frames = &session.function_samples[start..end];
    let sampled_count = sampled_frames.iter().filter(|f| !f.is_empty()).count() as u32;
    if sampled_count == 0 {
        return Vec::new();
    }

    match direction {
        "reverse" => build_reverse_tree(session, sampled_frames, sampled_count, category_filter, top_n.unwrap_or(30)),
        _ => build_forward_tree(session, sampled_frames, sampled_count, category_filter, top_n.unwrap_or(30)),
    }
}

/// Forward call tree: root functions at top, children below.
/// Aggregates across all frames — each function tracked by unique name index.
fn build_forward_tree(
    session: &GaprofSession,
    sampled_frames: &[Vec<FunctionSample>],
    sampled_count: u32,
    category_filter: Option<FunctionCategory>,
    top_n: usize,
) -> Vec<CallTreeNode> {
    // Aggregate stats per (name_index, parent_name_index) pair
    struct FuncAccum {
        self_time_sum: f64,
        total_time_sum: f64,
        call_count_sum: u64,
        frames_set: std::collections::HashSet<usize>,
        category: FunctionCategory,
        depth: u8,
    }

    // key = (function_name_index, parent_function_name_index_or_-1_for_root)
    let mut accum: HashMap<(u16, i32), FuncAccum> = HashMap::new();

    for (frame_idx, frame_samples) in sampled_frames.iter().enumerate() {
        for (sample_idx, s) in frame_samples.iter().enumerate() {
            if let Some(cat) = &category_filter {
                if s.category != *cat {
                    continue;
                }
            }

            let parent_name: i32 = if s.parent_index >= 0 && (s.parent_index as usize) < frame_samples.len() {
                frame_samples[s.parent_index as usize].function_name_index as i32
            } else {
                -1 // root
            };

            let key = (s.function_name_index, parent_name);
            let entry = accum.entry(key).or_insert_with(|| FuncAccum {
                self_time_sum: 0.0,
                total_time_sum: 0.0,
                call_count_sum: 0,
                frames_set: std::collections::HashSet::new(),
                category: s.category,
                depth: s.depth,
            });
            entry.self_time_sum += s.self_time_ms as f64;
            entry.total_time_sum += s.total_time_ms as f64;
            entry.call_count_sum += s.call_count as u64;
            entry.frames_set.insert(frame_idx);
            let _ = sample_idx; // used for parent lookup above
        }
    }

    // Compute grand totals for percentage calculation
    let grand_self_total: f64 = accum.values().map(|a| a.self_time_sum).sum();
    let grand_total_total: f64 = accum.values()
        .filter(|a| a.depth == 0)
        .map(|a| a.total_time_sum)
        .sum::<f64>()
        .max(1.0);

    // Build parent→children index
    let mut children_map: HashMap<i32, Vec<(u16, i32)>> = HashMap::new();
    for &(name_idx, parent_name) in accum.keys() {
        children_map.entry(parent_name).or_default().push((name_idx, parent_name));
    }

    // Recursive node builder
    fn build_node(
        name_idx: u16,
        parent_name: i32,
        accum: &HashMap<(u16, i32), FuncAccum>,
        children_map: &HashMap<i32, Vec<(u16, i32)>>,
        string_table: &[String],
        sampled_count: u32,
        grand_self_total: f64,
        grand_total_total: f64,
        max_depth: u8,
    ) -> Option<CallTreeNode> {
        let key = (name_idx, parent_name);
        let a = accum.get(&key)?;

        let name = string_table.get(name_idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("Function_{}", name_idx));

        let frames_called = a.frames_set.len() as u32;

        let mut children = Vec::new();
        if a.depth < max_depth {
            if let Some(child_keys) = children_map.get(&(name_idx as i32)) {
                for &(child_name, child_parent) in child_keys {
                    if let Some(child_node) = build_node(
                        child_name, child_parent, accum, children_map,
                        string_table, sampled_count, grand_self_total, grand_total_total,
                        max_depth,
                    ) {
                        children.push(child_node);
                    }
                }
                children.sort_by(|a, b| b.total_self_ms.partial_cmp(&a.total_self_ms).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        Some(CallTreeNode {
            name,
            category: a.category.label().to_string(),
            avg_self_ms: a.self_time_sum as f32 / sampled_count as f32,
            total_self_ms: a.self_time_sum as f32,
            self_pct: if grand_self_total > 0.0 { (a.self_time_sum / grand_self_total * 100.0) as f32 } else { 0.0 },
            avg_total_ms: a.total_time_sum as f32 / sampled_count as f32,
            total_total_ms: a.total_time_sum as f32,
            total_pct: if grand_total_total > 0.0 { (a.total_time_sum / grand_total_total * 100.0) as f32 } else { 0.0 },
            call_count: a.call_count_sum,
            avg_call_count: a.call_count_sum as f32 / sampled_count as f32,
            frames_called,
            calls_per_frame: if frames_called > 0 { a.call_count_sum as f32 / frames_called as f32 } else { 0.0 },
            depth: a.depth,
            children,
        })
    }

    // Build root-level nodes (parent_name == -1)
    let mut roots: Vec<CallTreeNode> = Vec::new();
    if let Some(root_keys) = children_map.get(&-1) {
        for &(name_idx, parent_name) in root_keys {
            if let Some(node) = build_node(
                name_idx, parent_name, &accum, &children_map,
                &session.string_table, sampled_count,
                grand_self_total, grand_total_total, 10,
            ) {
                roots.push(node);
            }
        }
    }

    roots.sort_by(|a, b| b.total_self_ms.partial_cmp(&a.total_self_ms).unwrap_or(std::cmp::Ordering::Equal));
    roots.truncate(top_n);
    roots
}

/// Reverse call tree: starts from specific functions and shows who calls them.
fn build_reverse_tree(
    session: &GaprofSession,
    sampled_frames: &[Vec<FunctionSample>],
    sampled_count: u32,
    category_filter: Option<FunctionCategory>,
    top_n: usize,
) -> Vec<CallTreeNode> {
    struct FuncTotal {
        self_time_sum: f64,
        total_time_sum: f64,
        call_count_sum: u64,
        frames_set: std::collections::HashSet<usize>,
        category: FunctionCategory,
    }

    let mut totals: HashMap<u16, FuncTotal> = HashMap::new();
    let mut caller_map: HashMap<u16, HashMap<u16, (f64, u64)>> = HashMap::new();

    for (frame_idx, frame_samples) in sampled_frames.iter().enumerate() {
        for s in frame_samples.iter() {
            if let Some(cat) = &category_filter {
                if s.category != *cat {
                    continue;
                }
            }

            let entry = totals.entry(s.function_name_index).or_insert_with(|| FuncTotal {
                self_time_sum: 0.0,
                total_time_sum: 0.0,
                call_count_sum: 0,
                frames_set: std::collections::HashSet::new(),
                category: s.category,
            });
            entry.self_time_sum += s.self_time_ms as f64;
            entry.total_time_sum += s.total_time_ms as f64;
            entry.call_count_sum += s.call_count as u64;
            entry.frames_set.insert(frame_idx);

            // Track caller
            if s.parent_index >= 0 && (s.parent_index as usize) < frame_samples.len() {
                let caller_idx = frame_samples[s.parent_index as usize].function_name_index;
                let caller_entry = caller_map.entry(s.function_name_index)
                    .or_default()
                    .entry(caller_idx)
                    .or_insert((0.0, 0));
                caller_entry.0 += s.self_time_ms as f64;
                caller_entry.1 += s.call_count as u64;
            }
        }
    }

    let grand_self_total: f64 = totals.values().map(|t| t.self_time_sum).sum::<f64>().max(1.0);
    let grand_total_total: f64 = totals.values().map(|t| t.total_time_sum).sum::<f64>().max(1.0);

    let mut sorted_funcs: Vec<_> = totals.iter().collect();
    sorted_funcs.sort_by(|a, b| b.1.self_time_sum.partial_cmp(&a.1.self_time_sum).unwrap_or(std::cmp::Ordering::Equal));
    sorted_funcs.truncate(top_n);

    fn build_reverse_node(
        name_idx: u16,
        depth: u8,
        totals: &HashMap<u16, FuncTotal>,
        caller_map: &HashMap<u16, HashMap<u16, (f64, u64)>>,
        string_table: &[String],
        sampled_count: u32,
        grand_self_total: f64,
        grand_total_total: f64,
        path: &mut Vec<u16>,
    ) -> Option<CallTreeNode> {
        let total = totals.get(&name_idx)?;
        let name = string_table.get(name_idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("Function_{}", name_idx));
        let frames_called = total.frames_set.len() as u32;

        if path.contains(&name_idx) {
            return None;
        }
        path.push(name_idx);

        let mut children = Vec::new();
        if let Some(callers) = caller_map.get(&name_idx) {
            let mut caller_entries: Vec<_> = callers.iter().collect();
            caller_entries.sort_by(|a, b| b.1 .0.partial_cmp(&a.1 .0).unwrap_or(std::cmp::Ordering::Equal));
            for (caller_idx, _) in caller_entries {
                if let Some(child) = build_reverse_node(
                    *caller_idx,
                    depth + 1,
                    totals,
                    caller_map,
                    string_table,
                    sampled_count,
                    grand_self_total,
                    grand_total_total,
                    path,
                ) {
                    children.push(child);
                }
            }
        }
        path.pop();

        Some(CallTreeNode {
            name,
            category: total.category.label().to_string(),
            avg_self_ms: total.self_time_sum as f32 / sampled_count as f32,
            total_self_ms: total.self_time_sum as f32,
            self_pct: (total.self_time_sum / grand_self_total * 100.0) as f32,
            avg_total_ms: total.total_time_sum as f32 / sampled_count as f32,
            total_total_ms: total.total_time_sum as f32,
            total_pct: (total.total_time_sum / grand_total_total * 100.0) as f32,
            call_count: total.call_count_sum,
            avg_call_count: total.call_count_sum as f32 / sampled_count as f32,
            frames_called,
            calls_per_frame: if frames_called > 0 { total.call_count_sum as f32 / frames_called as f32 } else { 0.0 },
            depth,
            children,
        })
    }

    let mut roots = Vec::new();
    for (name_idx, _) in sorted_funcs {
        let mut path = Vec::new();
        if let Some(node) = build_reverse_node(
            *name_idx,
            0,
            &totals,
            &caller_map,
            &session.string_table,
            sampled_count,
            grand_self_total,
            grand_total_total,
            &mut path,
        ) {
            roots.push(node);
        }
    }
    roots
}

/// Search for functions by name (case-insensitive substring match)
pub fn search_functions(
    session: &GaprofSession,
    query: &str,
) -> Vec<FunctionSearchResult> {
    if session.function_samples.is_empty() || query.is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();

    struct FuncAccum {
        self_time_sum: f64,
        call_count_sum: u64,
        frames_set: std::collections::HashSet<usize>,
        category: FunctionCategory,
    }
    let mut totals: HashMap<u16, FuncAccum> = HashMap::new();
    let sampled_count = session.function_samples.iter().filter(|f| !f.is_empty()).count() as f32;

    for (frame_idx, frame_samples) in session.function_samples.iter().enumerate() {
        for s in frame_samples.iter() {
            let entry = totals.entry(s.function_name_index).or_insert_with(|| FuncAccum {
                self_time_sum: 0.0,
                call_count_sum: 0,
                frames_set: std::collections::HashSet::new(),
                category: s.category,
            });
            entry.self_time_sum += s.self_time_ms as f64;
            entry.call_count_sum += s.call_count as u64;
            entry.frames_set.insert(frame_idx);
        }
    }

    let mut results: Vec<FunctionSearchResult> = totals.iter()
        .filter_map(|(name_idx, acc)| {
            let name = session.string_table.get(*name_idx as usize)?;
            if name.to_lowercase().contains(&query_lower) {
                Some(FunctionSearchResult {
                    name: name.clone(),
                    category: acc.category.label().to_string(),
                    avg_self_ms: acc.self_time_sum as f32 / sampled_count,
                    total_self_ms: acc.self_time_sum as f32,
                    frames_called: acc.frames_set.len() as u32,
                    avg_call_count: acc.call_count_sum as f32 / sampled_count,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.total_self_ms.partial_cmp(&a.total_self_ms).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(100);
    results
}
