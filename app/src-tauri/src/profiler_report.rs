use serde::{Deserialize, Serialize};

use crate::profiler_session::ProfilerSession;

/// AI-generated profiler report finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerFinding {
    pub category: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
    pub metric_name: Option<String>,
    pub metric_value: Option<String>,
}

/// AI-generated profiler analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerReport {
    pub session_id: String,
    pub health_score: u32,
    pub summary: String,
    pub findings: Vec<ProfilerFinding>,
    pub optimization_plan: String,
    pub raw_response: String,
    pub timestamp: String,
}

/// AI-generated deep analysis with source file correlations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepAnalysisReport {
    pub session_id: String,
    pub summary: String,
    pub source_findings: Vec<SourceFinding>,
    pub raw_response: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFinding {
    pub file_path: String,
    pub line_number: Option<u32>,
    pub category: String,
    pub issue: String,
    pub suggestion: String,
    pub estimated_impact: String,
}

/// Session comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub session_a_id: String,
    pub session_a_name: String,
    pub session_b_id: String,
    pub session_b_name: String,
    pub metrics: Vec<ComparisonMetric>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonMetric {
    pub name: String,
    pub unit: String,
    pub value_a: f64,
    pub value_b: f64,
    pub delta: f64,
    pub delta_percent: f64,
    pub improved: bool,
}

/// Build the AI prompt for profiler report generation
pub fn build_profiler_prompt(session: &ProfilerSession, language: &str) -> String {
    let summary = &session.summary;
    let lang_hint = if language == "zh" { "请用中文回复。" } else { "Please respond in English." };

    // Find worst frames (top 5 by frame_time)
    let mut worst: Vec<_> = session.frames.iter().collect();
    worst.sort_by(|a, b| b.frame_time.partial_cmp(&a.frame_time).unwrap_or(std::cmp::Ordering::Equal));
    worst.truncate(5);

    let worst_str: Vec<String> = worst.iter().map(|f| {
        format!("  t={:.1}s fps={:.1} frameTime={:.1}ms drawCalls={} triangles={} memory={}MB",
            f.timestamp, f.fps, f.frame_time, f.draw_calls, f.triangles, f.total_memory / 1024 / 1024)
    }).collect();

    let mem_section = if let Some(snap) = &session.memory_snapshot {
        let tex_top: Vec<String> = snap.textures.iter().take(10).map(|t| format!("    {} (count={}, size={}KB)", t.name, t.count, t.size / 1024)).collect();
        let mesh_top: Vec<String> = snap.meshes.iter().take(10).map(|m| format!("    {} (count={}, size={}KB)", m.name, m.count, m.size / 1024)).collect();
        format!(
            "\n内存快照:\n  贴图 Top 10:\n{}\n  网格 Top 10:\n{}\n  材质数: {}\n  音频数: {}",
            tex_top.join("\n"),
            mesh_top.join("\n"),
            snap.materials.len(),
            snap.audio.len()
        )
    } else {
        String::new()
    };

    format!(
r#"你是一位 Unity 性能优化专家。请分析以下 Profiler 数据并给出性能评估报告。

{lang_hint}

会话: "{name}" (时长 {dur:.1}s, {fc} 帧)
性能概要:
  平均 FPS: {avg_fps:.1}, 最低 FPS: {min_fps:.1}, 最高 FPS: {max_fps:.1}
  平均帧时间: {avg_ft:.2}ms, P99帧时间: {p99_ft:.2}ms, 最大帧时间: {max_ft:.2}ms
  平均 DrawCalls: {avg_dc:.0}, 最大 DrawCalls: {max_dc}
  平均 Batches: {avg_batch:.0}
  平均三角面: {avg_tri:.0}
  峰值内存: {peak_mem}MB, 平均内存: {avg_mem}MB
  峰值 Mono: {peak_mono}MB, 峰值 GPU内存: {peak_gpu}MB

最差帧 (Top 5):
{worst_frames}
{mem_section}

请按以下 JSON 格式输出（不要额外文本）：
{{
  "health_score": 0-100,
  "summary": "整体评价",
  "findings": [
    {{
      "category": "CPU|GPU|Memory|Rendering",
      "severity": "Critical|Warning|Info",
      "title": "问题标题",
      "description": "详细描述",
      "suggestion": "优化建议",
      "metric_name": "指标名",
      "metric_value": "指标值"
    }}
  ],
  "optimization_plan": "按优先级排列的优化方案 (markdown格式)"
}}"#,
        lang_hint = lang_hint,
        name = session.name,
        dur = summary.duration_secs,
        fc = summary.frame_count,
        avg_fps = summary.avg_fps,
        min_fps = summary.min_fps,
        max_fps = summary.max_fps,
        avg_ft = summary.avg_frame_time,
        p99_ft = summary.p99_frame_time,
        max_ft = summary.max_frame_time,
        avg_dc = summary.avg_draw_calls,
        max_dc = summary.max_draw_calls,
        avg_batch = summary.avg_batches,
        avg_tri = summary.avg_triangles,
        peak_mem = summary.peak_memory / 1024 / 1024,
        avg_mem = summary.avg_memory / 1024 / 1024,
        peak_mono = summary.peak_mono / 1024 / 1024,
        peak_gpu = summary.peak_graphics_memory / 1024 / 1024,
        worst_frames = worst_str.join("\n"),
        mem_section = mem_section,
    )
}

