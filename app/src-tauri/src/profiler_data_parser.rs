// profiler_data_parser.rs — Unity Profiler binary log (.data / .raw) parser.
// Parses files created by Profiler.logFile + Profiler.enableBinaryLog to extract
// complete call hierarchy including user script functions at full depth.
// Converts deep samples to the existing FunctionSample format for reuse by
// call_tree.rs, module_analysis.rs, and all frontend analysis pipelines.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::device_profile::{FrameData, FunctionCategory, FunctionSample};

// ======================== Data Structures ========================

/// Complete parsed deep profile data from a Unity .data/.raw file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepProfileData {
    pub name_table: Vec<String>,
    pub thread_info: Vec<ThreadInfo>,
    pub frames: Vec<DeepFrame>,
    pub version: u32,
    pub platform: String,
}

/// Thread metadata from the profiler data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub thread_id: u64,
    pub thread_name: String,
    pub group_name: String,
}

/// A single profiler frame containing per-thread sample data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepFrame {
    pub frame_index: u32,
    pub start_time_ns: u64,
    pub duration_ns: u64,
    pub threads: Vec<DeepThreadData>,
}

/// Per-thread samples within a frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepThreadData {
    pub thread_index: u16,
    pub thread_name: String,
    pub samples: Vec<DeepSample>,
}

/// A single deep profiler sample (one marker invocation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSample {
    pub marker_name_index: u32,
    pub start_time_ns: u64,
    pub duration_ns: u64,
    pub depth: u8,
    pub gc_alloc_bytes: u64,
    pub category: u16,
}

// ======================== Block Types ========================

// Unity profiler binary log block type IDs
const BLOCK_NAME_TABLE: u16 = 1;
const BLOCK_THREAD_INFO: u16 = 2;
const BLOCK_FRAME_DATA: u16 = 3;
const BLOCK_GPU_DATA: u16 = 4;

// Legacy Unity profiler log magic bytes
const PROFILER_LOG_MAGIC: &[u8; 4] = b"prof";
// Newer profiler format magic observed from real deep captures
const PROFILER_LOG_MAGIC_PD3U: &[u8; 4] = b"PD3U";
// GameAnalytics Deep Profile exported by Unity Editor helper
const GADP_MAGIC: &[u8; 4] = b"GADP";

// ======================== Parser ========================

