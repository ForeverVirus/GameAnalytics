// Device Transfer - WiFi communication with the Unity SDK's embedded HTTP server.
// Discovers devices on the local network, connects, streams live data, and downloads sessions.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

const DEFAULT_PORT: u16 = 9527;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const STOP_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceStatus {
    #[serde(rename = "deviceModel")]
    pub device_model: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "sdkVersion")]
    pub sdk_version: String,
    pub capturing: bool,
    #[serde(rename = "frameCount")]
    pub frame_count: u32,
    pub elapsed: f32,
    #[serde(rename = "currentFps")]
    pub current_fps: f32,
}

impl Default for DeviceStatus {
    fn default() -> Self {
        Self {
            device_model: String::new(),
            project_name: String::new(),
            sdk_version: String::new(),
            capturing: false,
            frame_count: 0,
            elapsed: 0.0,
            current_fps: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub ip: String,
    pub port: u16,
    pub status: DeviceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteSession {
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: i64,
    pub created: String,
}

impl Default for RemoteSession {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            size_bytes: 0,
            created: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RemoteStopCaptureResult {
    pub status: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "sessionName")]
    pub session_name: String,
    #[serde(rename = "frameCount")]
    pub frame_count: i32,
    pub duration: f32,
    #[serde(rename = "screenshotCount")]
    pub screenshot_count: i32,
}

/// Scan the local network subnet for devices running the GAProfiler HTTP server.
pub async fn discover_devices(port: Option<u16>) -> Vec<DiscoveredDevice> {
    let port = port.unwrap_or(DEFAULT_PORT);
    let local_ip = get_local_ip().unwrap_or(Ipv4Addr::new(192, 168, 1, 1));
    let octets = local_ip.octets();

    // Scan the /24 subnet in parallel
    let mut handles = Vec::new();
    for i in 1..=254u8 {
        let ip = Ipv4Addr::new(octets[0], octets[1], octets[2], i);
        let p = port;
        handles.push(tokio::spawn(async move {
            probe_device(ip, p).await
        }));
    }

    let mut devices = Vec::new();
    for handle in handles {
        if let Ok(Some(device)) = handle.await {
            devices.push(device);
        }
    }
    devices
}

async fn probe_device(ip: Ipv4Addr, port: u16) -> Option<DiscoveredDevice> {
    // Quick TCP connect check first
    let addr = SocketAddr::new(IpAddr::V4(ip), port);
    let tcp_ok = tokio::task::spawn_blocking(move || {
        TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).is_ok()
    }).await.unwrap_or(false);

    if !tcp_ok {
        return None;
    }

    // Try to get status
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;

    let url = format!("http://{}:{}/status", ip, port);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let body = resp.text().await.ok()?;
    let status = parse_device_status(&body).ok()?;
    Some(DiscoveredDevice {
        ip: ip.to_string(),
        port,
        status,
    })
}

/// Get device status from a known IP.
pub async fn get_device_status(ip: &str, port: u16) -> Result<DeviceStatus, String> {
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://{}:{}/status", ip, port);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    parse_device_status(&body)
}

/// List sessions available on the device.
pub async fn list_device_sessions(ip: &str, port: u16) -> Result<Vec<RemoteSession>, String> {
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://{}:{}/sessions", ip, port);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    parse_remote_sessions(&body)
}

/// Download a .gaprof session file from the device.
pub async fn download_session(ip: &str, port: u16, file_name: &str, save_dir: &str) -> Result<String, String> {
    let safe_file_name = sanitize_session_file_name(file_name)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(300)) // Large file timeout
        .build()
        .map_err(|e| e.to_string())?;

    let encoded_name = urlencoding::encode(file_name);
    let url = format!("http://{}:{}/sessions/{}/download", ip, port, encoded_name);
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("Download failed: {}", resp.status()));
    }

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    std::fs::create_dir_all(save_dir).map_err(|e| e.to_string())?;
    let save_path = Path::new(save_dir).join(safe_file_name);
    std::fs::write(&save_path, &bytes).map_err(|e| e.to_string())?;

    Ok(save_path.to_string_lossy().to_string())
}

/// Send start capture command to the device.
pub async fn remote_start_capture(ip: &str, port: u16, session_name: Option<String>) -> Result<(), String> {
    let client = Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://{}:{}/capture/start", ip, port);
    let mut req = client.post(&url);
    if let Some(name) = session_name {
        req = req.json(&serde_json::json!({"name": name}));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Start capture failed: {}", body));
    }
    Ok(())
}

/// Send stop capture command to the device.
pub async fn remote_stop_capture(ip: &str, port: u16) -> Result<RemoteStopCaptureResult, String> {
    let client = Client::builder()
        .timeout(STOP_REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://{}:{}/capture/stop", ip, port);
    let resp = client.post(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Stop capture failed: {}", body));
    }
    let body = resp.text().await.map_err(|e| e.to_string())?;
    parse_stop_capture_result(&body)
}

fn get_local_ip() -> Option<Ipv4Addr> {
    // Connect to a public address to discover local IP (no data is actually sent)
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

fn parse_device_status(body: &str) -> Result<DeviceStatus, String> {
    if let Ok(status) = serde_json::from_str::<DeviceStatus>(body) {
        return Ok(status);
    }

    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("解析设备状态失败: {} | body: {}", e, body))?;

    Ok(DeviceStatus {
        device_model: value.get("deviceModel").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        project_name: value.get("projectName").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        sdk_version: value.get("sdkVersion").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        capturing: value.get("capturing").and_then(|v| v.as_bool()).unwrap_or(false),
        frame_count: value.get("frameCount").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        elapsed: value.get("elapsed").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        current_fps: value.get("currentFps").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
    })
}

fn parse_remote_sessions(body: &str) -> Result<Vec<RemoteSession>, String> {
    if let Ok(items) = serde_json::from_str::<Vec<RemoteSession>>(body) {
        return Ok(items);
    }

    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("解析设备会话列表失败: {} | body: {}", e, body))?;
    let arr = value
        .as_array()
        .ok_or_else(|| format!("设备会话列表不是数组: {}", body))?;

    Ok(arr
        .iter()
        .map(|item| RemoteSession {
            file_name: item.get("fileName").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            size_bytes: item.get("sizeBytes").and_then(|v| v.as_i64()).unwrap_or(0),
            created: item.get("created").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        })
        .collect())
}

fn sanitize_session_file_name(file_name: &str) -> Result<&str, String> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return Err("会话文件名不能为空".to_string());
    }

    let path = Path::new(trimmed);
    let base = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("非法会话文件名".to_string())?;

    if base != trimmed {
        return Err("会话文件名不能包含目录".to_string());
    }

    Ok(base)
}

fn parse_stop_capture_result(body: &str) -> Result<RemoteStopCaptureResult, String> {
    if let Ok(result) = serde_json::from_str::<RemoteStopCaptureResult>(body) {
        return Ok(result);
    }

    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("解析停止采集响应失败: {} | body: {}", e, body))?;

    Ok(RemoteStopCaptureResult {
        status: value.get("status").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        file_path: value.get("filePath").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        session_name: value.get("sessionName").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        frame_count: value.get("frameCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        duration: value.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        screenshot_count: value.get("screenshotCount").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    })
}
