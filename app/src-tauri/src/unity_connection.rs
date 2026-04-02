use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unity-Skills REST API client
/// Unity-Skills runs on localhost:8090-8100 with HttpListener

const PORT_RANGE: std::ops::RangeInclusive<u16> = 8090..=8100;
const TIMEOUT: Duration = Duration::from_secs(3);

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .unwrap_or_default()
}

/// Discover Unity-Skills instance by scanning ports
pub async fn discover_unity() -> Result<u16, String> {
    let client = client();
    for port in PORT_RANGE {
        let url = format!("http://127.0.0.1:{}/health", port);
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return Ok(port);
            }
        }
    }
    Err("未发现 Unity-Skills 实例。请确保已在 Unity 编辑器中安装并启动 Unity-Skills 插件。".to_string())
}

/// Check if a specific port is still alive
pub async fn check_connection(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    if let Ok(resp) = client().get(&url).send().await {
        resp.status().is_success()
    } else {
        false
    }
}

/// Raw response types from Unity-Skills REST endpoints

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnityProfilerStats {
    #[serde(default)]
    pub fps: f64,
    #[serde(default, alias = "frameTime")]
    pub frame_time: f64,
    #[serde(default, alias = "renderTime")]
    pub render_time: f64,
    #[serde(default)]
    pub batches: u32,
    #[serde(default, alias = "drawCalls")]
    pub draw_calls: u32,
    #[serde(default)]
    pub triangles: u64,
    #[serde(default)]
    pub vertices: u64,
    #[serde(default, alias = "usedMemory")]
    pub used_memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnityMemoryInfo {
    #[serde(default, alias = "totalMemory")]
    pub total_memory: u64,
    #[serde(default, alias = "usedHeap")]
    pub used_heap: u64,
    #[serde(default, alias = "monoHeap")]
    pub mono_heap: u64,
    #[serde(default, alias = "monoUsed")]
    pub mono_used: u64,
    #[serde(default, alias = "graphicsMemory")]
    pub graphics_memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnityRenderingStats {
    #[serde(default, alias = "setPassCalls")]
    pub set_pass_calls: u32,
    #[serde(default, alias = "shadowCasters")]
    pub shadow_casters: u32,
    #[serde(default, alias = "visibleSkinned")]
    pub visible_skinned: u32,
    #[serde(default)]
    pub animations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnityEditorState {
    #[serde(default, alias = "isPlaying")]
    pub is_playing: bool,
    #[serde(default, alias = "isPaused")]
    pub is_paused: bool,
    #[serde(default, alias = "isCompiling")]
    pub is_compiling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetailItem {
    pub name: String,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub size: u64,
}

/// Call a Unity-Skills skill endpoint
async fn call_skill<T: serde::de::DeserializeOwned>(port: u16, skill_name: &str, params: Option<serde_json::Value>) -> Result<T, String> {
    let url = format!("http://127.0.0.1:{}/skill/{}", port, skill_name);
    let client = client();
    let body = params.unwrap_or(serde_json::json!({}));
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Unity-Skills 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Unity-Skills 返回错误 {}: {}", status.as_u16(), text));
    }

    // Unity-Skills wraps results in { "success": true, "result": ... }
    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    // Try to extract result field, otherwise use the whole response
    let data = if let Some(result) = raw.get("result") {
        result.clone()
    } else {
        raw
    };

    serde_json::from_value(data).map_err(|e| format!("反序列化失败: {}", e))
}

/// Get editor play/pause/compiling state
pub async fn get_editor_state(port: u16) -> Result<UnityEditorState, String> {
    call_skill(port, "editor_get_state", None).await
}

/// Get profiler performance stats (FPS, frame time, batches, etc.)
pub async fn get_profiler_stats(port: u16) -> Result<UnityProfilerStats, String> {
    call_skill(port, "profiler_get_stats", None).await
}

/// Get memory overview
pub async fn get_memory_info(port: u16) -> Result<UnityMemoryInfo, String> {
    call_skill(port, "profiler_get_memory", None).await
}

/// Get rendering stats
pub async fn get_rendering_stats(port: u16) -> Result<UnityRenderingStats, String> {
    call_skill(port, "profiler_get_rendering_stats", None).await
}

/// Get texture memory details
pub async fn get_texture_memory(port: u16) -> Result<Vec<MemoryDetailItem>, String> {
    call_skill(port, "profiler_get_texture_memory", None).await
}

/// Get mesh memory details
pub async fn get_mesh_memory(port: u16) -> Result<Vec<MemoryDetailItem>, String> {
    call_skill(port, "profiler_get_mesh_memory", None).await
}

/// Get material memory details
pub async fn get_material_memory(port: u16) -> Result<Vec<MemoryDetailItem>, String> {
    call_skill(port, "profiler_get_material_memory", None).await
}

/// Get audio memory details
pub async fn get_audio_memory(port: u16) -> Result<Vec<MemoryDetailItem>, String> {
    call_skill(port, "profiler_get_audio_memory", None).await
}

/// Combined profiler frame poll — merges stats + memory + rendering in one call
pub async fn poll_profiler_frame(port: u16) -> Result<(UnityProfilerStats, UnityMemoryInfo, UnityRenderingStats), String> {
    let (stats, mem, render) = tokio::join!(
        get_profiler_stats(port),
        get_memory_info(port),
        get_rendering_stats(port),
    );
    if stats.is_err() && mem.is_err() && render.is_err() {
        return Err(
            stats
                .err()
                .or_else(|| mem.err())
                .or_else(|| render.err())
                .unwrap_or_else(|| "Unity-Skills profiler polling failed".to_string()),
        );
    }
    Ok((
        stats.unwrap_or_default(),
        mem.unwrap_or_default(),
        render.unwrap_or_default(),
    ))
}

/// Get full memory snapshot for AI analysis
pub async fn get_memory_snapshot(port: u16) -> Result<MemorySnapshot, String> {
    let (tex, mesh, mat, audio) = tokio::join!(
        get_texture_memory(port),
        get_mesh_memory(port),
        get_material_memory(port),
        get_audio_memory(port),
    );
    Ok(MemorySnapshot {
        textures: tex.unwrap_or_default(),
        meshes: mesh.unwrap_or_default(),
        materials: mat.unwrap_or_default(),
        audio: audio.unwrap_or_default(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemorySnapshot {
    pub textures: Vec<MemoryDetailItem>,
    pub meshes: Vec<MemoryDetailItem>,
    pub materials: Vec<MemoryDetailItem>,
    pub audio: Vec<MemoryDetailItem>,
}

/// Get scene analysis from optimization skills
pub async fn get_scene_analysis(port: u16) -> Result<serde_json::Value, String> {
    call_skill(port, "optimize_analyze_scene", None).await
}