/// Parse a Unity profiler binary log file (.data or .raw).
pub fn parse_profiler_data(data: &[u8]) -> Result<DeepProfileData, String> {
    if data.len() < 16 {
        return Err("文件太小，不是有效的Unity Profiler数据文件".to_string());
    }

    let mut cursor = Cursor::new(data);

    // Read and validate magic
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic).map_err(|e| format!("读取文件头失败: {}", e))?;

    if &magic == GADP_MAGIC {
        cursor.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek: {}", e))?;
        return parse_gadp(data);
    }

    if &magic == PROFILER_LOG_MAGIC_PD3U {
        return Err(format!(
            "检测到新的 Unity Profiler 数据格式 'PD3U'，当前解析器只支持旧版 'prof' block 流格式。文件头十六进制={}",
            magic.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("")
        ));
    }

    // Unity profiler data files may have different magic sequences depending on version.
    // Some begin with "prof", others start directly with block data.
    // Try to detect the format.
    let (version, platform) = if &magic == PROFILER_LOG_MAGIC {
        let version = read_u32_le(&mut cursor)?;
        let platform_id = read_u32_le(&mut cursor)?;
        let platform = platform_id_to_string(platform_id);
        (version, platform)
    } else {
        // No recognized magic - try parsing from the beginning as raw block stream
        cursor.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek: {}", e))?;
        (0u32, "Unknown".to_string())
    };

    let mut name_table: Vec<String> = Vec::new();
    let mut thread_info: Vec<ThreadInfo> = Vec::new();
    let mut frames: Vec<DeepFrame> = Vec::new();
    let mut frame_index_counter: u32 = 0;
    let mut valid_block_count: usize = 0;
    let mut frame_block_count: usize = 0;
    let mut parsed_frame_count: usize = 0;

    // Parse block stream
    while (cursor.position() as usize) < data.len().saturating_sub(4) {
        // Try to read block header: type(u16) + size(u32)
        let block_type = match read_u16_le(&mut cursor) {
            Ok(v) => v,
            Err(_) => break,
        };
        let block_size = match read_u32_le(&mut cursor) {
            Ok(v) => v,
            Err(_) => break,
        };

        let block_start = cursor.position();
        let block_end = block_start + block_size as u64;

        if block_end > data.len() as u64 {
            // Truncated block — stop parsing
            break;
        }

        match block_type {
            BLOCK_NAME_TABLE => {
                name_table = parse_name_table_block(&data[block_start as usize..block_end as usize])?;
                valid_block_count += 1;
            }
            BLOCK_THREAD_INFO => {
                thread_info = parse_thread_info_block(&data[block_start as usize..block_end as usize])?;
                valid_block_count += 1;
            }
            BLOCK_FRAME_DATA => {
                frame_block_count += 1;
                if let Ok(frame) = parse_frame_data_block(
                    &data[block_start as usize..block_end as usize],
                    frame_index_counter,
                    &thread_info,
                ) {
                    frames.push(frame);
                    frame_index_counter += 1;
                    parsed_frame_count += 1;
                    valid_block_count += 1;
                }
            }
            BLOCK_GPU_DATA => {
                // Skip GPU data blocks for now
            }
            _ => {
                // Unknown block type — skip
            }
        }

        cursor.seek(SeekFrom::Start(block_end)).map_err(|e| format!("Seek: {}", e))?;
    }

    if valid_block_count == 0 || frames.is_empty() || (frame_block_count > 0 && parsed_frame_count == 0) {
        return Err("未能从深度数据文件解析出有效的帧数据".to_string());
    }

    Ok(DeepProfileData {
        name_table,
        thread_info,
        frames,
        version,
        platform,
    })
}

fn parse_gadp(data: &[u8]) -> Result<DeepProfileData, String> {
    let mut cursor = Cursor::new(data);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic).map_err(|e| format!("读取 GADP 头失败: {}", e))?;
    if &magic != GADP_MAGIC {
        return Err("Invalid GADP magic".to_string());
    }

    let version = read_u32_le(&mut cursor)?;
    let string_count = read_u32_le(&mut cursor)? as usize;
    let thread_count = read_u32_le(&mut cursor)? as usize;
    let frame_count = read_u32_le(&mut cursor)? as usize;

    let mut name_table = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        name_table.push(read_length_prefixed_string(&mut cursor)?);
    }

    let mut thread_info = Vec::with_capacity(thread_count);
    let mut thread_name_by_index: HashMap<u16, String> = HashMap::new();
    for _ in 0..thread_count {
        let thread_id = read_u64_le(&mut cursor)?;
        let thread_index = read_u16_le(&mut cursor)?;
        let thread_name_index = read_u32_le(&mut cursor)? as usize;
        let group_name_index = read_u32_le(&mut cursor)? as usize;
        let thread_name = name_table.get(thread_name_index).cloned().unwrap_or_else(|| format!("Thread_{}", thread_index));
        let group_name = name_table.get(group_name_index).cloned().unwrap_or_else(|| "Unknown".to_string());
        thread_name_by_index.insert(thread_index, thread_name.clone());
        thread_info.push(ThreadInfo {
            thread_id,
            thread_name,
            group_name,
        });
    }

    let mut frames = Vec::with_capacity(frame_count);
    for _ in 0..frame_count {
        let frame_index = read_u32_le(&mut cursor)?;
        let start_time_ns = read_u64_le(&mut cursor)?;
        let duration_ns = read_u64_le(&mut cursor)?;
        let per_frame_thread_count = read_u16_le(&mut cursor)? as usize;

        let mut threads = Vec::with_capacity(per_frame_thread_count);
        for _ in 0..per_frame_thread_count {
            let thread_index = read_u16_le(&mut cursor)?;
            let sample_count = read_u32_le(&mut cursor)? as usize;
            let thread_name = thread_name_by_index
                .get(&thread_index)
                .cloned()
                .unwrap_or_else(|| format!("Thread_{}", thread_index));

            let mut samples = Vec::with_capacity(sample_count);
            for _ in 0..sample_count {
                samples.push(DeepSample {
                    marker_name_index: read_u32_le(&mut cursor)?,
                    start_time_ns: read_u64_le(&mut cursor)?,
                    duration_ns: read_u64_le(&mut cursor)?,
                    depth: read_u8(&mut cursor)?,
                    gc_alloc_bytes: read_u64_le(&mut cursor)?,
                    category: read_u16_le(&mut cursor)?,
                });
            }

            threads.push(DeepThreadData {
                thread_index,
                thread_name,
                samples,
            });
        }

        frames.push(DeepFrame {
            frame_index,
            start_time_ns,
            duration_ns,
            threads,
        });
    }

    Ok(DeepProfileData {
        name_table,
        thread_info,
        frames,
        version,
        platform: "UnityEditorExport".to_string(),
    })
}

