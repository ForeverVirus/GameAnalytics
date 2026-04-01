use std::fs;
use std::path::Path;

use crate::graph::model::*;
use crate::graph::store::GraphStore;

/// Collect asset metrics for all asset nodes in the store
pub fn collect_asset_metrics(
    store: &GraphStore,
    project_root: &Path,
) -> Vec<AssetMetrics> {
    let mut results = Vec::new();

    for node in store.nodes.values() {
        if node.node_type != NodeType::Asset {
            continue;
        }
        let Some(fp) = &node.file_path else { continue };
        let full_path = project_root.join(fp.replace('/', std::path::MAIN_SEPARATOR_STR));
        let file_size = fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);

        let mut metrics = AssetMetrics {
            node_id: node.id.clone(),
            file_path: fp.clone(),
            file_size_bytes: file_size,
            texture_width: None,
            texture_height: None,
            texture_format: None,
            estimated_memory_bytes: None,
            has_mipmaps: None,
            vertex_count: None,
            triangle_count: None,
            submesh_count: None,
            sample_rate: None,
            duration_seconds: None,
            channels: None,
            performance_rating: None,
            ai_optimization_suggestion: None,
        };

        match &node.asset_kind {
            Some(AssetKind::Texture) => {
                parse_texture_info(&full_path, &mut metrics);
                try_parse_unity_meta_texture(&full_path, &mut metrics);
                estimate_texture_memory(&mut metrics);
                rate_texture_performance(&mut metrics);
            }
            Some(AssetKind::Audio) => {
                parse_audio_info(&full_path, &mut metrics);
                rate_audio_performance(&mut metrics);
            }
            Some(AssetKind::Other) => {
                // For unknown asset types, use file-size-based estimates
                rate_model_performance(&mut metrics);
            }
            _ => {}
        }

        results.push(metrics);
    }

    results.sort_by(|a, b| b.file_size_bytes.cmp(&a.file_size_bytes));
    results
}

/// Read PNG header to extract width/height
fn parse_texture_info(path: &Path, metrics: &mut AssetMetrics) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return,
    };

    match ext.as_str() {
        "png" => {
            // PNG: bytes 16..20 = width, 20..24 = height (IHDR chunk)
            if data.len() >= 24 && &data[0..8] == b"\x89PNG\r\n\x1a\n" {
                let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
                let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
                metrics.texture_width = Some(w);
                metrics.texture_height = Some(h);
                metrics.texture_format = Some("PNG".to_string());
            }
        }
        "jpg" | "jpeg" => {
            // JPEG SOF0 marker scan
            if let Some((w, h)) = parse_jpeg_dimensions(&data) {
                metrics.texture_width = Some(w);
                metrics.texture_height = Some(h);
                metrics.texture_format = Some("JPEG".to_string());
            }
        }
        "tga" => {
            // TGA: bytes 12..14 = width, 14..16 = height (little-endian)
            if data.len() >= 18 {
                let w = u16::from_le_bytes([data[12], data[13]]) as u32;
                let h = u16::from_le_bytes([data[14], data[15]]) as u32;
                metrics.texture_width = Some(w);
                metrics.texture_height = Some(h);
                metrics.texture_format = Some("TGA".to_string());
            }
        }
        "bmp" => {
            if data.len() >= 26 {
                let w = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
                let h = u32::from_le_bytes([data[22], data[23], data[24], data[25]]);
                metrics.texture_width = Some(w);
                metrics.texture_height = Some(h);
                metrics.texture_format = Some("BMP".to_string());
            }
        }
        _ => {
            metrics.texture_format = Some(ext.to_uppercase());
        }
    }
}