/// Build deep analysis prompt that correlates profiler data with source code
pub fn build_deep_analysis_prompt(
    session: &ProfilerSession,
    source_files: &[(String, String)], // (path, content snippet)
    language: &str,
) -> String {
    let lang_hint = if language == "zh" { "请用中文回复。" } else { "Please respond in English." };
    let summary = &session.summary;

    let files_section: Vec<String> = source_files.iter().take(20).map(|(path, content)| {
        // Truncate to first 200 lines
        let snippet: String = content.lines().take(200).collect::<Vec<_>>().join("\n");
        format!("--- {} ---\n{}", path, snippet)
    }).collect();

    format!(
r#"你是 Unity 性能优化专家。结合 Profiler 数据和源代码进行深度性能分析。

{lang_hint}

Profiler 概要:
  平均 FPS: {avg_fps:.1}, P99帧时间: {p99:.2}ms
  平均 DrawCalls: {dc:.0}, 峰值内存: {mem}MB

相关源代码:
{files}

请按以下 JSON 格式输出：
{{
  "summary": "深度分析总结",
  "source_findings": [
    {{
      "file_path": "文件路径",
      "line_number": null,
      "category": "CPU|GPU|Memory|Rendering|GC",
      "issue": "发现的问题",
      "suggestion": "具体的代码级优化建议",
      "estimated_impact": "High|Medium|Low"
    }}
  ]
}}"#,
        lang_hint = lang_hint,
        avg_fps = summary.avg_fps,
        p99 = summary.p99_frame_time,
        dc = summary.avg_draw_calls,
        mem = summary.peak_memory / 1024 / 1024,
        files = files_section.join("\n\n"),
    )
}