// ======================== Block Parsers ========================

fn parse_name_table_block(data: &[u8]) -> Result<Vec<String>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_u32_le(&mut cursor)? as usize;
    let mut names = Vec::with_capacity(count);

    for _ in 0..count {
        let name = read_length_prefixed_string(&mut cursor)?;
        names.push(name);
    }

    Ok(names)
}

fn parse_thread_info_block(data: &[u8]) -> Result<Vec<ThreadInfo>, String> {
    let mut cursor = Cursor::new(data);
    let count = read_u32_le(&mut cursor)? as usize;
    let mut threads = Vec::with_capacity(count);

    for _ in 0..count {
        let thread_id = read_u64_le(&mut cursor)?;
        let thread_name = read_length_prefixed_string(&mut cursor)?;
        let group_name = read_length_prefixed_string(&mut cursor)?;

        threads.push(ThreadInfo {
            thread_id,
            thread_name,
            group_name,
        });
    }

    Ok(threads)
}

fn parse_frame_data_block(
    data: &[u8],
    frame_index: u32,
    thread_info: &[ThreadInfo],
) -> Result<DeepFrame, String> {
    let mut cursor = Cursor::new(data);

    let start_time_ns = read_u64_le(&mut cursor)?;
    let duration_ns = read_u64_le(&mut cursor)?;

    let thread_count = read_u16_le(&mut cursor)? as usize;
    let mut threads = Vec::with_capacity(thread_count);

    for _ in 0..thread_count {
        let thread_idx = read_u16_le(&mut cursor)? as usize;
        let sample_count = read_u32_le(&mut cursor)? as usize;

        let thread_name = if thread_idx < thread_info.len() {
            thread_info[thread_idx].thread_name.clone()
        } else {
            format!("Thread_{}", thread_idx)
        };

        let mut samples = Vec::with_capacity(sample_count);
        for _ in 0..sample_count {
            let marker_name_index = read_u32_le(&mut cursor)?;
            let sample_start_ns = read_u64_le(&mut cursor)?;
            let sample_duration_ns = read_u64_le(&mut cursor)?;
            let depth = read_u8(&mut cursor)?;
            let gc_alloc = read_u64_le(&mut cursor)?;
            let category = read_u16_le(&mut cursor)?;

            samples.push(DeepSample {
                marker_name_index,
                start_time_ns: sample_start_ns,
                duration_ns: sample_duration_ns,
                depth,
                gc_alloc_bytes: gc_alloc,
                category,
            });
        }

        threads.push(DeepThreadData {
            thread_index: thread_idx as u16,
            thread_name,
            samples,
        });
    }

    Ok(DeepFrame {
        frame_index,
        start_time_ns,
        duration_ns,
        threads,
    })
}

// ======================== Conversion to FunctionSample ========================