/// Parse JPEG dimensions from SOF markers
fn parse_jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2; // Skip SOI marker
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if marker == 0xD9 { break; } // EOI
        if marker == 0x00 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 3 >= data.len() { break; }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;

        // SOF0..SOF2 markers
        if (0xC0..=0xC2).contains(&marker) && i + 9 < data.len() {
            let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
            let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

/// Try to parse Unity .meta file for texture import settings
fn try_parse_unity_meta_texture(texture_path: &Path, metrics: &mut AssetMetrics) {
    let meta_path = texture_path.with_extension(
        format!("{}.meta", texture_path.extension().and_then(|e| e.to_str()).unwrap_or(""))
    );
    let content = match fs::read_to_string(&meta_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Check for mipmaps
    if content.contains("enableMipMap: 1") {
        metrics.has_mipmaps = Some(true);
    } else if content.contains("enableMipMap: 0") {
        metrics.has_mipmaps = Some(false);
    }
}

/// Estimate texture GPU memory
fn estimate_texture_memory(metrics: &mut AssetMetrics) {
    let (Some(w), Some(h)) = (metrics.texture_width, metrics.texture_height) else { return };
    // Assume 4 bytes per pixel (RGBA32), × 1.33 if mipmaps
    let base = (w as u64) * (h as u64) * 4;
    let with_mip = if metrics.has_mipmaps.unwrap_or(true) {
        (base as f64 * 1.33) as u64
    } else {
        base
    };
    metrics.estimated_memory_bytes = Some(with_mip);
}

fn rate_texture_performance(metrics: &mut AssetMetrics) {
    let mem = metrics.estimated_memory_bytes.unwrap_or(0);
    let max_dim = std::cmp::max(
        metrics.texture_width.unwrap_or(0),
        metrics.texture_height.unwrap_or(0)
    );

    let rating = if max_dim > 4096 || mem > 64 * 1024 * 1024 {
        "poor"
    } else if max_dim > 2048 || mem > 16 * 1024 * 1024 {
        "fair"
    } else {
        "good"
    };
    metrics.performance_rating = Some(rating.to_string());
}

/// Parse basic audio file info (WAV header)
fn parse_audio_info(path: &Path, metrics: &mut AssetMetrics) {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if ext == "wav" {
        let data = match fs::read(path) {
            Ok(d) => d,
            Err(_) => return,
        };
        // WAV: bytes 22..24 = channels, 24..28 = sample rate
        if data.len() >= 28 && &data[0..4] == b"RIFF" {
            let channels = u16::from_le_bytes([data[22], data[23]]) as u32;
            let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
            let bits_per_sample = if data.len() >= 36 {
                u16::from_le_bytes([data[34], data[35]]) as u32
            } else {
                16
            };
            metrics.channels = Some(channels);
            metrics.sample_rate = Some(sample_rate);

            // Estimate duration from file size
            if sample_rate > 0 && channels > 0 && bits_per_sample > 0 {
                let bytes_per_second = sample_rate * channels * (bits_per_sample / 8);
                if bytes_per_second > 0 {
                    let data_size = metrics.file_size_bytes.saturating_sub(44); // WAV header ~44 bytes
                    metrics.duration_seconds = Some(data_size as f32 / bytes_per_second as f32);
                }
            }
        }
    }
    // For MP3/OGG, just record extension info; full parsing requires specialized libraries
}

fn rate_audio_performance(metrics: &mut AssetMetrics) {
    let size_mb = metrics.file_size_bytes as f64 / (1024.0 * 1024.0);
    let duration = metrics.duration_seconds.unwrap_or(0.0);

    let rating = if size_mb > 50.0 || (duration > 0.0 && size_mb / (duration as f64 / 60.0) > 30.0) {
        "poor"
    } else if size_mb > 10.0 {
        "fair"
    } else {
        "good"
    };
    metrics.performance_rating = Some(rating.to_string());
}

fn rate_model_performance(metrics: &mut AssetMetrics) {
    let size_mb = metrics.file_size_bytes as f64 / (1024.0 * 1024.0);
    let rating = if size_mb > 50.0 {
        "poor"
    } else if size_mb > 10.0 {
        "fair"
    } else {
        "good"
    };
    metrics.performance_rating = Some(rating.to_string());
}