/// Compare two sessions numerically
pub fn compare_sessions(a: &ProfilerSession, b: &ProfilerSession) -> ComparisonResult {
    let sa = &a.summary;
    let sb = &b.summary;

    let mut metrics = Vec::new();

    let add = |metrics: &mut Vec<ComparisonMetric>, name: &str, unit: &str, va: f64, vb: f64, higher_is_better: bool| {
        let delta = vb - va;
        let delta_percent = if va.abs() > 0.001 { (delta / va) * 100.0 } else { 0.0 };
        let improved = if higher_is_better { delta > 0.0 } else { delta < 0.0 };
        metrics.push(ComparisonMetric {
            name: name.to_string(),
            unit: unit.to_string(),
            value_a: va,
            value_b: vb,
            delta,
            delta_percent,
            improved,
        });
    };

    add(&mut metrics, "平均 FPS", "fps", sa.avg_fps, sb.avg_fps, true);
    add(&mut metrics, "最低 FPS", "fps", sa.min_fps, sb.min_fps, true);
    add(&mut metrics, "平均帧时间", "ms", sa.avg_frame_time, sb.avg_frame_time, false);
    add(&mut metrics, "P99帧时间", "ms", sa.p99_frame_time, sb.p99_frame_time, false);
    add(&mut metrics, "最大帧时间", "ms", sa.max_frame_time, sb.max_frame_time, false);
    add(&mut metrics, "平均 DrawCalls", "", sa.avg_draw_calls, sb.avg_draw_calls, false);
    add(&mut metrics, "最大 DrawCalls", "", sa.max_draw_calls as f64, sb.max_draw_calls as f64, false);
    add(&mut metrics, "平均 Batches", "", sa.avg_batches, sb.avg_batches, false);
    add(&mut metrics, "平均三角面", "", sa.avg_triangles, sb.avg_triangles, false);
    add(&mut metrics, "峰值内存", "MB", sa.peak_memory as f64 / 1048576.0, sb.peak_memory as f64 / 1048576.0, false);
    add(&mut metrics, "平均内存", "MB", sa.avg_memory as f64 / 1048576.0, sb.avg_memory as f64 / 1048576.0, false);
    add(&mut metrics, "峰值 Mono", "MB", sa.peak_mono as f64 / 1048576.0, sb.peak_mono as f64 / 1048576.0, false);

    let improved_count = metrics.iter().filter(|m| m.improved).count();
    let total = metrics.len();
    let verdict = if improved_count > total / 2 {
        format!("整体改善: {}/{} 项指标好转", improved_count, total)
    } else if improved_count == total / 2 {
        format!("持平: {}/{} 项指标好转", improved_count, total)
    } else {
        format!("整体退化: 仅 {}/{} 项指标好转", improved_count, total)
    };

    ComparisonResult {
        session_a_id: a.id.clone(),
        session_a_name: a.name.clone(),
        session_b_id: b.id.clone(),
        session_b_name: b.name.clone(),
        metrics,
        verdict,
    }
}

/// Export a profiler report as markdown
pub fn export_report_markdown(report: &ProfilerReport, session_name: &str) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 性能分析报告: {}\n\n", session_name));
    md.push_str(&format!("**健康评分**: {}/100\n\n", report.health_score));
    md.push_str(&format!("## 总结\n\n{}\n\n", report.summary));

    if !report.findings.is_empty() {
        md.push_str("## 发现的问题\n\n");
        for (i, f) in report.findings.iter().enumerate() {
            md.push_str(&format!(
                "### {}. [{}] {} ({})\n\n{}\n\n**建议**: {}\n\n",
                i + 1,
                f.severity,
                f.title,
                f.category,
                f.description,
                f.suggestion
            ));
        }
    }

    md.push_str("## 优化方案\n\n");
    md.push_str(&report.optimization_plan);
    md.push_str("\n\n---\n\n");
    md.push_str(&format!("*生成时间: {}*\n", report.timestamp));
    md
}

/// Export comparison as markdown
pub fn export_comparison_markdown(result: &ComparisonResult) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 性能对比报告\n\n"));
    md.push_str(&format!("**会话 A**: {} | **会话 B**: {}\n\n", result.session_a_name, result.session_b_name));
    md.push_str(&format!("**结论**: {}\n\n", result.verdict));

    md.push_str("| 指标 | 会话 A | 会话 B | 变化 | 变化% | 趋势 |\n");
    md.push_str("|------|--------|--------|------|-------|------|\n");
    for m in &result.metrics {
        let trend = if m.improved { "✅ 改善" } else if m.delta.abs() < 0.01 { "➖ 持平" } else { "⚠️ 退化" };
        md.push_str(&format!(
            "| {} | {:.1}{} | {:.1}{} | {:+.1} | {:+.1}% | {} |\n",
            m.name, m.value_a, m.unit, m.value_b, m.unit, m.delta, m.delta_percent, trend
        ));
    }
    md
}