/// Convert deep profile data to per-frame FunctionSample vectors compatible with
/// the existing .gaprof analysis pipeline.
pub fn convert_deep_to_function_samples(
    deep: &DeepProfileData,
    string_table: &mut Vec<String>,
) -> Vec<Vec<FunctionSample>> {
    // Build mapping from deep name_table indices to the shared string_table
    let mut name_index_map: HashMap<u32, u16> = HashMap::new();
    let mut string_to_index: HashMap<String, u16> = HashMap::new();

    // Pre-populate with existing string table
    for (i, s) in string_table.iter().enumerate() {
        string_to_index.insert(s.clone(), i as u16);
    }

    // Map thread names to thread indices: Main=0, Render=1, others=2+
    let thread_index_map = build_thread_index_map(&deep.thread_info);

    let mut result = Vec::with_capacity(deep.frames.len());

    for frame in &deep.frames {
        let mut frame_samples = Vec::new();

        for thread_data in &frame.threads {
            let thread_idx = thread_index_map
                .get(&thread_data.thread_name)
                .copied()
                .unwrap_or(thread_data.thread_index as u8);

            // Track parent indices using a depth stack
            let mut depth_stack: Vec<(u8, usize)> = Vec::new(); // (depth, sample_index_in_frame_samples)

            for sample in &thread_data.samples {
                // Resolve marker name
                let marker_name = if (sample.marker_name_index as usize) < deep.name_table.len() {
                    &deep.name_table[sample.marker_name_index as usize]
                } else {
                    continue; // Skip unknown markers
                };

                // Get or create string table index
                let name_idx = if let Some(&cached) = name_index_map.get(&sample.marker_name_index) {
                    cached
                } else {
                    let idx = if let Some(&existing) = string_to_index.get(marker_name) {
                        existing
                    } else {
                        if string_table.len() >= u16::MAX as usize {
                            continue; // String table full, skip this sample
                        }
                        let new_idx = string_table.len() as u16;
                        string_table.push(marker_name.clone());
                        string_to_index.insert(marker_name.clone(), new_idx);
                        new_idx
                    };
                    name_index_map.insert(sample.marker_name_index, idx);
                    idx
                };

                // Compute self time (total_time - sum of direct children times)
                // For now set self = total; we'll adjust after all samples are collected
                let total_ms = sample.duration_ns as f64 / 1_000_000.0;

                // Find parent using depth stack
                while let Some(&(d, _)) = depth_stack.last() {
                    if d >= sample.depth {
                        depth_stack.pop();
                    } else {
                        break;
                    }
                }

                let parent_index = depth_stack
                    .last()
                    .map(|&(_, idx)| idx as i16)
                    .unwrap_or(-1);

                let sample_idx = frame_samples.len();
                depth_stack.push((sample.depth, sample_idx));

                // Classify category from Unity profiler category ID
                let category = classify_profiler_category(sample.category, marker_name);

                frame_samples.push(FunctionSample {
                    function_name_index: name_idx,
                    category,
                    self_time_ms: total_ms as f32, // Will be adjusted below
                    total_time_ms: total_ms as f32,
                    call_count: 1,
                    depth: sample.depth,
                    parent_index,
                    thread_index: thread_idx,
                });
            }
        }

        // Adjust self_time: subtract direct children's total_time from parent
        compute_self_times(&mut frame_samples);

        result.push(frame_samples);
    }

    result
}

/// Compute self_time for each sample by subtracting direct children's total_time.
fn compute_self_times(samples: &mut Vec<FunctionSample>) {
    // Accumulate children total_time for each parent
    let mut children_time: HashMap<usize, f32> = HashMap::new();

    for (_i, sample) in samples.iter().enumerate() {
        if sample.parent_index >= 0 {
            let parent = sample.parent_index as usize;
            *children_time.entry(parent).or_insert(0.0) += sample.total_time_ms;
        }
    }

    // Adjust self time
    for (i, sample) in samples.iter_mut().enumerate() {
        if let Some(&child_total) = children_time.get(&i) {
            sample.self_time_ms = (sample.total_time_ms - child_total).max(0.0);
        }
    }
}

/// Map thread names to canonical thread indices.
fn build_thread_index_map(threads: &[ThreadInfo]) -> HashMap<String, u8> {
    let mut map = HashMap::new();
    let mut next_idx: u8 = 2;

    for ti in threads {
        let name_lower = ti.thread_name.to_lowercase();
        if name_lower.contains("main") || name_lower == "main thread" {
            map.insert(ti.thread_name.clone(), 0);
        } else if name_lower.contains("render") {
            map.insert(ti.thread_name.clone(), 1);
        } else {
            map.insert(ti.thread_name.clone(), next_idx);
            if next_idx < 255 {
                next_idx += 1;
            }
        }
    }

    map
}

