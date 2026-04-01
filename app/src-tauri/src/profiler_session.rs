use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex as TokioMutex;

use crate::unity_connection;

/// A single profiler data frame captured during polling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerFrame {
    pub timestamp: f64, // seconds since session start
    // Performance
    pub fps: f64,
    pub frame_time: f64,
    pub render_time: f64,
    // Batching
    pub batches: u32,
    pub draw_calls: u32,
    pub set_pass_calls: u32,
    // Geometry
    pub triangles: u64,
    pub vertices: u64,
    // Memory
    pub total_memory: u64,
    pub used_heap: u64,
    pub mono_heap: u64,
    pub mono_used: u64,
    pub graphics_memory: u64,
}

impl ProfilerFrame {
    pub fn from_poll(
        elapsed_secs: f64,
        stats: &unity_connection::UnityProfilerStats,
        mem: &unity_connection::UnityMemoryInfo,
        render: &unity_connection::UnityRenderingStats,
    ) -> Self {
        Self {
            timestamp: elapsed_secs,
            fps: stats.fps,
            frame_time: stats.frame_time,
            render_time: stats.render_time,
            batches: stats.batches,
            draw_calls: stats.draw_calls,
            set_pass_calls: render.set_pass_calls,
            triangles: stats.triangles,
            vertices: stats.vertices,
            total_memory: mem.total_memory,
            used_heap: mem.used_heap,
            mono_heap: mem.mono_heap,
            mono_used: mem.mono_used,
            graphics_memory: mem.graphics_memory,
        }
    }
}

/// Summary statistics computed from a completed session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub frame_count: u32,
    pub duration_secs: f64,
    pub avg_fps: f64,
    pub min_fps: f64,
    pub max_fps: f64,
    pub p99_frame_time: f64,
    pub avg_frame_time: f64,
    pub max_frame_time: f64,
    pub avg_draw_calls: f64,
    pub max_draw_calls: u32,
    pub avg_batches: f64,
    pub avg_triangles: f64,
    pub peak_memory: u64,
    pub avg_memory: u64,
    pub peak_mono: u64,
    pub peak_graphics_memory: u64,
}

impl SessionSummary {
    pub fn compute(frames: &[ProfilerFrame]) -> Self {
        if frames.is_empty() {
            return Self {
                frame_count: 0,
                duration_secs: 0.0,
                avg_fps: 0.0,
                min_fps: 0.0,
                max_fps: 0.0,
                p99_frame_time: 0.0,
                avg_frame_time: 0.0,
                max_frame_time: 0.0,
                avg_draw_calls: 0.0,
                max_draw_calls: 0,
                avg_batches: 0.0,
                avg_triangles: 0.0,
                peak_memory: 0,
                avg_memory: 0,
                peak_mono: 0,
                peak_graphics_memory: 0,
            };
        }
        let n = frames.len() as f64;
        let duration = frames.last().map(|f| f.timestamp).unwrap_or(0.0);

        let avg_fps = frames.iter().map(|f| f.fps).sum::<f64>() / n;
        let min_fps = frames.iter().map(|f| f.fps).fold(f64::MAX, f64::min);
        let max_fps = frames.iter().map(|f| f.fps).fold(0.0_f64, f64::max);

        let mut frame_times: Vec<f64> = frames.iter().map(|f| f.frame_time).collect();
        frame_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p99_idx = ((frame_times.len() as f64 * 0.99) as usize).min(frame_times.len() - 1);
        let p99_frame_time = frame_times[p99_idx];
        let avg_frame_time = frame_times.iter().sum::<f64>() / n;
        let max_frame_time = frame_times.last().copied().unwrap_or(0.0);

        let avg_draw_calls = frames.iter().map(|f| f.draw_calls as f64).sum::<f64>() / n;
        let max_draw_calls = frames.iter().map(|f| f.draw_calls).max().unwrap_or(0);
        let avg_batches = frames.iter().map(|f| f.batches as f64).sum::<f64>() / n;
        let avg_triangles = frames.iter().map(|f| f.triangles as f64).sum::<f64>() / n;

        let peak_memory = frames.iter().map(|f| f.total_memory).max().unwrap_or(0);
        let avg_memory = (frames.iter().map(|f| f.total_memory as f64).sum::<f64>() / n) as u64;
        let peak_mono = frames.iter().map(|f| f.mono_used).max().unwrap_or(0);
        let peak_graphics_memory = frames.iter().map(|f| f.graphics_memory).max().unwrap_or(0);

        Self {
            frame_count: frames.len() as u32,
            duration_secs: duration,
            avg_fps,
            min_fps,
            max_fps,
            p99_frame_time,
            avg_frame_time,
            max_frame_time,
            avg_draw_calls,
            max_draw_calls,
            avg_batches,
            avg_triangles,
            peak_memory,
            avg_memory,
            peak_mono,
            peak_graphics_memory,
        }
    }
}

/// A complete profiler recording session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerSession {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub frames: Vec<ProfilerFrame>,
    pub summary: SessionSummary,
    pub memory_snapshot: Option<unity_connection::MemorySnapshot>,
}

/// Session metadata (for listing without full frame data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub frame_count: u32,
    pub duration_secs: f64,
    pub avg_fps: f64,
    pub peak_memory: u64,
}

impl From<&ProfilerSession> for SessionMeta {
    fn from(s: &ProfilerSession) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            created_at: s.created_at.clone(),
            frame_count: s.summary.frame_count,
            duration_secs: s.summary.duration_secs,
            avg_fps: s.summary.avg_fps,
            peak_memory: s.summary.peak_memory,
        }
    }
}

