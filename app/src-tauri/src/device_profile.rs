// Device Profiler - .gaprof Binary Parser & Report Module
// Parses .gaprof files exported by the Unity SDK and generates
// performance analysis reports with AI integration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

// ======================== Data Structures ========================

const MAGIC: &[u8; 6] = b"GAPROF";
const FRAME_BYTE_SIZE_V2: usize = 155;
const FRAME_BYTE_SIZE_V3: usize = 235; // v3: +9 resource memory i64 (72) + 2 × f32 (8) = +80 bytes

// Module flag bits (must match C# ModuleFlags)
const FLAG_FUNCTION_SAMPLES: u32 = 1 << 8;
const FLAG_LOG_ENTRIES: u32 = 1 << 9;
const FLAG_RESOURCE_MEMORY: u32 = 1 << 10;
const FLAG_GPU_ANALYSIS: u32 = 1 << 11;
const FLAG_CUSTOM_MODULE: u32 = 1 << 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaprofHeader {
    pub version: u16,
    pub module_flags: u32,
    pub frame_count: u32,
    pub duration: f64,
    pub screenshot_count: u16,
    pub device_info_offset: u64,
    pub frame_data_offset: u64,
    pub screenshot_index_offset: u64,
    pub overdraw_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeviceInfo {
    #[serde(alias = "deviceModel")]
    pub device_model: String,
    #[serde(alias = "deviceName")]
    pub device_name: String,
    #[serde(alias = "operatingSystem")]
    pub operating_system: String,
    #[serde(alias = "processorType")]
    pub processor_type: String,
    #[serde(alias = "processorCount")]
    pub processor_count: i32,
    #[serde(alias = "processorFrequency")]
    pub processor_frequency: i32,
    #[serde(alias = "systemMemoryMB", alias = "systemMemoryMb")]
    pub system_memory_mb: i32,
    #[serde(alias = "graphicsDeviceName")]
    pub graphics_device_name: String,
    #[serde(alias = "graphicsMemoryMB", alias = "graphicsMemoryMb")]
    pub graphics_memory_mb: i32,
    #[serde(alias = "screenWidth")]
    pub screen_width: i32,
    #[serde(alias = "screenHeight")]
    pub screen_height: i32,
    #[serde(alias = "screenDpi")]
    pub screen_dpi: f32,
    #[serde(alias = "qualityLevel")]
    pub quality_level: i32,
    #[serde(alias = "qualityName")]
    pub quality_name: String,
    #[serde(alias = "unityVersion")]
    pub unity_version: String,
    #[serde(alias = "appVersion")]
    pub app_version: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    // Timing
    pub timestamp: f32,
    pub delta_time: f32,
    pub fps: f32,
    pub cpu_time_ms: f32,
    pub gpu_time_ms: f32,
    // Module timings (12 floats)
    pub render_time: f32,
    pub scripts_update_time: f32,
    pub scripts_late_update_time: f32,
    pub physics_time: f32,
    pub animation_time: f32,
    pub ui_time: f32,
    pub particle_time: f32,
    pub loading_time: f32,
    pub gc_collect_time: f32,
    pub fixed_update_time: f32,
    pub render_submit_time: f32,
    pub other_time: f32,
    // Memory (6 x i64)
    pub total_allocated: i64,
    pub total_reserved: i64,
    pub mono_heap_size: i64,
    pub mono_used_size: i64,
    pub gfx_memory: i64,
    pub gc_alloc_bytes: i64,
    // Rendering (7 x i32)
    pub batches: i32,
    pub draw_calls: i32,
    pub set_pass_calls: i32,
    pub triangles: i32,
    pub vertices: i32,
    pub shadow_casters: i32,
    pub visible_skinned_meshes: i32,
    // Jank
    pub jank_level: u8,
    // Hardware
    pub battery_level: f32,
    pub temperature: f32,
    // Scene
    pub scene_index: u16,
    // V3: Resource memory (MB)
    pub texture_memory: f32,
    pub mesh_memory: f32,
    pub material_memory: f32,
    pub shader_memory: f32,
    pub anim_clip_memory: f32,
    pub audio_clip_memory: f32,
    pub font_memory: f32,
    pub render_texture_memory: f32,
    pub particle_system_memory: f32,
    // V3: GPU
    pub gpu_utilization: f32,
    pub cpu_frequency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotEntry {
    pub frame_index: u32,
    pub jpeg_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverdrawSample {
    pub frame_index: u32,
    pub timestamp: f32,
    pub avg_overdraw_layers: f32,
    pub heatmap_jpeg: Option<Vec<u8>>,
}

// ======================== V2 Data Structures ========================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunctionCategory {
    Rendering = 0,
    Scripting = 1,
    Physics = 2,
    Animation = 3,
    UI = 4,
    Loading = 5,
    Particles = 6,
    Sync = 7,
    Overhead = 8,
    GC = 9,
    Other = 10,
    Custom = 11,
}

impl FunctionCategory {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Rendering,
            1 => Self::Scripting,
            2 => Self::Physics,
            3 => Self::Animation,
            4 => Self::UI,
            5 => Self::Loading,
            6 => Self::Particles,
            7 => Self::Sync,
            8 => Self::Overhead,
            9 => Self::GC,
            11 => Self::Custom,
            _ => Self::Other,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Rendering => "渲染模块",
            Self::Scripting => "用户脚本",
            Self::Physics => "物理模块",
            Self::Animation => "动画模块",
            Self::UI => "UI模块",
            Self::Loading => "加载模块",
            Self::Particles => "粒子模块",
            Self::Sync => "同步等待",
            Self::Overhead => "引擎开销",
            Self::GC => "GC",
            Self::Other => "其他",
            Self::Custom => "自定义模块",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSample {
    pub function_name_index: u16,
    pub category: FunctionCategory,
    pub self_time_ms: f32,
    pub total_time_ms: f32,
    pub call_count: u16,
    pub depth: u8,
    pub parent_index: i16,
    pub thread_index: u8, // V3: 0=Main, 1=Render, 2+=Job threads
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: f32,
    pub frame_index: i32,
    pub log_type: u8, // Unity LogType raw enum: 0=Error, 1=Assert, 2=Warning, 3=Log, 4=Exception
    pub message: String,
    pub stack_trace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaprofSession {
    pub header: GaprofHeader,
    pub device_info: DeviceInfo,
    pub string_table: Vec<String>,
    pub frames: Vec<FrameData>,
    pub screenshots: Vec<ScreenshotEntry>,
    pub overdraw_samples: Vec<OverdrawSample>,
    // V2 fields
    pub function_samples: Vec<Vec<FunctionSample>>, // per-frame function samples
    pub log_entries: Vec<LogEntry>,
}

// ======================== Parser ========================

pub fn parse_gaprof(data: &[u8]) -> Result<GaprofSession, String> {
    let mut cursor = Cursor::new(data);

    // Read header
    let header = read_header(&mut cursor)?;

    // Read device info
    cursor.seek(SeekFrom::Start(header.device_info_offset))
        .map_err(|e| format!("Seek device info: {}", e))?;
    let device_info = read_device_info(&mut cursor)?;

    // Read string table
    let string_table = read_string_table(&mut cursor)?;

    // Read frames
    cursor.seek(SeekFrom::Start(header.frame_data_offset))
        .map_err(|e| format!("Seek frames: {}", e))?;
    let mut frames = Vec::with_capacity(header.frame_count as usize);
    for _ in 0..header.frame_count {
        frames.push(read_frame(&mut cursor, header.version)?);
    }

    // Some SDK builds wrote incorrect screenshot/overdraw offsets in the header.
    // The blocks are still laid out sequentially after frame data, so use the
    // actual post-frame cursor position as the canonical start of the screenshot block.
    let screenshot_block_offset = cursor.position();
    let (screenshots, overdraw_block_offset) = read_screenshots(data, screenshot_block_offset, header.screenshot_count)?;

    // Overdraw follows the screenshot block immediately.
    cursor.seek(SeekFrom::Start(overdraw_block_offset))
        .map_err(|e| format!("Seek overdraw: {}", e))?;
    let overdraw_samples = read_overdraw(&mut cursor)?;

    // V2: Read function samples
    let function_samples = if header.version >= 2 && (header.module_flags & FLAG_FUNCTION_SAMPLES) != 0 {
        read_function_samples(&mut cursor, header.version)?
    } else {
        Vec::new()
    };

    // V2: Read log entries
    let log_entries = if header.version >= 2 && (header.module_flags & FLAG_LOG_ENTRIES) != 0 {
        read_log_entries(&mut cursor)?
    } else {
        Vec::new()
    };

    Ok(GaprofSession {
        header,
        device_info,
        string_table,
        frames,
        screenshots,
        overdraw_samples,
        function_samples,
        log_entries,
    })
}

fn read_header(cursor: &mut Cursor<&[u8]>) -> Result<GaprofHeader, String> {
    let mut magic = [0u8; 6];
    cursor.read_exact(&mut magic).map_err(|e| format!("Read magic: {}", e))?;
    if &magic != MAGIC {
        return Err("Invalid .gaprof file: bad magic".into());
    }

    Ok(GaprofHeader {
        version: read_u16(cursor)?,
        module_flags: read_u32(cursor)?,
        frame_count: read_u32(cursor)?,
        duration: read_f64(cursor)?,
        screenshot_count: read_u16(cursor)?,
        device_info_offset: read_u64(cursor)?,
        frame_data_offset: read_u64(cursor)?,
        screenshot_index_offset: read_u64(cursor)?,
        overdraw_offset: read_u64(cursor)?,
    })
}

fn read_device_info(cursor: &mut Cursor<&[u8]>) -> Result<DeviceInfo, String> {
    let len = read_i32(cursor)? as usize;
    if len > 10 * 1024 * 1024 {
        return Err("Device info too large".into());
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read device info: {}", e))?;
    let json = String::from_utf8(buf).map_err(|e| format!("Device info UTF8: {}", e))?;
    serde_json::from_str(&json).map_err(|e| format!("Parse device info: {}", e))
}

fn read_string_table(cursor: &mut Cursor<&[u8]>) -> Result<Vec<String>, String> {
    let count = read_u16(cursor)? as usize;
    let mut table = Vec::with_capacity(count);
    for _ in 0..count {
        let len = read_u16(cursor)? as usize;
        let mut buf = vec![0u8; len];
        cursor.read_exact(&mut buf).map_err(|e| format!("Read string: {}", e))?;
        table.push(String::from_utf8(buf).map_err(|e| format!("String UTF8: {}", e))?);
    }
    Ok(table)
}

fn read_frame(cursor: &mut Cursor<&[u8]>, version: u16) -> Result<FrameData, String> {
    let mut frame = FrameData {
        timestamp: read_f32(cursor)?,
        delta_time: read_f32(cursor)?,
        fps: read_f32(cursor)?,
        cpu_time_ms: read_f32(cursor)?,
        gpu_time_ms: read_f32(cursor)?,
        render_time: read_f32(cursor)?,
        scripts_update_time: read_f32(cursor)?,
        scripts_late_update_time: read_f32(cursor)?,
        physics_time: read_f32(cursor)?,
        animation_time: read_f32(cursor)?,
        ui_time: read_f32(cursor)?,
        particle_time: read_f32(cursor)?,
        loading_time: read_f32(cursor)?,
        gc_collect_time: read_f32(cursor)?,
        fixed_update_time: read_f32(cursor)?,
        render_submit_time: read_f32(cursor)?,
        other_time: read_f32(cursor)?,
        total_allocated: read_i64(cursor)?,
        total_reserved: read_i64(cursor)?,
        mono_heap_size: read_i64(cursor)?,
        mono_used_size: read_i64(cursor)?,
        gfx_memory: read_i64(cursor)?,
        gc_alloc_bytes: read_i64(cursor)?,
        batches: read_i32(cursor)?,
        draw_calls: read_i32(cursor)?,
        set_pass_calls: read_i32(cursor)?,
        triangles: read_i32(cursor)?,
        vertices: read_i32(cursor)?,
        shadow_casters: read_i32(cursor)?,
        visible_skinned_meshes: read_i32(cursor)?,
        jank_level: read_u8(cursor)?,
        battery_level: read_f32(cursor)?,
        temperature: read_f32(cursor)?,
        scene_index: read_u16(cursor)?,
        // V3 defaults
        texture_memory: 0.0,
        mesh_memory: 0.0,
        material_memory: 0.0,
        shader_memory: 0.0,
        anim_clip_memory: 0.0,
        audio_clip_memory: 0.0,
        font_memory: 0.0,
        render_texture_memory: 0.0,
        particle_system_memory: 0.0,
        gpu_utilization: 0.0,
        cpu_frequency: 0.0,
    };
    // V3 extended frame data (no padding; v2 had 2-byte pad that was removed in v3)
    if version >= 3 {
        // Resource memory: 9 × i64 (bytes) → convert to f32 (MB)
        frame.texture_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.mesh_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.material_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.shader_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.anim_clip_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.audio_clip_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.font_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.render_texture_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        frame.particle_system_memory = read_i64(cursor)? as f32 / 1_048_576.0;
        // GPU metrics: 2 × f32
        frame.gpu_utilization = read_f32(cursor)?;
        frame.cpu_frequency = read_f32(cursor)?;
    } else {
        // V2: skip 2-byte alignment pad
        let mut pad = [0u8; 2];
        let _ = cursor.read_exact(&mut pad);
    }

    Ok(frame)
}

fn read_screenshots(
    data: &[u8],
    index_offset: u64,
    expected_count: u16,
) -> Result<(Vec<ScreenshotEntry>, u64), String> {
    struct IndexEntry {
        frame_index: u32,
        offset: u64,
        size: u32,
    }

    let data_len = data.len() as u64;
    let mut cursor = Cursor::new(data);
    cursor.seek(SeekFrom::Start(index_offset))
        .map_err(|e| format!("Seek screenshots: {}", e))?;

    let count = read_u16(&mut cursor)? as usize;
    if count > 10_000 {
        return Err(format!("Too many screenshots: {}", count));
    }
    if expected_count > 0 && count != expected_count as usize {
        return Err(format!(
            "Screenshot count mismatch: header={}, block={}",
            expected_count, count
        ));
    }

    let mut index = Vec::with_capacity(count);
    for _ in 0..count {
        index.push(IndexEntry {
            frame_index: read_u32(&mut cursor)?,
            offset: read_u64(&mut cursor)?,
            size: read_u32(&mut cursor)?,
        });
    }

    let data_start = index_offset
        .checked_add(2)
        .and_then(|v| v.checked_add(count as u64 * 16))
        .ok_or_else(|| "Screenshot block offset overflow".to_string())?;
    if data_start > data_len {
        return Err("Screenshot block starts beyond file size".into());
    }

    let sequential_end = index.iter().try_fold(data_start, |acc, entry| {
        acc.checked_add(entry.size as u64)
            .ok_or_else(|| "Screenshot block size overflow".to_string())
    })?;
    if sequential_end > data_len {
        return Err("Screenshot data exceeds file size".into());
    }

    let stored_offsets_valid = index
        .iter()
        .scan(data_start, |prev_end, entry| {
            let start = entry.offset;
            let end = entry.offset.saturating_add(entry.size as u64);
            let valid = start >= *prev_end && end <= data_len;
            *prev_end = end;
            Some(valid)
        })
        .all(|valid| valid);

    let mut screenshots = Vec::with_capacity(count);
    let mut next_offset = data_start;
    let mut block_end = data_start;

    for entry in index {
        let actual_offset = if stored_offsets_valid {
            entry.offset
        } else {
            let offset = next_offset;
            next_offset = next_offset
                .checked_add(entry.size as u64)
                .ok_or_else(|| "Screenshot offset overflow".to_string())?;
            offset
        };
        let end = actual_offset
            .checked_add(entry.size as u64)
            .ok_or_else(|| "Screenshot end offset overflow".to_string())?;
        if end > data_len {
            return Err("Screenshot data out of bounds".into());
        }

        let start = actual_offset as usize;
        let end = end as usize;
        screenshots.push(ScreenshotEntry {
            frame_index: entry.frame_index,
            jpeg_data: data[start..end].to_vec(),
        });
        block_end = block_end.max(actual_offset + entry.size as u64);
    }

    Ok((screenshots, block_end))
}

fn read_overdraw(cursor: &mut Cursor<&[u8]>) -> Result<Vec<OverdrawSample>, String> {
    let count = read_u16(cursor)? as usize;
    if count > 10_000 {
        return Err(format!("Too many overdraw samples: {}", count));
    }
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let frame_index = read_u32(cursor)?;
        let timestamp = read_f32(cursor)?;
        let avg = read_f32(cursor)?;
        let heatmap_size = read_u32(cursor)? as usize;
        let heatmap = if heatmap_size > 0 {
            let mut buf = vec![0u8; heatmap_size];
            cursor.read_exact(&mut buf).map_err(|e| format!("Read heatmap: {}", e))?;
            Some(buf)
        } else {
            None
        };
        samples.push(OverdrawSample { frame_index, timestamp, avg_overdraw_layers: avg, heatmap_jpeg: heatmap });
    }
    Ok(samples)
}

fn read_function_samples(cursor: &mut Cursor<&[u8]>, version: u16) -> Result<Vec<Vec<FunctionSample>>, String> {
    let frame_count = read_u32(cursor)? as usize;
    let mut all_frames = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        let sample_count = read_u16(cursor)? as usize;
        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let function_name_index = read_u16(cursor)?;
            let category = FunctionCategory::from_u8(read_u8(cursor)?);
            let self_time_ms = read_f32(cursor)?;
            let total_time_ms = read_f32(cursor)?;
            let call_count = read_u16(cursor)?;
            let depth = read_u8(cursor)?;
            let parent_index = read_i16(cursor)?;
            let thread_index = if version >= 3 { read_u8(cursor)? } else { 0 };
            samples.push(FunctionSample {
                function_name_index,
                category,
                self_time_ms,
                total_time_ms,
                call_count,
                depth,
                parent_index,
                thread_index,
            });
        }
        all_frames.push(samples);
    }
    Ok(all_frames)
}

fn read_log_entries(cursor: &mut Cursor<&[u8]>) -> Result<Vec<LogEntry>, String> {
    let count = read_u32(cursor)? as usize;
    if count > 100_000 {
        return Err("Too many log entries".into());
    }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let timestamp = read_f32(cursor)?;
        let frame_index = read_i32(cursor)?;
        let log_type = read_u8(cursor)?;
        let msg_len = read_u16(cursor)? as usize;
        let mut msg_buf = vec![0u8; msg_len];
        cursor.read_exact(&mut msg_buf).map_err(|e| format!("Read log msg: {}", e))?;
        let message = String::from_utf8(msg_buf).unwrap_or_default();
        let st_len = read_u16(cursor)? as usize;
        let mut st_buf = vec![0u8; st_len];
        cursor.read_exact(&mut st_buf).map_err(|e| format!("Read log stack: {}", e))?;
        let stack_trace = String::from_utf8(st_buf).unwrap_or_default();
        entries.push(LogEntry { timestamp, frame_index, log_type, message, stack_trace });
    }
    Ok(entries)
}

fn read_i16(c: &mut Cursor<&[u8]>) -> Result<i16, String> {
    let mut buf = [0u8; 2];
    c.read_exact(&mut buf).map_err(|e| format!("read i16: {}", e))?;
    Ok(i16::from_le_bytes(buf))
}

// ======================== Binary Helpers ========================

fn read_u8(c: &mut Cursor<&[u8]>) -> Result<u8, String> {
    let mut buf = [0u8; 1];
    c.read_exact(&mut buf).map_err(|e| format!("read u8: {}", e))?;
    Ok(buf[0])
}

fn read_u16(c: &mut Cursor<&[u8]>) -> Result<u16, String> {
    let mut buf = [0u8; 2];
    c.read_exact(&mut buf).map_err(|e| format!("read u16: {}", e))?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32(c: &mut Cursor<&[u8]>) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    c.read_exact(&mut buf).map_err(|e| format!("read u32: {}", e))?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i32(c: &mut Cursor<&[u8]>) -> Result<i32, String> {
    let mut buf = [0u8; 4];
    c.read_exact(&mut buf).map_err(|e| format!("read i32: {}", e))?;
    Ok(i32::from_le_bytes(buf))
}

fn read_u64(c: &mut Cursor<&[u8]>) -> Result<u64, String> {
    let mut buf = [0u8; 8];
    c.read_exact(&mut buf).map_err(|e| format!("read u64: {}", e))?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64(c: &mut Cursor<&[u8]>) -> Result<i64, String> {
    let mut buf = [0u8; 8];
    c.read_exact(&mut buf).map_err(|e| format!("read i64: {}", e))?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32(c: &mut Cursor<&[u8]>) -> Result<f32, String> {
    let mut buf = [0u8; 4];
    c.read_exact(&mut buf).map_err(|e| format!("read f32: {}", e))?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64(c: &mut Cursor<&[u8]>) -> Result<f64, String> {
    let mut buf = [0u8; 8];
    c.read_exact(&mut buf).map_err(|e| format!("read f64: {}", e))?;
    Ok(f64::from_le_bytes(buf))
}

fn build_frame_timeline<F>(frames: &[FrameData], sample_step: usize, extractor: F) -> Vec<TimelinePoint>
where
    F: Fn(&FrameData) -> f32,
{
    frames
        .iter()
        .enumerate()
        .step_by(sample_step.max(1))
        .map(|(idx, frame)| TimelinePoint {
            time: frame.timestamp,
            value: extractor(frame),
            frame_index: Some(idx as u32),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_gaprof;

    fn push_u8(buf: &mut Vec<u8>, value: u8) {
        buf.push(value);
    }

    fn push_u16(buf: &mut Vec<u8>, value: u16) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i32(buf: &mut Vec<u8>, value: i32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u64(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(buf: &mut Vec<u8>, value: f32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f64(buf: &mut Vec<u8>, value: f64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn build_broken_v3_gaprof() -> Vec<u8> {
        let mut data = Vec::new();

        let device_info_bytes = br#"{}"#;
        let frame_data_offset = 64 + 4 + device_info_bytes.len() as u64 + 2;
        let frame_size_v3 = 235u64;
        let broken_frame_size = 157u64;
        let frame_count = 1u32;
        let screenshot_count = 1u16;

        let actual_screenshot_index_offset = frame_data_offset + frame_size_v3;
        let broken_screenshot_index_offset = frame_data_offset + broken_frame_size;
        let screenshot_block_size = 2 + 16u64;
        let actual_screenshot_data_offset = actual_screenshot_index_offset + screenshot_block_size;
        let broken_screenshot_data_offset = broken_screenshot_index_offset + screenshot_block_size;

        let screenshot_bytes = [1u8, 2, 3];
        let actual_overdraw_offset = actual_screenshot_data_offset + screenshot_bytes.len() as u64;
        let broken_overdraw_offset = broken_screenshot_data_offset + screenshot_bytes.len() as u64;
        let heatmap_bytes = [4u8, 5];

        data.extend_from_slice(b"GAPROF");
        push_u16(&mut data, 3);
        push_u32(&mut data, 0);
        push_u32(&mut data, frame_count);
        push_f64(&mut data, 1.0);
        push_u16(&mut data, screenshot_count);
        push_u64(&mut data, 64);
        push_u64(&mut data, frame_data_offset);
        push_u64(&mut data, broken_screenshot_index_offset);
        push_u64(&mut data, broken_overdraw_offset);
        data.resize(64, 0);

        push_i32(&mut data, device_info_bytes.len() as i32);
        data.extend_from_slice(device_info_bytes);

        push_u16(&mut data, 0);

        // One zeroed v3 frame.
        for _ in 0..17 {
            push_f32(&mut data, 0.0);
        }
        for _ in 0..6 {
            data.extend_from_slice(&0i64.to_le_bytes());
        }
        for _ in 0..7 {
            push_i32(&mut data, 0);
        }
        push_u8(&mut data, 0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);
        push_u16(&mut data, 0);
        for _ in 0..9 {
            data.extend_from_slice(&0i64.to_le_bytes());
        }
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);

        assert_eq!(data.len() as u64, actual_screenshot_index_offset);

        push_u16(&mut data, 1);
        push_u32(&mut data, 0);
        push_u64(&mut data, broken_screenshot_data_offset);
        push_u32(&mut data, screenshot_bytes.len() as u32);

        assert_eq!(data.len() as u64, actual_screenshot_data_offset);
        data.extend_from_slice(&screenshot_bytes);

        assert_eq!(data.len() as u64, actual_overdraw_offset);
        push_u16(&mut data, 1);
        push_u32(&mut data, 0);
        push_f32(&mut data, 1.0);
        push_f32(&mut data, 2.0);
        push_u32(&mut data, heatmap_bytes.len() as u32);
        data.extend_from_slice(&heatmap_bytes);

        data
    }

    #[test]
    fn parse_gaprof_recovers_from_broken_v3_block_offsets() {
        let data = build_broken_v3_gaprof();
        let session = parse_gaprof(&data).expect("parser should recover from broken offsets");

        assert_eq!(session.frames.len(), 1);
        assert_eq!(session.screenshots.len(), 1);
        assert_eq!(session.screenshots[0].frame_index, 0);
        assert_eq!(session.screenshots[0].jpeg_data, vec![1, 2, 3]);
        assert_eq!(session.overdraw_samples.len(), 1);
        assert_eq!(session.overdraw_samples[0].frame_index, 0);
        assert_eq!(session.overdraw_samples[0].heatmap_jpeg.as_deref(), Some(&[4, 5][..]));
    }
}

// ======================== Analysis & Report ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfileReport {
    pub session_name: String,
    pub source_file_path: Option<String>,
    pub device_info: DeviceInfo,
    pub duration_seconds: f64,
    pub total_frames: u32,
    pub summary: PerformanceSummary,
    pub fps_analysis: FpsAnalysis,
    pub memory_analysis: MemoryAnalysis,
    pub rendering_analysis: RenderingAnalysis,
    pub module_analysis: ModuleAnalysis,
    pub jank_analysis: JankAnalysis,
    pub thermal_analysis: ThermalAnalysis,
    pub overdraw_analysis: Option<OverdrawAnalysis>,
    pub function_analysis: Option<FunctionAnalysis>,
    pub log_analysis: Option<LogAnalysis>,
    pub scene_breakdown: Vec<SceneStats>,
    pub overall_grade: String,
    pub screenshot_count: usize,
    pub screenshot_frame_indices: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub avg_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub p1_fps: f32,
    pub p5_fps: f32,
    pub p50_fps: f32,
    pub p95_fps: f32,
    pub p99_fps: f32,
    pub fps_stability: f32, // coefficient of variation
    pub avg_cpu_ms: f32,
    pub avg_gpu_ms: f32,
    pub peak_memory_mb: f32,
    pub avg_memory_mb: f32,
    pub total_gc_alloc_mb: f32,
    pub jank_count: u32,
    pub severe_jank_count: u32,
    pub jank_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsAnalysis {
    pub target_fps: i32,
    pub frames_below_target: u32,
    pub below_target_pct: f32,
    pub frames_below_30: u32,
    pub below_30_pct: f32,
    pub fps_histogram: Vec<FpsBucket>,
    pub fps_timeline: Vec<TimelinePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsBucket {
    pub label: String,
    pub count: u32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub time: f32,
    pub value: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    pub peak_total_mb: f32,
    pub avg_total_mb: f32,
    pub peak_mono_mb: f32,
    pub peak_gfx_mb: f32,
    pub total_gc_alloc_mb: f32,
    pub gc_alloc_per_frame_bytes: f32,
    pub memory_timeline: Vec<TimelinePoint>,
    pub memory_trend: String, // "stable", "growing", "leaking"
    pub memory_growth_rate_mb_per_min: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderingAnalysis {
    pub avg_draw_calls: f32,
    pub max_draw_calls: i32,
    pub avg_batches: f32,
    pub avg_triangles: f32,
    pub max_triangles: i32,
    pub avg_set_pass: f32,
    pub batching_efficiency: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleAnalysis {
    pub avg_render_ms: f32,
    pub avg_scripts_ms: f32,
    pub avg_physics_ms: f32,
    pub avg_animation_ms: f32,
    pub avg_ui_ms: f32,
    pub avg_particle_ms: f32,
    pub avg_loading_ms: f32,
    pub avg_gc_ms: f32,
    pub bottleneck: String,
    pub module_breakdown: Vec<ModuleBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleBreakdown {
    pub name: String,
    pub avg_ms: f32,
    pub max_ms: f32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JankAnalysis {
    pub total_jank_frames: u32,
    pub severe_jank_frames: u32,
    pub jank_rate_pct: f32,
    pub severe_jank_rate_pct: f32,
    pub worst_frame_ms: f32,
    pub worst_frame_index: u32,
    pub jank_timeline: Vec<TimelinePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalAnalysis {
    pub has_data: bool,
    pub avg_temperature: f32,
    pub max_temperature: f32,
    pub battery_drain: f32, // start level - end level
    pub temperature_timeline: Vec<TimelinePoint>,
    pub thermal_throttle_risk: String, // "low", "medium", "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverdrawAnalysis {
    pub avg_overdraw: f32,
    pub max_overdraw: f32,
    pub sample_count: usize,
}

// ======================== V2 Analysis Structures ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionStats {
    pub name: String,
    pub category: String,
    pub avg_self_ms: f32,
    pub total_self_ms: f32,
    pub self_pct: f32,
    pub avg_total_ms: f32,
    pub total_total_ms: f32,
    pub total_pct: f32,
    pub avg_call_count: f32,
    pub total_call_count: u64,
    pub frames_called: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionAnalysis {
    pub has_data: bool,
    pub total_sampled_frames: u32,
    pub top_functions: Vec<FunctionStats>,           // by total self time, top 50
    pub category_breakdown: Vec<CategoryBreakdown>,
    pub per_frame_data: Vec<PerFrameFunctions>,       // for per-frame navigator
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryBreakdown {
    pub category: String,
    pub avg_ms: f32,
    pub total_ms: f32,
    pub percentage: f32,
    pub function_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerFrameFunctions {
    pub frame_index: u32,
    pub functions: Vec<PerFrameFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerFrameFunction {
    pub name: String,
    pub category: String,
    pub self_ms: f32,
    pub total_ms: f32,
    pub call_count: u16,
    pub depth: u8,
    pub parent_index: i16,
    pub thread_index: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAnalysis {
    pub has_data: bool,
    pub total_logs: usize,
    pub info_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub exception_count: usize,
    pub top_info: Vec<LogSummaryEntry>,
    pub top_errors: Vec<LogSummaryEntry>,
    pub top_warnings: Vec<LogSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummaryEntry {
    pub message: String,
    pub count: usize,
    pub first_frame: i32,
    pub log_type: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneStats {
    pub scene_name: String,
    pub frame_count: u32,
    pub avg_fps: f32,
    pub avg_memory_mb: f32,
    pub jank_count: u32,
}

// ======================== Report Generator ========================

pub fn generate_report(session: &GaprofSession, session_name: &str) -> DeviceProfileReport {
    let frames = &session.frames;
    let total = frames.len() as u32;
    let target_fps = 60;

    // FPS percentiles
    let mut fps_sorted: Vec<f32> = frames.iter().map(|f| f.fps).collect();
    fps_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let percentile = |sorted: &[f32], p: f32| -> f32 {
        if sorted.is_empty() { return 0.0; }
        let idx = ((p / 100.0) * (sorted.len() - 1) as f32) as usize;
        sorted[idx.min(sorted.len() - 1)]
    };

    let avg_fps: f32 = if total > 0 { fps_sorted.iter().sum::<f32>() / total as f32 } else { 0.0 };
    let min_fps = fps_sorted.first().copied().unwrap_or(0.0);
    let max_fps = fps_sorted.last().copied().unwrap_or(0.0);

    // FPS stability (coefficient of variation)
    let fps_variance: f32 = if total > 0 {
        frames.iter().map(|f| (f.fps - avg_fps).powi(2)).sum::<f32>() / total as f32
    } else { 0.0 };
    let fps_std = fps_variance.sqrt();
    let fps_stability = if avg_fps > 0.0 { 1.0 - (fps_std / avg_fps).min(1.0) } else { 0.0 };

    // CPU/GPU
    let avg_cpu_ms: f32 = if total > 0 { frames.iter().map(|f| f.cpu_time_ms).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_gpu_ms: f32 = if total > 0 { frames.iter().map(|f| f.gpu_time_ms).sum::<f32>() / total as f32 } else { 0.0 };

    // Memory
    let peak_memory = frames.iter().map(|f| f.total_allocated).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let avg_memory = if total > 0 {
        frames.iter().map(|f| f.total_allocated).sum::<i64>() as f32 / total as f32 / (1024.0 * 1024.0)
    } else { 0.0 };
    let total_gc_alloc = frames.iter().map(|f| f.gc_alloc_bytes).sum::<i64>() as f32 / (1024.0 * 1024.0);

    // Jank
    let jank_count = frames.iter().filter(|f| f.jank_level >= 1).count() as u32;
    let severe_jank_count = frames.iter().filter(|f| f.jank_level >= 2).count() as u32;
    let jank_rate = if total > 0 { jank_count as f32 / total as f32 * 100.0 } else { 0.0 };

    let summary = PerformanceSummary {
        avg_fps,
        min_fps,
        max_fps,
        p1_fps: percentile(&fps_sorted, 1.0),
        p5_fps: percentile(&fps_sorted, 5.0),
        p50_fps: percentile(&fps_sorted, 50.0),
        p95_fps: percentile(&fps_sorted, 95.0),
        p99_fps: percentile(&fps_sorted, 99.0),
        fps_stability,
        avg_cpu_ms,
        avg_gpu_ms,
        peak_memory_mb: peak_memory,
        avg_memory_mb: avg_memory,
        total_gc_alloc_mb: total_gc_alloc,
        jank_count,
        severe_jank_count,
        jank_rate,
    };

    // FPS Analysis
    let frames_below_target = frames.iter().filter(|f| f.fps < target_fps as f32).count() as u32;
    let frames_below_30 = frames.iter().filter(|f| f.fps < 30.0).count() as u32;

    let fps_buckets = vec![
        ("0-15", 0.0, 15.0),
        ("15-30", 15.0, 30.0),
        ("30-45", 30.0, 45.0),
        ("45-55", 45.0, 55.0),
        ("55-60", 55.0, 60.0),
        ("60+", 60.0, f32::MAX),
    ];
    let fps_histogram: Vec<FpsBucket> = fps_buckets.iter().map(|(label, lo, hi)| {
        let count = frames.iter().filter(|f| f.fps >= *lo && f.fps < *hi).count() as u32;
        FpsBucket {
            label: label.to_string(),
            count,
            percentage: if total > 0 { count as f32 / total as f32 * 100.0 } else { 0.0 },
        }
    }).collect();

    // Timeline (sample every N frames for manageable size)
    let sample_step = (total as usize / 500).max(1);
    let fps_timeline = build_frame_timeline(frames, sample_step, |f| f.fps);

    let fps_analysis = FpsAnalysis {
        target_fps,
        frames_below_target,
        below_target_pct: if total > 0 { frames_below_target as f32 / total as f32 * 100.0 } else { 0.0 },
        frames_below_30,
        below_30_pct: if total > 0 { frames_below_30 as f32 / total as f32 * 100.0 } else { 0.0 },
        fps_histogram,
        fps_timeline,
    };

    // Memory Analysis
    let peak_mono = frames.iter().map(|f| f.mono_heap_size).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let peak_gfx = frames.iter().map(|f| f.gfx_memory).max().unwrap_or(0) as f32 / (1024.0 * 1024.0);
    let gc_per_frame = if total > 0 { frames.iter().map(|f| f.gc_alloc_bytes).sum::<i64>() as f32 / total as f32 } else { 0.0 };

    let memory_timeline = build_frame_timeline(frames, sample_step, |f| f.total_allocated as f32 / (1024.0 * 1024.0));

    // Memory trend: compare first 10% avg to last 10% avg
    let ten_pct = (total as usize / 10).max(1);
    let early_mem = if total > 0 {
        frames[..ten_pct].iter().map(|f| f.total_allocated).sum::<i64>() as f32 / ten_pct as f32 / (1024.0 * 1024.0)
    } else { 0.0 };
    let late_mem = if total > 0 {
        frames[frames.len().saturating_sub(ten_pct)..].iter().map(|f| f.total_allocated).sum::<i64>() as f32 / ten_pct as f32 / (1024.0 * 1024.0)
    } else { 0.0 };
    let growth = late_mem - early_mem;
    let (memory_trend, growth_rate) = if session.header.duration > 0.0 {
        let rate = growth / (session.header.duration as f32 / 60.0);
        if rate > 5.0 { ("leaking".to_string(), rate) }
        else if rate > 1.0 { ("growing".to_string(), rate) }
        else { ("stable".to_string(), rate) }
    } else {
        ("stable".to_string(), 0.0)
    };

    let memory_analysis = MemoryAnalysis {
        peak_total_mb: peak_memory,
        avg_total_mb: avg_memory,
        peak_mono_mb: peak_mono,
        peak_gfx_mb: peak_gfx,
        total_gc_alloc_mb: total_gc_alloc,
        gc_alloc_per_frame_bytes: gc_per_frame,
        memory_timeline,
        memory_trend,
        memory_growth_rate_mb_per_min: growth_rate,
    };

    // Rendering Analysis
    let avg_dc = if total > 0 { frames.iter().map(|f| f.draw_calls as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let max_dc = frames.iter().map(|f| f.draw_calls).max().unwrap_or(0);
    let avg_batches = if total > 0 { frames.iter().map(|f| f.batches as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let avg_tris = if total > 0 { frames.iter().map(|f| f.triangles as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let max_tris = frames.iter().map(|f| f.triangles).max().unwrap_or(0);
    let avg_sp = if total > 0 { frames.iter().map(|f| f.set_pass_calls as f32).sum::<f32>() / total as f32 } else { 0.0 };
    let batching_eff = if avg_dc > 0.0 { 1.0 - (avg_batches / avg_dc) } else { 0.0 };

    let rendering_analysis = RenderingAnalysis {
        avg_draw_calls: avg_dc,
        max_draw_calls: max_dc,
        avg_batches,
        avg_triangles: avg_tris,
        max_triangles: max_tris,
        avg_set_pass: avg_sp,
        batching_efficiency: batching_eff.max(0.0),
    };

    // Module Analysis
    let module_avg = |f_fn: fn(&FrameData) -> f32| -> f32 {
        if total > 0 { frames.iter().map(|f| f_fn(f)).sum::<f32>() / total as f32 } else { 0.0 }
    };
    let module_max = |f_fn: fn(&FrameData) -> f32| -> f32 {
        frames.iter().map(|f| f_fn(f)).fold(0.0f32, f32::max)
    };

    let avg_render = module_avg(|f| f.render_time);
    let avg_scripts = module_avg(|f| f.scripts_update_time + f.scripts_late_update_time + f.fixed_update_time);
    let avg_physics = module_avg(|f| f.physics_time);
    let avg_anim = module_avg(|f| f.animation_time);
    let avg_ui = module_avg(|f| f.ui_time);
    let avg_particle = module_avg(|f| f.particle_time);
    let avg_loading = module_avg(|f| f.loading_time);
    let avg_gc = module_avg(|f| f.gc_collect_time);

    let total_module_time = avg_render + avg_scripts + avg_physics + avg_anim + avg_ui + avg_particle + avg_loading + avg_gc;

    let modules = vec![
        ("Rendering", avg_render, module_max(|f| f.render_time)),
        ("Scripts", avg_scripts, module_max(|f| f.scripts_update_time + f.scripts_late_update_time + f.fixed_update_time)),
        ("Physics", avg_physics, module_max(|f| f.physics_time)),
        ("Animation", avg_anim, module_max(|f| f.animation_time)),
        ("UI", avg_ui, module_max(|f| f.ui_time)),
        ("Particles", avg_particle, module_max(|f| f.particle_time)),
        ("Loading", avg_loading, module_max(|f| f.loading_time)),
        ("GC", avg_gc, module_max(|f| f.gc_collect_time)),
    ];

    let bottleneck = modules.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(name, _, _)| name.to_string())
        .unwrap_or_else(|| "Unknown".into());

    let module_breakdown: Vec<ModuleBreakdown> = modules.iter().map(|(name, avg, max)| {
        ModuleBreakdown {
            name: name.to_string(),
            avg_ms: *avg,
            max_ms: *max,
            percentage: if total_module_time > 0.0 { *avg / total_module_time * 100.0 } else { 0.0 },
        }
    }).collect();

    let module_analysis = ModuleAnalysis {
        avg_render_ms: avg_render,
        avg_scripts_ms: avg_scripts,
        avg_physics_ms: avg_physics,
        avg_animation_ms: avg_anim,
        avg_ui_ms: avg_ui,
        avg_particle_ms: avg_particle,
        avg_loading_ms: avg_loading,
        avg_gc_ms: avg_gc,
        bottleneck,
        module_breakdown,
    };

    // Jank Analysis
    let worst_frame = frames.iter().enumerate()
        .max_by(|(_, a), (_, b)| a.delta_time.partial_cmp(&b.delta_time).unwrap_or(std::cmp::Ordering::Equal));
    let (worst_idx, worst_ms) = worst_frame
        .map(|(i, f)| (i as u32, f.delta_time * 1000.0))
        .unwrap_or((0, 0.0));

    let jank_timeline: Vec<TimelinePoint> = frames.iter()
        .enumerate()
        .filter(|(_, f)| f.jank_level >= 1)
        .map(|(idx, f)| TimelinePoint { time: f.timestamp, value: f.delta_time * 1000.0, frame_index: Some(idx as u32) })
        .collect();

    let jank_analysis = JankAnalysis {
        total_jank_frames: jank_count,
        severe_jank_frames: severe_jank_count,
        jank_rate_pct: jank_rate,
        severe_jank_rate_pct: if total > 0 { severe_jank_count as f32 / total as f32 * 100.0 } else { 0.0 },
        worst_frame_ms: worst_ms,
        worst_frame_index: worst_idx,
        jank_timeline,
    };

    // Thermal Analysis
    let temps: Vec<f32> = frames.iter().map(|f| f.temperature).filter(|t| *t > 0.0).collect();
    let has_thermal = !temps.is_empty();
    let avg_temp = if has_thermal { temps.iter().sum::<f32>() / temps.len() as f32 } else { 0.0 };
    let max_temp = temps.iter().cloned().fold(0.0f32, f32::max);
    let battery_start = frames.first().map(|f| f.battery_level).unwrap_or(0.0);
    let battery_end = frames.last().map(|f| f.battery_level).unwrap_or(0.0);
    let battery_drain = if battery_start > 0.0 && battery_end > 0.0 { battery_start - battery_end } else { 0.0 };

    let throttle_risk = if max_temp > 45.0 { "high" } else if max_temp > 38.0 { "medium" } else { "low" };

    let temp_timeline: Vec<TimelinePoint> = frames.iter()
        .enumerate()
        .step_by(sample_step)
        .filter(|(_, f)| f.temperature > 0.0)
        .map(|(idx, f)| TimelinePoint { time: f.timestamp, value: f.temperature, frame_index: Some(idx as u32) })
        .collect();

    let thermal_analysis = ThermalAnalysis {
        has_data: has_thermal,
        avg_temperature: avg_temp,
        max_temperature: max_temp,
        battery_drain,
        temperature_timeline: temp_timeline,
        thermal_throttle_risk: throttle_risk.to_string(),
    };

    // Overdraw Analysis
    let overdraw_analysis = if !session.overdraw_samples.is_empty() {
        let avg_od = session.overdraw_samples.iter().map(|s| s.avg_overdraw_layers).sum::<f32>()
            / session.overdraw_samples.len() as f32;
        let max_od = session.overdraw_samples.iter().map(|s| s.avg_overdraw_layers).fold(0.0f32, f32::max);
        Some(OverdrawAnalysis {
            avg_overdraw: avg_od,
            max_overdraw: max_od,
            sample_count: session.overdraw_samples.len(),
        })
    } else {
        None
    };

    // Function Analysis (V2)
    let function_analysis = generate_function_analysis(session);

    // Log Analysis (V2)
    let log_analysis = generate_log_analysis(session);

    // Scene Breakdown
    let mut scene_map: HashMap<u16, Vec<&FrameData>> = HashMap::new();
    for frame in frames {
        scene_map.entry(frame.scene_index).or_default().push(frame);
    }
    let scene_breakdown: Vec<SceneStats> = scene_map.iter().map(|(idx, scene_frames)| {
        let name = session.string_table.get(*idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("Scene_{}", idx));
        let count = scene_frames.len() as u32;
        let avg_scene_fps = scene_frames.iter().map(|f| f.fps).sum::<f32>() / count as f32;
        let avg_scene_mem = scene_frames.iter().map(|f| f.total_allocated as f32).sum::<f32>() / count as f32 / (1024.0 * 1024.0);
        let jank = scene_frames.iter().filter(|f| f.jank_level >= 1).count() as u32;
        SceneStats { scene_name: name, frame_count: count, avg_fps: avg_scene_fps, avg_memory_mb: avg_scene_mem, jank_count: jank }
    }).collect();

    // Overall Grade
    let grade = compute_grade(&summary, &jank_analysis, &thermal_analysis);

    let screenshot_frame_indices: Vec<u32> = session.screenshots.iter().map(|s| s.frame_index).collect();

    DeviceProfileReport {
        session_name: session_name.to_string(),
        source_file_path: None,
        device_info: session.device_info.clone(),
        duration_seconds: session.header.duration,
        total_frames: total,
        summary,
        fps_analysis,
        memory_analysis,
        rendering_analysis,
        module_analysis,
        jank_analysis,
        thermal_analysis,
        overdraw_analysis,
        function_analysis,
        log_analysis,
        scene_breakdown,
        overall_grade: grade,
        screenshot_count: session.screenshots.len(),
        screenshot_frame_indices,
    }
}

fn generate_function_analysis(session: &GaprofSession) -> Option<FunctionAnalysis> {
    if session.function_samples.is_empty() {
        return None;
    }

    let sampled_frames: Vec<&Vec<FunctionSample>> = session.function_samples.iter()
        .filter(|f| !f.is_empty())
        .collect();

    if sampled_frames.is_empty() {
        return None;
    }

    let total_sampled = sampled_frames.len() as u32;

    // Aggregate per-function stats: key = name index
    struct Accum {
        self_time_sum: f64,
        total_time_sum: f64,
        call_count_sum: u64,
        frames_called: u32,
        category: FunctionCategory,
    }
    let mut stats_map: HashMap<u16, Accum> = HashMap::new();

    for frame_samples in &sampled_frames {
        // Track which functions appeared in this frame
        let mut seen_this_frame: std::collections::HashSet<u16> = std::collections::HashSet::new();
        for s in frame_samples.iter() {
            let entry = stats_map.entry(s.function_name_index).or_insert_with(|| Accum {
                self_time_sum: 0.0,
                total_time_sum: 0.0,
                call_count_sum: 0,
                frames_called: 0,
                category: s.category,
            });
            entry.self_time_sum += s.self_time_ms as f64;
            entry.total_time_sum += s.total_time_ms as f64;
            entry.call_count_sum += s.call_count as u64;
            if seen_this_frame.insert(s.function_name_index) {
                entry.frames_called += 1;
            }
        }
    }

    // Compute total self time across all functions for percentage
    let grand_total_self: f64 = stats_map.values().map(|a| a.self_time_sum).sum();
    let grand_total_total: f64 = stats_map.values().map(|a| a.total_time_sum).sum();

    // Build sorted function stats
    let mut func_stats: Vec<FunctionStats> = stats_map.iter().map(|(name_idx, acc)| {
        let name = session.string_table.get(*name_idx as usize)
            .cloned()
            .unwrap_or_else(|| format!("Function_{}", name_idx));
        FunctionStats {
            name,
            category: acc.category.label().to_string(),
            avg_self_ms: acc.self_time_sum as f32 / total_sampled as f32,
            total_self_ms: acc.self_time_sum as f32,
            self_pct: if grand_total_self > 0.0 { (acc.self_time_sum / grand_total_self * 100.0) as f32 } else { 0.0 },
            avg_total_ms: acc.total_time_sum as f32 / total_sampled as f32,
            total_total_ms: acc.total_time_sum as f32,
            total_pct: if grand_total_total > 0.0 { (acc.total_time_sum / grand_total_total * 100.0) as f32 } else { 0.0 },
            avg_call_count: acc.call_count_sum as f32 / total_sampled as f32,
            total_call_count: acc.call_count_sum,
            frames_called: acc.frames_called,
        }
    }).collect();
    func_stats.sort_by(|a, b| b.total_self_ms.partial_cmp(&a.total_self_ms).unwrap_or(std::cmp::Ordering::Equal));
    let top_functions: Vec<FunctionStats> = func_stats.into_iter().take(50).collect();

    // Category breakdown
    let mut cat_map: HashMap<FunctionCategory, (f64, usize)> = HashMap::new(); // (total_self_ms, fn_count)
    for (_, acc) in &stats_map {
        let entry = cat_map.entry(acc.category).or_insert((0.0, 0));
        entry.0 += acc.self_time_sum;
        entry.1 += 1;
    }
    let cat_total: f64 = cat_map.values().map(|v| v.0).sum();
    let mut category_breakdown: Vec<CategoryBreakdown> = cat_map.iter().map(|(cat, (total_ms, fn_count))| {
        CategoryBreakdown {
            category: cat.label().to_string(),
            avg_ms: *total_ms as f32 / total_sampled as f32,
            total_ms: *total_ms as f32,
            percentage: if cat_total > 0.0 { (*total_ms / cat_total * 100.0) as f32 } else { 0.0 },
            function_count: *fn_count,
        }
    }).collect();
    category_breakdown.sort_by(|a, b| b.total_ms.partial_cmp(&a.total_ms).unwrap_or(std::cmp::Ordering::Equal));

    // Per-frame function detail is loaded on demand via get_frame_functions.
    // Keeping it out of the main report payload avoids huge Tauri responses
    // and oversized history JSON when deep captures contain thousands of samples per frame.
    let per_frame_data: Vec<PerFrameFunctions> = Vec::new();

    Some(FunctionAnalysis {
        has_data: true,
        total_sampled_frames: total_sampled,
        top_functions,
        category_breakdown,
        per_frame_data,
    })
}

fn generate_log_analysis(session: &GaprofSession) -> Option<LogAnalysis> {
    if session.log_entries.is_empty() {
        return None;
    }

    let info_count = session.log_entries.iter().filter(|l| is_info_log_type(l.log_type)).count();
    let error_count = session.log_entries.iter().filter(|l| is_error_log_type(l.log_type)).count();
    let warning_count = session.log_entries.iter().filter(|l| is_warning_log_type(l.log_type)).count();
    let exception_count = session.log_entries.iter().filter(|l| is_exception_log_type(l.log_type)).count();

    let mut info_groups: HashMap<String, (usize, i32, u8)> = HashMap::new();
    for log in session.log_entries.iter().filter(|l| is_info_log_type(l.log_type)) {
        let key = log.message.lines().next().unwrap_or(&log.message).to_string();
        let entry = info_groups.entry(key).or_insert((0, log.frame_index, log.log_type));
        entry.0 += 1;
    }
    let mut top_info: Vec<LogSummaryEntry> = info_groups.into_iter().map(|(msg, (count, first_frame, lt))| {
        LogSummaryEntry { message: msg, count, first_frame, log_type: lt }
    }).collect();
    top_info.sort_by(|a, b| b.count.cmp(&a.count));
    top_info.truncate(20);

    // Group errors by message (first line)
    let mut error_groups: HashMap<String, (usize, i32, u8)> = HashMap::new();
    for log in session.log_entries.iter().filter(|l| is_error_log_type(l.log_type) || is_exception_log_type(l.log_type)) {
        let key = log.message.lines().next().unwrap_or(&log.message).to_string();
        let entry = error_groups.entry(key).or_insert((0, log.frame_index, log.log_type));
        entry.0 += 1;
    }
    let mut top_errors: Vec<LogSummaryEntry> = error_groups.into_iter().map(|(msg, (count, first_frame, lt))| {
        LogSummaryEntry { message: msg, count, first_frame, log_type: lt }
    }).collect();
    top_errors.sort_by(|a, b| b.count.cmp(&a.count));
    top_errors.truncate(20);

    // Group warnings
    let mut warn_groups: HashMap<String, (usize, i32, u8)> = HashMap::new();
    for log in session.log_entries.iter().filter(|l| is_warning_log_type(l.log_type)) {
        let key = log.message.lines().next().unwrap_or(&log.message).to_string();
        let entry = warn_groups.entry(key).or_insert((0, log.frame_index, log.log_type));
        entry.0 += 1;
    }
    let mut top_warnings: Vec<LogSummaryEntry> = warn_groups.into_iter().map(|(msg, (count, first_frame, lt))| {
        LogSummaryEntry { message: msg, count, first_frame, log_type: lt }
    }).collect();
    top_warnings.sort_by(|a, b| b.count.cmp(&a.count));
    top_warnings.truncate(20);

    Some(LogAnalysis {
        has_data: true,
        total_logs: session.log_entries.len(),
        info_count,
        error_count,
        warning_count,
        exception_count,
        top_info,
        top_errors,
        top_warnings,
    })
}

fn is_error_log_type(log_type: u8) -> bool {
    matches!(log_type, 0 | 1)
}

fn is_warning_log_type(log_type: u8) -> bool {
    log_type == 2
}

fn is_info_log_type(log_type: u8) -> bool {
    log_type == 3
}

fn is_exception_log_type(log_type: u8) -> bool {
    log_type == 4
}

fn compute_grade(
    summary: &PerformanceSummary,
    jank: &JankAnalysis,
    thermal: &ThermalAnalysis,
) -> String {
    let mut score: f32 = 100.0;

    // FPS scoring (40 points)
    if summary.avg_fps < 30.0 { score -= 40.0; }
    else if summary.avg_fps < 45.0 { score -= 25.0; }
    else if summary.avg_fps < 55.0 { score -= 10.0; }
    else if summary.avg_fps < 58.0 { score -= 5.0; }

    // FPS stability (15 points)
    score -= (1.0 - summary.fps_stability) * 15.0;

    // Jank (20 points)
    if jank.jank_rate_pct > 10.0 { score -= 20.0; }
    else if jank.jank_rate_pct > 5.0 { score -= 12.0; }
    else if jank.jank_rate_pct > 2.0 { score -= 6.0; }
    else if jank.jank_rate_pct > 0.5 { score -= 3.0; }

    // Memory (15 points)
    if summary.total_gc_alloc_mb > 100.0 { score -= 15.0; }
    else if summary.total_gc_alloc_mb > 50.0 { score -= 8.0; }
    else if summary.total_gc_alloc_mb > 20.0 { score -= 4.0; }

    // Thermal (10 points)
    if thermal.max_temperature > 45.0 { score -= 10.0; }
    else if thermal.max_temperature > 40.0 { score -= 5.0; }

    score = score.max(0.0);

    if score >= 95.0 { "SSS".into() }
    else if score >= 90.0 { "SS".into() }
    else if score >= 85.0 { "S".into() }
    else if score >= 80.0 { "A".into() }
    else if score >= 70.0 { "B".into() }
    else if score >= 60.0 { "C".into() }
    else { "D".into() }
}

// ======================== Markdown Export ========================

pub fn export_device_report_markdown(report: &DeviceProfileReport) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Device Profile Report: {}\n\n", report.session_name));
    md.push_str(&format!("**Overall Grade: {}**\n\n", report.overall_grade));

    // Device Info
    md.push_str("## Device Information\n\n");
    md.push_str(&format!("| Property | Value |\n|---|---|\n"));
    md.push_str(&format!("| Device | {} |\n", report.device_info.device_model));
    md.push_str(&format!("| OS | {} |\n", report.device_info.operating_system));
    md.push_str(&format!("| CPU | {} ({} cores @ {}MHz) |\n", report.device_info.processor_type, report.device_info.processor_count, report.device_info.processor_frequency));
    md.push_str(&format!("| GPU | {} ({}MB) |\n", report.device_info.graphics_device_name, report.device_info.graphics_memory_mb));
    md.push_str(&format!("| RAM | {}MB |\n", report.device_info.system_memory_mb));
    md.push_str(&format!("| Screen | {}x{} @ {:.0}dpi |\n", report.device_info.screen_width, report.device_info.screen_height, report.device_info.screen_dpi));
    md.push_str(&format!("| Unity | {} |\n", report.device_info.unity_version));
    md.push_str(&format!("| Duration | {:.1}s ({} frames) |\n\n", report.duration_seconds, report.total_frames));

    // Summary
    md.push_str("## Performance Summary\n\n");
    md.push_str(&format!("| Metric | Value |\n|---|---|\n"));
    md.push_str(&format!("| Avg FPS | {:.1} |\n", report.summary.avg_fps));
    md.push_str(&format!("| P1 / P5 / P50 / P95 FPS | {:.0} / {:.0} / {:.0} / {:.0} |\n", report.summary.p1_fps, report.summary.p5_fps, report.summary.p50_fps, report.summary.p95_fps));
    md.push_str(&format!("| FPS Stability | {:.1}% |\n", report.summary.fps_stability * 100.0));
    md.push_str(&format!("| Avg CPU / GPU | {:.1}ms / {:.1}ms |\n", report.summary.avg_cpu_ms, report.summary.avg_gpu_ms));
    md.push_str(&format!("| Peak Memory | {:.1}MB |\n", report.summary.peak_memory_mb));
    md.push_str(&format!("| GC Alloc Total | {:.1}MB |\n", report.summary.total_gc_alloc_mb));
    md.push_str(&format!("| Jank Rate | {:.1}% ({} frames) |\n", report.summary.jank_rate, report.summary.jank_count));
    md.push_str(&format!("| Severe Jank | {} frames |\n\n", report.summary.severe_jank_count));

    // FPS Analysis
    md.push_str("## FPS Analysis\n\n");
    md.push_str(&format!("- Target FPS: {}\n", report.fps_analysis.target_fps));
    md.push_str(&format!(
        "- Below Target: {} frames ({:.1}%)\n",
        report.fps_analysis.frames_below_target,
        report.fps_analysis.below_target_pct
    ));
    md.push_str(&format!(
        "- Below 30 FPS: {} frames ({:.1}%)\n\n",
        report.fps_analysis.frames_below_30,
        report.fps_analysis.below_30_pct
    ));
    md.push_str("| Bucket | Count | Percentage |\n|---|---:|---:|\n");
    for bucket in &report.fps_analysis.fps_histogram {
        md.push_str(&format!(
            "| {} | {} | {:.1}% |\n",
            bucket.label, bucket.count, bucket.percentage
        ));
    }
    md.push('\n');

    // Module Breakdown
    md.push_str("## Module Breakdown\n\n");
    md.push_str("| Module | Avg (ms) | Max (ms) | % |\n|---|---|---|---|\n");
    for m in &report.module_analysis.module_breakdown {
        md.push_str(&format!("| {} | {:.2} | {:.2} | {:.1}% |\n", m.name, m.avg_ms, m.max_ms, m.percentage));
    }
    md.push_str(&format!("\n**Bottleneck: {}**\n\n", report.module_analysis.bottleneck));

    // Memory
    md.push_str("## Memory\n\n");
    md.push_str(&format!("- Trend: **{}** ({:+.1} MB/min)\n", report.memory_analysis.memory_trend, report.memory_analysis.memory_growth_rate_mb_per_min));
    md.push_str(&format!("- Peak Total: {:.1}MB, Peak Mono: {:.1}MB, Peak GFX: {:.1}MB\n", report.memory_analysis.peak_total_mb, report.memory_analysis.peak_mono_mb, report.memory_analysis.peak_gfx_mb));
    md.push_str(&format!("- Average Total Memory: {:.1}MB\n", report.memory_analysis.avg_total_mb));
    md.push_str(&format!("- GC Alloc/Frame: {:.0} bytes\n\n", report.memory_analysis.gc_alloc_per_frame_bytes));

    // Rendering
    md.push_str("## Rendering\n\n");
    md.push_str(&format!("- Draw Calls: avg={:.0}, max={}\n", report.rendering_analysis.avg_draw_calls, report.rendering_analysis.max_draw_calls));
    md.push_str(&format!("- Batches: avg={:.0}\n", report.rendering_analysis.avg_batches));
    md.push_str(&format!("- Triangles: avg={:.0}, max={}\n", report.rendering_analysis.avg_triangles, report.rendering_analysis.max_triangles));
    md.push_str(&format!("- SetPass Calls: avg={:.0}\n", report.rendering_analysis.avg_set_pass));
    md.push_str(&format!("- Batching Efficiency: {:.1}%\n\n", report.rendering_analysis.batching_efficiency * 100.0));

    // Jank
    md.push_str("## Jank Analysis\n\n");
    md.push_str(&format!(
        "- Jank Frames: {} ({:.1}%)\n",
        report.jank_analysis.total_jank_frames,
        report.jank_analysis.jank_rate_pct
    ));
    md.push_str(&format!(
        "- Severe Jank Frames: {} ({:.1}%)\n",
        report.jank_analysis.severe_jank_frames,
        report.jank_analysis.severe_jank_rate_pct
    ));
    md.push_str(&format!(
        "- Worst Frame: {:.1}ms @ frame #{}\n\n",
        report.jank_analysis.worst_frame_ms,
        report.jank_analysis.worst_frame_index
    ));

    // Thermal
    if report.thermal_analysis.has_data {
        md.push_str("## Thermal & Battery\n\n");
        md.push_str(&format!("- Avg Temp: {:.1}°C, Max: {:.1}°C\n", report.thermal_analysis.avg_temperature, report.thermal_analysis.max_temperature));
        md.push_str(&format!("- Throttle Risk: **{}**\n", report.thermal_analysis.thermal_throttle_risk));
        md.push_str(&format!("- Battery Drain: {:.1}%\n\n", report.thermal_analysis.battery_drain * 100.0));
    }

    // Overdraw
    if let Some(od) = &report.overdraw_analysis {
        md.push_str("## Overdraw\n\n");
        md.push_str(&format!("- Avg Overdraw: {:.2}x\n", od.avg_overdraw));
        md.push_str(&format!("- Max Overdraw: {:.2}x\n", od.max_overdraw));
        md.push_str(&format!("- Sample Count: {}\n\n", od.sample_count));
    }

    // Function Analysis (V2)
    if let Some(fa) = &report.function_analysis {
        md.push_str("## Function-Level Analysis (Deep Profiling)\n\n");
        md.push_str(&format!("Sampled Frames: {}\n\n", fa.total_sampled_frames));

        md.push_str("### Category Breakdown\n\n");
        md.push_str("| Category | Avg (ms) | Total (ms) | % | Functions |\n|---|---:|---:|---:|---:|\n");
        for cat in &fa.category_breakdown {
            md.push_str(&format!("| {} | {:.2} | {:.1} | {:.1}% | {} |\n", cat.category, cat.avg_ms, cat.total_ms, cat.percentage, cat.function_count));
        }
        md.push('\n');

        md.push_str("### Top Functions (by Self Time)\n\n");
        md.push_str("| Function | Category | Avg Self (ms) | Total Self (ms) | Self% | Avg Calls | Frames |\n|---|---|---:|---:|---:|---:|---:|\n");
        for f in fa.top_functions.iter().take(30) {
            md.push_str(&format!("| {} | {} | {:.3} | {:.1} | {:.1}% | {:.1} | {} |\n",
                f.name, f.category, f.avg_self_ms, f.total_self_ms, f.self_pct, f.avg_call_count, f.frames_called));
        }
        md.push('\n');
    }

    // Log Analysis (V2)
    if let Some(la) = &report.log_analysis {
        md.push_str("## Runtime Logs\n\n");
        md.push_str(&format!("- Total Logs: {}\n", la.total_logs));
        md.push_str(&format!("- Errors: {}\n", la.error_count));
        md.push_str(&format!("- Warnings: {}\n", la.warning_count));
        md.push_str(&format!("- Exceptions: {}\n\n", la.exception_count));

        if !la.top_errors.is_empty() {
            md.push_str("### Top Errors\n\n");
            md.push_str("| Message | Count | First Frame |\n|---|---:|---:|\n");
            for e in la.top_errors.iter().take(10) {
                let msg_short: String = e.message.chars().take(80).collect();
                md.push_str(&format!("| {} | {} | #{} |\n", msg_short, e.count, e.first_frame));
            }
            md.push('\n');
        }
    }

    // Scene Breakdown
    if !report.scene_breakdown.is_empty() {
        md.push_str("## Scene Breakdown\n\n");
        md.push_str("| Scene | Frames | Avg FPS | Avg Memory | Jank |\n|---|---:|---:|---:|---:|\n");
        for scene in &report.scene_breakdown {
            md.push_str(&format!(
                "| {} | {} | {:.1} | {:.1}MB | {} |\n",
                scene.scene_name,
                scene.frame_count,
                scene.avg_fps,
                scene.avg_memory_mb,
                scene.jank_count
            ));
        }
        md.push('\n');
    }

    // Screenshots
    if report.screenshot_count > 0 {
        md.push_str("## Screenshots\n\n");
        md.push_str(&format!("- Screenshot Count: {}\n", report.screenshot_count));
        if !report.screenshot_frame_indices.is_empty() {
            let frames = report
                .screenshot_frame_indices
                .iter()
                .map(|idx| format!("#{}", idx))
                .collect::<Vec<_>>()
                .join(", ");
            md.push_str(&format!("- Frames: {}\n", frames));
        }
        md.push_str("\n> Screenshot image binaries are not embedded in the Markdown export. Use the desktop report view to inspect image content.\n\n");
    }

    md.push_str("---\n\n");
    md.push_str("*This Markdown export is a text summary of the desktop report. Interactive charts and raw screenshots are available in the desktop UI.*\n");

    md
}

// ======================== AI Report Generation ========================

pub fn build_ai_prompt(report: &DeviceProfileReport) -> String {
    let mut prompt = String::new();
    prompt.push_str("You are a mobile game performance expert. Analyze this device profiling report and provide actionable optimization recommendations.\n\n");
    prompt.push_str(&format!("## Device: {} ({})\n", report.device_info.device_model, report.device_info.operating_system));
    prompt.push_str(&format!("## Overall Grade: {}\n", report.overall_grade));
    prompt.push_str(&format!("## Duration: {:.1}s, Frames: {}\n\n", report.duration_seconds, report.total_frames));

    prompt.push_str("### Key Metrics\n");
    prompt.push_str(&format!("- FPS: avg={:.1}, P1={:.0}, P5={:.0}, stability={:.1}%\n", report.summary.avg_fps, report.summary.p1_fps, report.summary.p5_fps, report.summary.fps_stability * 100.0));
    prompt.push_str(&format!("- CPU: avg={:.1}ms, GPU: avg={:.1}ms\n", report.summary.avg_cpu_ms, report.summary.avg_gpu_ms));
    prompt.push_str(&format!("- Memory: peak={:.1}MB, GC alloc={:.1}MB total, {:.0} bytes/frame\n", report.summary.peak_memory_mb, report.summary.total_gc_alloc_mb, report.memory_analysis.gc_alloc_per_frame_bytes));
    prompt.push_str(&format!("- Memory trend: {} ({:+.1} MB/min)\n", report.memory_analysis.memory_trend, report.memory_analysis.memory_growth_rate_mb_per_min));
    prompt.push_str(&format!("- Jank: {}% ({} frames), severe: {} frames\n", report.summary.jank_rate, report.summary.jank_count, report.summary.severe_jank_count));
    prompt.push_str(&format!("- Bottleneck module: {}\n", report.module_analysis.bottleneck));

    if report.thermal_analysis.has_data {
        prompt.push_str(&format!("- Temperature: avg={:.1}°C, max={:.1}°C, throttle risk={}\n", report.thermal_analysis.avg_temperature, report.thermal_analysis.max_temperature, report.thermal_analysis.thermal_throttle_risk));
        prompt.push_str(&format!("- Battery drain: {:.1}%\n", report.thermal_analysis.battery_drain * 100.0));
    }

    prompt.push_str("\n### Module Breakdown (avg ms)\n");
    for m in &report.module_analysis.module_breakdown {
        prompt.push_str(&format!("- {}: avg={:.2}ms, max={:.2}ms ({:.1}%)\n", m.name, m.avg_ms, m.max_ms, m.percentage));
    }

    prompt.push_str("\n### Rendering\n");
    prompt.push_str(&format!("- DrawCalls: avg={:.0}, max={}\n", report.rendering_analysis.avg_draw_calls, report.rendering_analysis.max_draw_calls));
    prompt.push_str(&format!("- Triangles: avg={:.0}, max={}\n", report.rendering_analysis.avg_triangles, report.rendering_analysis.max_triangles));
    prompt.push_str(&format!("- Batching efficiency: {:.1}%\n", report.rendering_analysis.batching_efficiency * 100.0));

    if let Some(od) = &report.overdraw_analysis {
        prompt.push_str(&format!("- Overdraw: avg={:.2}x, max={:.2}x\n", od.avg_overdraw, od.max_overdraw));
    }

    if !report.scene_breakdown.is_empty() {
        prompt.push_str("\n### Per-Scene\n");
        for s in &report.scene_breakdown {
            prompt.push_str(&format!("- {}: {} frames, avg FPS={:.1}, mem={:.1}MB, jank={}\n", s.scene_name, s.frame_count, s.avg_fps, s.avg_memory_mb, s.jank_count));
        }
    }

    // V2: Function-level data
    if let Some(fa) = &report.function_analysis {
        prompt.push_str("\n### Function Profiling (Deep Analysis)\n");
        prompt.push_str(&format!("Sampled {} frames with function-level timing.\n", fa.total_sampled_frames));

        prompt.push_str("\nCategory breakdown:\n");
        for cat in &fa.category_breakdown {
            prompt.push_str(&format!("- {}: avg={:.2}ms, {:.1}%\n", cat.category, cat.avg_ms, cat.percentage));
        }

        prompt.push_str("\nTop 20 functions by self-time:\n");
        for f in fa.top_functions.iter().take(20) {
            prompt.push_str(&format!("- {} [{}]: selfAvg={:.3}ms, selfTotal={:.1}ms ({:.1}%), calls={:.1}/frame\n",
                f.name, f.category, f.avg_self_ms, f.total_self_ms, f.self_pct, f.avg_call_count));
        }
    }

    // V2: Log data
    if let Some(la) = &report.log_analysis {
        prompt.push_str(&format!("\n### Runtime Logs: {} total ({} errors, {} warnings, {} exceptions)\n",
            la.total_logs, la.error_count, la.warning_count, la.exception_count));
        if !la.top_errors.is_empty() {
            prompt.push_str("Top errors:\n");
            for e in la.top_errors.iter().take(10) {
                let msg_short: String = e.message.chars().take(100).collect();
                prompt.push_str(&format!("- [x{}] {}\n", e.count, msg_short));
            }
        }
    }

    prompt.push_str("\nPlease provide:\n1. Overall assessment\n2. Top 5 optimization priorities (ranked by impact)\n3. Specific actionable recommendations for each issue\n4. Expected improvement estimates\n\nRespond in the user's language. Use Markdown formatting.");
    prompt
}
