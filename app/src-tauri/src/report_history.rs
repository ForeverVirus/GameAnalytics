// Report History Management
// Stores parsed reports as JSON in project/.ga-reports/ for history browsing.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::device_profile::DeviceProfileReport;

// ======================== Data Structures ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMeta {
    pub id: String,
    pub session_name: String,
    pub device_model: String,
    pub overall_grade: String,
    pub duration_seconds: f64,
    pub total_frames: u32,
    pub avg_fps: f32,
    pub peak_memory_mb: f32,
    pub timestamp: String,
    pub source_file: Option<String>,
}

// ======================== Operations ========================

fn reports_dir(project_path: &str) -> PathBuf {
    PathBuf::from(project_path).join(".ga-reports")
}

fn report_file(project_path: &str, id: &str) -> PathBuf {
    reports_dir(project_path).join(format!("{}.json", id))
}

fn meta_file(project_path: &str, id: &str) -> PathBuf {
    reports_dir(project_path).join(format!("{}.meta.json", id))
}

/// Save a report and its metadata after parsing.
/// Returns the generated report ID.
pub fn save_report(project_path: &str, report: &DeviceProfileReport) -> Result<String, String> {
    let dir = reports_dir(project_path);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Create reports dir: {}", e))?;

    let id = format!("{}_{}", 
        report.session_name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_"),
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    );

    // Save full report JSON
    let report_json = serde_json::to_string(report)
        .map_err(|e| format!("Serialize report: {}", e))?;
    std::fs::write(report_file(project_path, &id), &report_json)
        .map_err(|e| format!("Write report: {}", e))?;

    // Save compact meta
    let meta = ReportMeta {
        id: id.clone(),
        session_name: report.session_name.clone(),
        device_model: report.device_info.device_model.clone(),
        overall_grade: report.overall_grade.clone(),
        duration_seconds: report.duration_seconds,
        total_frames: report.total_frames,
        avg_fps: report.summary.avg_fps,
        peak_memory_mb: report.summary.peak_memory_mb,
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        source_file: report.source_file_path.clone(),
    };
    let meta_json = serde_json::to_string(&meta)
        .map_err(|e| format!("Serialize meta: {}", e))?;
    std::fs::write(meta_file(project_path, &id), &meta_json)
        .map_err(|e| format!("Write meta: {}", e))?;

    Ok(id)
}

/// List all saved report metadata for a project.
pub fn list_reports(project_path: &str) -> Result<Vec<ReportMeta>, String> {
    let dir = reports_dir(project_path);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut metas = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Read reports dir: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Read entry: {}", e))?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json")
            && path.file_name().map_or(false, |n| n.to_string_lossy().ends_with(".meta.json"))
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    if let Ok(meta) = serde_json::from_str::<ReportMeta>(&content) {
                        metas.push(meta);
                    }
                }
                Err(_) => continue,
            }
        }
    }

    // Sort by timestamp descending (newest first)
    metas.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(metas)
}

/// Get a full report by ID.
pub fn get_report(project_path: &str, report_id: &str) -> Result<DeviceProfileReport, String> {
    let path = report_file(project_path, report_id);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Read report: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Parse report: {}", e))
}

/// Delete a report by ID (removes both .json and .meta.json).
pub fn delete_report(project_path: &str, report_id: &str) -> Result<(), String> {
    let report_path = report_file(project_path, report_id);
    let meta_path = meta_file(project_path, report_id);

    if report_path.exists() {
        std::fs::remove_file(&report_path)
            .map_err(|e| format!("Delete report file: {}", e))?;
    }
    if meta_path.exists() {
        std::fs::remove_file(&meta_path)
            .map_err(|e| format!("Delete meta file: {}", e))?;
    }

    Ok(())
}