/// Live profiler state maintained during an active recording
pub struct LiveProfilerState {
    pub active: bool,
    pub port: u16,
    pub session_id: String,
    pub session_name: String,
    pub start_time: std::time::Instant,
    pub frames: Vec<ProfilerFrame>,
    pub cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Default for LiveProfilerState {
    fn default() -> Self {
        Self {
            active: false,
            port: 0,
            session_id: String::new(),
            session_name: String::new(),
            start_time: std::time::Instant::now(),
            frames: Vec::new(),
            cancel_tx: None,
        }
    }
}

/// Manages profiler state and session persistence
pub struct ProfilerManager {
    pub live: Arc<TokioMutex<LiveProfilerState>>,
}

impl Default for ProfilerManager {
    fn default() -> Self {
        Self {
            live: Arc::new(TokioMutex::new(LiveProfilerState::default())),
        }
    }
}

impl ProfilerManager {
    /// Start a profiling session — spawns a polling task
    pub async fn start_session(
        &self,
        port: u16,
        session_name: String,
        app: tauri::AppHandle,
    ) -> Result<String, String> {
        let mut live = self.live.lock().await;
        if live.active {
            return Err("已有正在进行的性能分析会话".to_string());
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        live.active = true;
        live.port = port;
        live.session_id = session_id.clone();
        live.session_name = session_name;
        live.start_time = std::time::Instant::now();
        live.frames.clear();
        live.cancel_tx = Some(cancel_tx);

        let live_ref = Arc::clone(&self.live);
        let sid = session_id.clone();

        // Spawn polling task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            let mut cancel_rx = cancel_rx;
            let start = std::time::Instant::now();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let elapsed = start.elapsed().as_secs_f64();
                        // Poll profiler data
                        match unity_connection::poll_profiler_frame(port).await {
                            Ok((stats, mem, render)) => {
                                let frame = ProfilerFrame::from_poll(elapsed, &stats, &mem, &render);
                                // Emit to frontend
                                let _ = app.emit("profiler_frame", &frame);
                                // Store in live state
                                if let Ok(mut live) = live_ref.try_lock() {
                                    live.frames.push(frame);
                                }
                            }
                            Err(_) => {
                                // Connection lost — check if Unity stopped playing
                                if let Ok(state) = unity_connection::get_editor_state(port).await {
                                    if !state.is_playing {
                                        let _ = app.emit("profiler_auto_stop", &sid);
                                        break;
                                    }
                                } else {
                                    // Connection completely lost
                                    let _ = app.emit("profiler_auto_stop", &sid);
                                    break;
                                }
                            }
                        }
                    }
                    _ = &mut cancel_rx => {
                        break;
                    }
                }
            }
        });

        Ok(session_id)
    }

    /// Stop current session, compute summary, return completed session
    pub async fn stop_session(&self) -> Result<ProfilerSession, String> {
        let mut live = self.live.lock().await;
        if !live.active {
            return Err("没有正在进行的性能分析会话".to_string());
        }

        let port = live.port;

        // Signal cancel
        if let Some(tx) = live.cancel_tx.take() {
            let _ = tx.send(());
        }

        // Grab memory snapshot
        let snapshot = unity_connection::get_memory_snapshot(port).await.ok();

        let frames = std::mem::take(&mut live.frames);
        let summary = SessionSummary::compute(&frames);

        let session = ProfilerSession {
            id: live.session_id.clone(),
            name: live.session_name.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            frames,
            summary,
            memory_snapshot: snapshot,
        };

        live.active = false;
        Ok(session)
    }

    pub async fn is_active(&self) -> bool {
        self.live.lock().await.active
    }
}

// ======================== Session File I/O ========================

fn profiler_dir(project_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(project_path).join(".analytics").join("profiler")
}

pub fn save_session(project_path: &str, session: &ProfilerSession) -> Result<(), String> {
    let dir = profiler_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败: {}", e))?;
    let path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string(session).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

pub fn load_session(project_path: &str, session_id: &str) -> Result<ProfilerSession, String> {
    let path = profiler_dir(project_path).join(format!("{}.json", session_id));
    let json = std::fs::read_to_string(&path).map_err(|e| format!("读取会话失败: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("解析会话失败: {}", e))
}

pub fn list_sessions(project_path: &str) -> Vec<SessionMeta> {
    let dir = profiler_dir(project_path);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut metas = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<ProfilerSession>(&json) {
                        metas.push(SessionMeta::from(&session));
                    }
                }
            }
        }
    }
    metas.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    metas
}

pub fn delete_session(project_path: &str, session_id: &str) -> Result<(), String> {
    let path = profiler_dir(project_path).join(format!("{}.json", session_id));
    std::fs::remove_file(&path).map_err(|e| format!("删除会话失败: {}", e))
}

pub fn rename_session(project_path: &str, session_id: &str, new_name: &str) -> Result<(), String> {
    let path = profiler_dir(project_path).join(format!("{}.json", session_id));
    let json = std::fs::read_to_string(&path).map_err(|e| format!("读取会话失败: {}", e))?;
    let mut session: ProfilerSession = serde_json::from_str(&json).map_err(|e| format!("解析失败: {}", e))?;
    session.name = new_name.to_string();
    let json = serde_json::to_string(&session).map_err(|e| format!("序列化失败: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}
