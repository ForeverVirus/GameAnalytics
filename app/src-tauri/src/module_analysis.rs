// Per-Module Analysis Engine
// Generates dedicated analysis for each module page (rendering, scripting, physics, etc.)
// Each analysis contains: timeline data, function call tree, top functions, module-specific metrics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::device_profile::{
    FunctionCategory, GaprofSession, TimelinePoint,
};

// ======================== Data Structures ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulePageAnalysis {
    pub module_name: String,
    pub module_label: String,
    /// Module CPU time per frame (sampled timeline)
    pub timeline: Vec<TimelinePoint>,
    /// Sub-function timelines (key sub-functions of this module)
    pub sub_timelines: Vec<SubTimeline>,
    /// Top functions for this module, sorted by total self time
    pub top_functions: Vec<ModuleFunctionStats>,
    /// Module-specific metrics
    pub metrics: ModuleMetrics,
    /// Total sampled frames that had data for this module
    pub sampled_frames: u32,
    /// Average module CPU time ms
    pub avg_module_ms: f32,
    /// Max module CPU time ms
    pub max_module_ms: f32,
    /// Percentage of total CPU time consumed by this module
    pub percentage_of_total: f32,
    /// Whether this report contains any deep profiling function samples.
    pub function_sampling_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTimeline {
    pub name: String,
    pub timeline: Vec<TimelinePoint>,
    pub avg_ms: f32,
    pub max_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleFunctionStats {
    pub name: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetrics {
    /// Key-value pairs of module-specific metrics
    pub entries: Vec<MetricEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub label: String,
    pub value: String,
    pub severity: String, // "normal", "warning", "critical"
}

// ======================== Resource Memory Analysis ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMemoryAnalysis {
    pub total_memory_timeline: Vec<TimelinePoint>,
    pub mono_memory_timeline: Vec<TimelinePoint>,
    pub gfx_memory_timeline: Vec<TimelinePoint>,
    pub gc_alloc_timeline: Vec<TimelinePoint>,
    pub peak_total_mb: f32,
    pub avg_total_mb: f32,
    pub peak_mono_mb: f32,
    pub peak_gfx_mb: f32,
    pub total_gc_alloc_mb: f32,
    pub gc_alloc_per_frame_kb: f32,
    pub memory_trend: String,
    pub growth_rate_mb_per_min: f32,
    /// Per resource type breakdowns (placeholder for v3 extended data)
    pub resource_types: Vec<ResourceTypeBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTypeBreakdown {
    pub type_name: String,
    pub peak_mb: f32,
    pub avg_mb: f32,
    pub timeline: Vec<TimelinePoint>,
    pub top_instances: Vec<ResourceInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInstance {
    pub name: String,
    pub size_bytes: u64,
    pub size_label: String,
}

// ======================== Module Analysis Generators ========================

/// Main entry point: generate analysis for a named module.
pub fn generate_module_analysis(
    session: &GaprofSession,
    module_name: &str,
) -> Result<ModulePageAnalysis, String> {
    match module_name {
        "rendering" => Ok(gen_rendering(session)),
        "gpu_sync" => Ok(gen_gpu_sync(session)),
        "scripting" => Ok(gen_scripting(session)),
        "ui" => Ok(gen_ui(session)),
        "loading" => Ok(gen_loading(session)),
        "physics" => Ok(gen_physics(session)),
        "animation" => Ok(gen_animation(session)),
        "particles" => Ok(gen_particles(session)),
        "gpu" => Ok(gen_gpu(session)),
        _ => Err(format!("Unknown module: {}", module_name)),
    }
}

/// Generate resource memory analysis from session data
pub fn generate_resource_memory_analysis(session: &GaprofSession) -> ResourceMemoryAnalysis {
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let total_mem_tl = build_timeline(frames, sample_step, |f| f.total_allocated as f32 / (1024.0 * 1024.0));
    let mono_mem_tl = build_timeline(frames, sample_step, |f| f.mono_used_size as f32 / (1024.0 * 1024.0));
    let gfx_mem_tl = build_timeline(frames, sample_step, |f| f.gfx_memory as f32 / (1024.0 * 1024.0));
    let gc_alloc_tl = build_timeline(frames, sample_step, |f| f.gc_alloc_bytes as f32 / 1024.0);

    let peak_total = frames.iter().map(|f| f.total_allocated).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let avg_total = if total > 0 { frames.iter().map(|f| f.total_allocated).sum::<i64>() as f32 / total as f32 / (1024.0 * 1024.0) } else { 0.0 };
    let peak_mono = frames.iter().map(|f| f.mono_heap_size).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let peak_gfx = frames.iter().map(|f| f.gfx_memory).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let total_gc = frames.iter().map(|f| f.gc_alloc_bytes).sum::<i64>() as f32 / (1024.0 * 1024.0);
    let gc_per_frame = if total > 0 { frames.iter().map(|f| f.gc_alloc_bytes).sum::<i64>() as f32 / total as f32 / 1024.0 } else { 0.0 };

    // Memory trend
    let ten_pct = (total / 10).max(1);
    let early = if total > 0 { frames[..ten_pct].iter().map(|f| f.total_allocated).sum::<i64>() as f32 / ten_pct as f32 / (1024.0 * 1024.0) } else { 0.0 };
    let late = if total > 0 { frames[total.saturating_sub(ten_pct)..].iter().map(|f| f.total_allocated).sum::<i64>() as f32 / ten_pct as f32 / (1024.0 * 1024.0) } else { 0.0 };
    let growth = late - early;
    let duration_min = session.header.duration as f32 / 60.0;
    let rate = if duration_min > 0.0 { growth / duration_min } else { 0.0 };
    let trend = if rate > 5.0 { "leaking" } else if rate > 1.0 { "growing" } else { "stable" };

    ResourceMemoryAnalysis {
        total_memory_timeline: total_mem_tl,
        mono_memory_timeline: mono_mem_tl,
        gfx_memory_timeline: gfx_mem_tl,
        gc_alloc_timeline: gc_alloc_tl,
        peak_total_mb: peak_total,
        avg_total_mb: avg_total,
        peak_mono_mb: peak_mono,
        peak_gfx_mb: peak_gfx,
        total_gc_alloc_mb: total_gc,
        gc_alloc_per_frame_kb: gc_per_frame,
        memory_trend: trend.to_string(),
        growth_rate_mb_per_min: rate,
        resource_types: build_resource_types_from_frames(frames, sample_step),
    }
}

/// Build per-resource-type breakdowns from v3 per-frame data (already in MB)
fn build_resource_types_from_frames(frames: &[crate::device_profile::FrameData], sample_step: usize) -> Vec<ResourceTypeBreakdown> {
    let total = frames.len();
    if total == 0 { return Vec::new(); }

    let type_extractors: &[(&str, fn(&crate::device_profile::FrameData) -> f32)] = &[
        ("Texture",        |f| f.texture_memory),
        ("Mesh",           |f| f.mesh_memory),
        ("Material",       |f| f.material_memory),
        ("Shader",         |f| f.shader_memory),
        ("AnimationClip",  |f| f.anim_clip_memory),
        ("AudioClip",      |f| f.audio_clip_memory),
        ("Font",           |f| f.font_memory),
        ("RenderTexture",  |f| f.render_texture_memory),
        ("ParticleSystem", |f| f.particle_system_memory),
    ];

    let mut results = Vec::new();
    for &(name, extractor) in type_extractors {
        let has_data = frames.iter().any(|f| extractor(f) > 0.0);
        if !has_data { continue; }

        let peak = frames.iter().map(|f| extractor(f)).fold(0.0f32, f32::max);
        let avg = frames.iter().map(|f| extractor(f)).sum::<f32>() / total as f32;
        let timeline = build_timeline(frames, sample_step, extractor);

        results.push(ResourceTypeBreakdown {
            type_name: name.to_string(),
            peak_mb: peak,
            avg_mb: avg,
            timeline,
            top_instances: Vec::new(), // Per-instance data requires ResourceMemoryBlock parsing
        });
    }
    results
}

// ======================== Per-module generators ========================

fn gen_rendering(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Rendering;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.render_time);

    let avg = if total > 0 { frames.iter().map(|f| f.render_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.render_time).fold(0.0f32, f32::max);

    let submit_tl = build_timeline(frames, sample_step, |f| f.render_submit_time);
    let avg_submit = if total > 0 { frames.iter().map(|f| f.render_submit_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max_submit = frames.iter().map(|f| f.render_submit_time).fold(0.0f32, f32::max);

    let total_cpu = avg + avg_submit;
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { (avg + avg_submit) / total_module_time * 100.0 } else { 0.0 };

    let top_functions = extract_module_functions(session, category, 50);

    let avg_dc = if total > 0 { frames.iter().map(|f| f.draw_calls as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_batches = if total > 0 { frames.iter().map(|f| f.batches as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_tris = if total > 0 { frames.iter().map(|f| f.triangles as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_sp = if total > 0 { frames.iter().map(|f| f.set_pass_calls as f32).sum::<f32>() / total as f32 } else { 0.0 };

    ModulePageAnalysis {
        module_name: "rendering".to_string(),
        module_label: "渲染模块".to_string(),
        timeline,
        sub_timelines: vec![
            SubTimeline { name: "RenderSubmit".to_string(), timeline: submit_tl, avg_ms: avg_submit, max_ms: max_submit },
        ],
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均DrawCalls".into(), value: format!("{:.0}", avg_dc), severity: severity_draw_calls(avg_dc) },
                MetricEntry { label: "平均Batches".into(), value: format!("{:.0}", avg_batches), severity: "normal".into() },
                MetricEntry { label: "平均三角面".into(), value: format!("{:.0}", avg_tris), severity: severity_triangles(avg_tris) },
                MetricEntry { label: "平均SetPassCalls".into(), value: format!("{:.0}", avg_sp), severity: severity_setpass(avg_sp) },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: total_cpu,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_gpu_sync(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Sync;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.gpu_time_ms);

    let avg = if total > 0 { frames.iter().map(|f| f.gpu_time_ms).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.gpu_time_ms).fold(0.0f32, f32::max);

    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };

    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "gpu_sync".to_string(),
        module_label: "GPU同步模块".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均GPU同步耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 5.0 { "critical".into() } else if avg > 2.0 { "warning".into() } else { "normal".into() } },
                MetricEntry { label: "最大GPU同步耗时".into(), value: format!("{:.2}ms", max), severity: if max > 16.0 { "critical".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_scripting(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Scripting;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let script_time = |f: &crate::device_profile::FrameData| f.scripts_update_time + f.scripts_late_update_time + f.fixed_update_time;

    let timeline = build_timeline(frames, sample_step, script_time);

    let avg = if total > 0 { frames.iter().map(|f| script_time(f)).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| script_time(f)).fold(0.0f32, f32::max);

    // Sub-timelines for Update/LateUpdate/FixedUpdate
    let update_tl = build_timeline(frames, sample_step, |f| f.scripts_update_time);
    let late_tl = build_timeline(frames, sample_step, |f| f.scripts_late_update_time);
    let fixed_tl = build_timeline(frames, sample_step, |f| f.fixed_update_time);

    let avg_update = if total > 0 { frames.iter().map(|f| f.scripts_update_time).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_late = if total > 0 { frames.iter().map(|f| f.scripts_late_update_time).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_fixed = if total > 0 { frames.iter().map(|f| f.fixed_update_time).sum::<f32>() / total as f32 } else { 0.0 };

    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };

    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "scripting".to_string(),
        module_label: "逻辑代码模块".to_string(),
        timeline,
        sub_timelines: vec![
            SubTimeline { name: "Update".into(), timeline: update_tl, avg_ms: avg_update, max_ms: frames.iter().map(|f| f.scripts_update_time).fold(0.0f32, f32::max) },
            SubTimeline { name: "LateUpdate".into(), timeline: late_tl, avg_ms: avg_late, max_ms: frames.iter().map(|f| f.scripts_late_update_time).fold(0.0f32, f32::max) },
            SubTimeline { name: "FixedUpdate".into(), timeline: fixed_tl, avg_ms: avg_fixed, max_ms: frames.iter().map(|f| f.fixed_update_time).fold(0.0f32, f32::max) },
        ],
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均Update耗时".into(), value: format!("{:.2}ms", avg_update), severity: if avg_update > 5.0 { "warning".into() } else { "normal".into() } },
                MetricEntry { label: "平均LateUpdate耗时".into(), value: format!("{:.2}ms", avg_late), severity: if avg_late > 3.0 { "warning".into() } else { "normal".into() } },
                MetricEntry { label: "平均FixedUpdate耗时".into(), value: format!("{:.2}ms", avg_fixed), severity: if avg_fixed > 3.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_physics(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Physics;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.physics_time);

    let avg = if total > 0 { frames.iter().map(|f| f.physics_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.physics_time).fold(0.0f32, f32::max);
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };
    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "physics".to_string(),
        module_label: "物理系统".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均物理耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 3.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_animation(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Animation;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.animation_time);

    let avg = if total > 0 { frames.iter().map(|f| f.animation_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.animation_time).fold(0.0f32, f32::max);
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };
    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "animation".to_string(),
        module_label: "动画模块".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均动画耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 3.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_ui(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::UI;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.ui_time);

    let avg = if total > 0 { frames.iter().map(|f| f.ui_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.ui_time).fold(0.0f32, f32::max);
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };
    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "ui".to_string(),
        module_label: "UI模块".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均UI耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 2.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_loading(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Loading;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.loading_time);

    let avg = if total > 0 { frames.iter().map(|f| f.loading_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.loading_time).fold(0.0f32, f32::max);
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };
    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "loading".to_string(),
        module_label: "加载模块".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均加载耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 2.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_particles(session: &GaprofSession) -> ModulePageAnalysis {
    let category = FunctionCategory::Particles;
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.particle_time);

    let avg = if total > 0 { frames.iter().map(|f| f.particle_time).sum::<f32>() / total as f32 } else { 0.0 };
    let max = frames.iter().map(|f| f.particle_time).fold(0.0f32, f32::max);
    let total_module_time = compute_total_module_time(frames);
    let pct = if total_module_time > 0.0 { avg / total_module_time * 100.0 } else { 0.0 };
    let top_functions = extract_module_functions(session, category, 50);

    ModulePageAnalysis {
        module_name: "particles".to_string(),
        module_label: "粒子系统".to_string(),
        timeline,
        sub_timelines: Vec::new(),
        top_functions,
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均粒子耗时".into(), value: format!("{:.2}ms", avg), severity: if avg > 2.0 { "warning".into() } else { "normal".into() } },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg,
        max_module_ms: max,
        percentage_of_total: pct,
        function_sampling_enabled: has_function_samples(session),
    }
}

fn gen_gpu(session: &GaprofSession) -> ModulePageAnalysis {
    let frames = &session.frames;
    let total = frames.len();
    let sample_step = (total / 500).max(1);

    let timeline = build_timeline(frames, sample_step, |f| f.gpu_time_ms);

    let avg_gpu = if total > 0 { frames.iter().map(|f| f.gpu_time_ms).sum::<f32>() / total as f32 } else { 0.0 };
    let max_gpu = frames.iter().map(|f| f.gpu_time_ms).fold(0.0f32, f32::max);
    let avg_cpu = if total > 0 { frames.iter().map(|f| f.cpu_time_ms).sum::<f32>() / total as f32 } else { 0.0 };

    // GPU pressure: gpu_time / (1000/60) target frame budget
    let target_budget = 1000.0 / 60.0;
    let avg_pressure = avg_gpu / target_budget;
    let max_pressure = max_gpu / target_budget;

    let bottleneck = if avg_gpu > avg_cpu { "GPU Bound" } else { "CPU Bound" };

    // CPU time timeline for comparison
    let cpu_tl = build_timeline(frames, sample_step, |f| f.cpu_time_ms);

    ModulePageAnalysis {
        module_name: "gpu".to_string(),
        module_label: "GPU分析".to_string(),
        timeline,
        sub_timelines: vec![
            SubTimeline { name: "CPU Time".into(), timeline: cpu_tl, avg_ms: avg_cpu, max_ms: frames.iter().map(|f| f.cpu_time_ms).fold(0.0f32, f32::max) },
        ],
        top_functions: extract_module_functions_multi(session, &[FunctionCategory::Rendering, FunctionCategory::Sync], 50),
        metrics: ModuleMetrics {
            entries: vec![
                MetricEntry { label: "平均GPU耗时".into(), value: format!("{:.2}ms", avg_gpu), severity: if avg_gpu > 16.0 { "critical".into() } else if avg_gpu > 10.0 { "warning".into() } else { "normal".into() } },
                MetricEntry { label: "GPU压力系数".into(), value: format!("{:.2}", avg_pressure), severity: if avg_pressure > 1.0 { "critical".into() } else if avg_pressure > 0.7 { "warning".into() } else { "normal".into() } },
                MetricEntry { label: "最大GPU压力".into(), value: format!("{:.2}", max_pressure), severity: if max_pressure > 1.5 { "critical".into() } else { "normal".into() } },
                MetricEntry { label: "瓶颈判断".into(), value: bottleneck.into(), severity: "normal".into() },
            ],
        },
        sampled_frames: total as u32,
        avg_module_ms: avg_gpu,
        max_module_ms: max_gpu,
        percentage_of_total: 0.0, // N/A for GPU
        function_sampling_enabled: has_function_samples(session),
    }
}

// ======================== Helpers ========================

fn build_timeline<F>(
    frames: &[crate::device_profile::FrameData],
    sample_step: usize,
    extractor: F,
) -> Vec<TimelinePoint>
where
    F: Fn(&crate::device_profile::FrameData) -> f32,
{
    frames
        .iter()
        .enumerate()
        .step_by(sample_step.max(1))
        .map(|(idx, f)| TimelinePoint {
            time: f.timestamp,
            value: extractor(f),
            frame_index: Some(idx as u32),
        })
        .collect()
}

fn has_function_samples(session: &GaprofSession) -> bool {
    session.function_samples.iter().any(|samples| !samples.is_empty())
}

fn compute_total_module_time(frames: &[crate::device_profile::FrameData]) -> f32 {
    let total = frames.len();
    if total == 0 { return 0.0; }
    let avg = |f_fn: fn(&crate::device_profile::FrameData) -> f32| -> f32 {
        frames.iter().map(|f| f_fn(f)).sum::<f32>() / total as f32
    };
    avg(|f| f.render_time) + avg(|f| f.scripts_update_time + f.scripts_late_update_time + f.fixed_update_time)
        + avg(|f| f.physics_time) + avg(|f| f.animation_time) + avg(|f| f.ui_time)
        + avg(|f| f.particle_time) + avg(|f| f.loading_time) + avg(|f| f.gc_collect_time)
}

fn extract_module_functions(
    session: &GaprofSession,
    category: FunctionCategory,
    top_n: usize,
) -> Vec<ModuleFunctionStats> {
    extract_module_functions_multi(session, &[category], top_n)
}

fn extract_module_functions_multi(
    session: &GaprofSession,
    categories: &[FunctionCategory],
    top_n: usize,
) -> Vec<ModuleFunctionStats> {
    if session.function_samples.is_empty() {
        return Vec::new();
    }

    struct Accum {
        self_sum: f64,
        total_sum: f64,
        call_sum: u64,
        frames: std::collections::HashSet<usize>,
    }

    let mut map: HashMap<u16, Accum> = HashMap::new();
    let sampled_count = session.function_samples.iter().filter(|f| !f.is_empty()).count() as u32;

    for (frame_idx, frame_samples) in session.function_samples.iter().enumerate() {
        for s in frame_samples.iter() {
            if !categories.contains(&s.category) {
                continue;
            }
            let entry = map.entry(s.function_name_index).or_insert_with(|| Accum {
                self_sum: 0.0, total_sum: 0.0, call_sum: 0, frames: std::collections::HashSet::new(),
            });
            entry.self_sum += s.self_time_ms as f64;
            entry.total_sum += s.total_time_ms as f64;
            entry.call_sum += s.call_count as u64;
            entry.frames.insert(frame_idx);
        }
    }

    if map.is_empty() || sampled_count == 0 {
        return Vec::new();
    }

    let grand_self: f64 = map.values().map(|a| a.self_sum).sum::<f64>().max(1.0);
    let grand_total: f64 = map.values().map(|a| a.total_sum).sum::<f64>().max(1.0);

    let mut results: Vec<ModuleFunctionStats> = map.iter().map(|(name_idx, acc)| {
        let name = session.string_table.get(*name_idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("Function_{}", name_idx));
        let frames_called = acc.frames.len() as u32;
        ModuleFunctionStats {
            name,
            avg_self_ms: acc.self_sum as f32 / sampled_count as f32,
            total_self_ms: acc.self_sum as f32,
            self_pct: (acc.self_sum / grand_self * 100.0) as f32,
            avg_total_ms: acc.total_sum as f32 / sampled_count as f32,
            total_total_ms: acc.total_sum as f32,
            total_pct: (acc.total_sum / grand_total * 100.0) as f32,
            call_count: acc.call_sum,
            avg_call_count: acc.call_sum as f32 / sampled_count as f32,
            frames_called,
            calls_per_frame: if frames_called > 0 { acc.call_sum as f32 / frames_called as f32 } else { 0.0 },
        }
    }).collect();

    results.sort_by(|a, b| b.total_self_ms.partial_cmp(&a.total_self_ms).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_n);
    results
}

fn severity_draw_calls(avg: f32) -> String {
    if avg > 500.0 { "critical".into() } else if avg > 200.0 { "warning".into() } else { "normal".into() }
}
fn severity_triangles(avg: f32) -> String {
    if avg > 500000.0 { "critical".into() } else if avg > 200000.0 { "warning".into() } else { "normal".into() }
}
fn severity_setpass(avg: f32) -> String {
    if avg > 100.0 { "critical".into() } else if avg > 50.0 { "warning".into() } else { "normal".into() }
}