/// Classify a Unity profiler category ID into our FunctionCategory enum.
fn classify_profiler_category(category_id: u16, marker_name: &str) -> FunctionCategory {
    // Unity ProfilerCategory constants (from Unity source)
    match category_id {
        0 => FunctionCategory::Rendering,  // Render
        1 => FunctionCategory::Scripting,  // Scripts
        2 => FunctionCategory::UI,         // GUI
        3 => FunctionCategory::Physics,    // Physics
        4 => FunctionCategory::Animation,  // Animation
        5 => FunctionCategory::Loading,    // Loading
        6 => FunctionCategory::GC,         // Memory / GC
        7 => FunctionCategory::Overhead,   // Internal
        8 => FunctionCategory::Particles,  // Particles (VFX)
        _ => {
            // Try to infer from marker name
            let name = marker_name.to_lowercase();
            if name.contains("render") || name.contains("camera") || name.contains("draw") || name.contains("culling") {
                FunctionCategory::Rendering
            } else if name.contains("physics") || name.contains("physx") {
                FunctionCategory::Physics
            } else if name.contains("animation") || name.contains("animator") || name.contains("director") {
                FunctionCategory::Animation
            } else if name.contains("canvas") || name.contains("ugui") || name.contains("ui.") || name.contains("layout") {
                FunctionCategory::UI
            } else if name.contains("loading") || name.contains("async") || name.contains("preload") {
                FunctionCategory::Loading
            } else if name.contains("particle") || name.contains("vfx") {
                FunctionCategory::Particles
            } else if name.contains("gc.") || name.contains("gc_") {
                FunctionCategory::GC
            } else if name.contains("gfx.wait") || name.contains("present") || name.contains("sync") {
                FunctionCategory::Sync
            } else {
                FunctionCategory::Scripting // Default to scripting for user functions
            }
        }
    }
}

// ======================== Merged Session Loading ========================

/// Load a deep profile and merge it with an existing GaprofSession.
/// The deep function samples replace the shallow ones from the .gaprof file,
/// while device info, frame data, screenshots, etc. are kept.
pub fn merge_deep_profile_into_session(
    session: &mut crate::device_profile::GaprofSession,
    deep_data_path: &str,
) -> Result<MergeResult, String> {
    let raw_data = std::fs::read(deep_data_path)
        .map_err(|e| format!("读取深度数据文件失败: {}", e))?;

    let deep = parse_profiler_data(&raw_data)?;

    let deep_frame_count = deep.frames.len();
    let deep_samples = convert_deep_to_function_samples(&deep, &mut session.string_table);
    let non_empty_deep_frames = deep_samples.iter().filter(|samples| !samples.is_empty()).count();
    if non_empty_deep_frames == 0 {
        return Err("深度数据文件解析成功，但没有可用的函数样本".to_string());
    }

    let gaprof_frame_count = session.frames.len();
    if gaprof_frame_count == 0 {
        return Err("基础 .gaprof 不包含任何帧数据，无法合并深度样本".to_string());
    }

    let mut merged_samples = session.function_samples.clone();
    while merged_samples.len() < gaprof_frame_count {
        merged_samples.push(Vec::new());
    }
    if merged_samples.len() > gaprof_frame_count {
        merged_samples.truncate(gaprof_frame_count);
    }

    let mapped_frames = align_and_merge_deep_samples(
        &deep,
        &deep_samples,
        &session.frames,
        &mut merged_samples,
    )?;
    if mapped_frames == 0 {
        return Err("深度数据未能与 .gaprof 帧时间线对齐，已保留原始浅层样本".to_string());
    }

    session.function_samples = merged_samples;

    Ok(MergeResult {
        deep_frame_count,
        replaced_sample_frames: mapped_frames,
        total_deep_samples: session.function_samples.iter()
            .map(|s| s.len())
            .sum(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub deep_frame_count: usize,
    pub replaced_sample_frames: usize,
    pub total_deep_samples: usize,
}

fn align_and_merge_deep_samples(
    deep: &DeepProfileData,
    deep_samples: &[Vec<FunctionSample>],
    session_frames: &[FrameData],
    merged_samples: &mut [Vec<FunctionSample>],
) -> Result<usize, String> {
    if deep.frames.len() != deep_samples.len() {
        return Err("深度帧和样本数量不一致".to_string());
    }

    let mut used_session_indices = vec![false; session_frames.len()];
    let mut replaced_frames = 0usize;

    let can_time_align = deep.frames.len() > 1
        && session_frames.len() > 1
        && deep.frames.windows(2).all(|w| w[0].start_time_ns <= w[1].start_time_ns);
    let deep_start_ns = deep.frames.first().map(|f| f.start_time_ns).unwrap_or(0);
    let session_times: Vec<f64> = session_frames.iter().map(|f| f.timestamp as f64).collect();

    for (deep_idx, samples) in deep_samples.iter().enumerate() {
        if samples.is_empty() {
            continue;
        }

        let preferred_idx = if can_time_align {
            let deep_time_sec = (deep.frames[deep_idx].start_time_ns.saturating_sub(deep_start_ns)) as f64 / 1_000_000_000.0;
            find_nearest_session_index(&session_times, deep_time_sec)
        } else if deep.frames.len() == 1 {
            0
        } else {
            (((deep_idx as f64) * ((session_frames.len() - 1) as f64)) / ((deep.frames.len() - 1) as f64)).round() as usize
        };

        let Some(session_idx) = pick_nearest_available_index(preferred_idx.min(session_frames.len() - 1), &used_session_indices) else {
            break;
        };

        merged_samples[session_idx] = samples.clone();
        used_session_indices[session_idx] = true;
        replaced_frames += 1;
    }

    Ok(replaced_frames)
}

fn find_nearest_session_index(session_times: &[f64], target_time: f64) -> usize {
    let mut best_idx = 0usize;
    let mut best_delta = f64::MAX;
    for (idx, time) in session_times.iter().enumerate() {
        let delta = (*time - target_time).abs();
        if delta < best_delta {
            best_delta = delta;
            best_idx = idx;
        }
    }
    best_idx
}

fn pick_nearest_available_index(preferred_idx: usize, used: &[bool]) -> Option<usize> {
    if used.is_empty() {
        return None;
    }
    if !used[preferred_idx] {
        return Some(preferred_idx);
    }

    for radius in 1..used.len() {
        if preferred_idx >= radius && !used[preferred_idx - radius] {
            return Some(preferred_idx - radius);
        }
        let right = preferred_idx + radius;
        if right < used.len() && !used[right] {
            return Some(right);
        }
    }
    None
}

// ======================== IO Helpers ========================

fn read_u8(cursor: &mut Cursor<&[u8]>) -> Result<u8, String> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read u8: {}", e))?;
    Ok(buf[0])
}

fn read_u16_le(cursor: &mut Cursor<&[u8]>) -> Result<u16, String> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read u16: {}", e))?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32_le(cursor: &mut Cursor<&[u8]>) -> Result<u32, String> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read u32: {}", e))?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64_le(cursor: &mut Cursor<&[u8]>) -> Result<u64, String> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read u64: {}", e))?;
    Ok(u64::from_le_bytes(buf))
}

fn read_length_prefixed_string(cursor: &mut Cursor<&[u8]>) -> Result<String, String> {
    let len = read_u32_le(cursor)? as usize;
    if len == 0 {
        return Ok(String::new());
    }
    if len > 10_000_000 {
        return Err(format!("String too long: {} bytes", len));
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).map_err(|e| format!("Read string: {}", e))?;
    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8: {}", e))
}

fn platform_id_to_string(id: u32) -> String {
    match id {
        0 => "Editor".to_string(),
        1 => "Windows".to_string(),
        2 => "OSX".to_string(),
        7 => "Linux".to_string(),
        8 => "iOS".to_string(),
        11 => "Android".to_string(),
        13 => "WebGL".to_string(),
        _ => format!("Platform_{}", id),
    }
}
