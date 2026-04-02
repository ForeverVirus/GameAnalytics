import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface ProjectInfo {
  name: string;
  path: string;
  engine: string;
  file_count: number;
  scan_time: string | null;
}

export interface AnalysisStats {
  total_files: number;
  asset_count: number;
  script_count: number;
  class_count: number;
  method_count: number;
  official_edges: number;
  suspected_count: number;
  hardcode_count: number;
}

export interface FrontendNode {
  id: string;
  name: string;
  node_type: string;
  asset_kind: string | null;
  file_path: string | null;
  line_number: number | null;
  metadata: Record<string, string>;
}

export interface FrontendEdge {
  source: string;
  target: string;
  edge_type: string;
  reference_class: string;
  label: string | null;
}

export interface FrontendGraph {
  nodes: FrontendNode[];
  edges: FrontendEdge[];
}

export interface SuspectedReference {
  id: string;
  resource_path: string;
  resource_type: string | null;
  code_location: string;
  code_line: number | null;
  code_excerpt: string | null;
  load_method: string;
  confidence: number;
  status: string;
  ai_explanation: string | null;
}

export interface HardcodeFinding {
  id: string;
  file_path: string;
  line_number: number;
  value: string;
  code_excerpt: string;
  category: string;
  severity: string;
}

export interface AppSettings {
  ai_cli: string;
  language: string;
  scan_scope: string;
  hardcode_enabled: boolean;
  suspected_enabled: boolean;
  ai_model?: string | null;
  ai_thinking?: string | null;
}

export interface CliStatus {
  available: boolean;
  resolved_path: string | null;
}

// V2 types
export interface OrphanReport {
  node_id: string;
  node_name: string;
  node_type: string;
  asset_kind: string | null;
  file_path: string | null;
  file_size_bytes: number;
  suggestion: string;
}

export interface DuplicateItem {
  node_id: string;
  file_path: string;
  file_size: number;
  hash: string | null;
}

export interface DuplicateGroup {
  group_id: string;
  asset_kind: string | null;
  files: DuplicateItem[];
  total_size: number;
  similarity: number;
}

export interface HotspotReport {
  node_id: string;
  node_name: string;
  node_type: string;
  file_path: string | null;
  in_degree: number;
  dependents: string[];
  risk_level: string;
}

export interface AssetMetricsData {
  node_id: string;
  file_path: string;
  file_size_bytes: number;
  texture_width: number | null;
  texture_height: number | null;
  texture_format: string | null;
  estimated_memory_bytes: number | null;
  has_mipmaps: boolean | null;
  vertex_count: number | null;
  triangle_count: number | null;
  submesh_count: number | null;
  sample_rate: number | null;
  duration_seconds: number | null;
  channels: number | null;
  performance_rating: string | null;
  ai_optimization_suggestion: string | null;
}

export interface ReviewFinding {
  id: string;
  review_type: string;
  file_path: string;
  line_number: number | null;
  line_end: number | null;
  severity: string;
  category: string;
  message: string;
  suggestion: string | null;
}

export interface ReviewResult {
  node_id: string;
  review_type: string;
  findings: ReviewFinding[];
  summary: string;
  timestamp: string;
  raw_response: string;
}

// ======================== Profiler Types ========================

export interface ProfilerFrame {
  timestamp: number;
  fps: number;
  frame_time: number;
  render_time: number;
  batches: number;
  draw_calls: number;
  set_pass_calls: number;
  triangles: number;
  vertices: number;
  total_memory: number;
  used_heap: number;
  mono_heap: number;
  mono_used: number;
  graphics_memory: number;
}

export interface SessionMeta {
  id: string;
  name: string;
  created_at: string;
  frame_count: number;
  duration_secs: number;
  avg_fps: number;
  peak_memory: number;
}

export interface SessionSummary {
  frame_count: number;
  duration_secs: number;
  avg_fps: number;
  min_fps: number;
  max_fps: number;
  p99_frame_time: number;
  avg_frame_time: number;
  max_frame_time: number;
  avg_draw_calls: number;
  max_draw_calls: number;
  avg_batches: number;
  avg_triangles: number;
  peak_memory: number;
  avg_memory: number;
  peak_mono: number;
  peak_graphics_memory: number;
}

export interface MemoryDetailItem {
  name: string;
  count: number;
  size: number;
}

export interface MemorySnapshot {
  textures: MemoryDetailItem[];
  meshes: MemoryDetailItem[];
  materials: MemoryDetailItem[];
  audio: MemoryDetailItem[];
}

export interface ProfilerSession {
  id: string;
  name: string;
  created_at: string;
  frames: ProfilerFrame[];
  summary: SessionSummary;
  memory_snapshot: MemorySnapshot | null;
}

export interface ProfilerFinding {
  category: string;
  severity: string;
  title: string;
  description: string;
  suggestion: string;
  metric_name: string | null;
  metric_value: string | null;
}

export interface ProfilerReport {
  session_id: string;
  health_score: number;
  summary: string;
  findings: ProfilerFinding[];
  optimization_plan: string;
  raw_response: string;
  timestamp: string;
}

export interface SourceFinding {
  file_path: string;
  line_number: number | null;
  category: string;
  issue: string;
  suggestion: string;
  estimated_impact: string;
}

export interface DeepAnalysisReport {
  session_id: string;
  summary: string;
  source_findings: SourceFinding[];
  raw_response: string;
  timestamp: string;
}

export interface ComparisonMetric {
  name: string;
  unit: string;
  value_a: number;
  value_b: number;
  delta: number;
  delta_percent: number;
  improved: boolean;
}

export interface ComparisonResult {
  session_a_id: string;
  session_a_name: string;
  session_b_id: string;
  session_b_name: string;
  metrics: ComparisonMetric[];
  verdict: string;
}

export interface UnityStatus {
  connected: boolean;
  port: number | null;
  editor_state: {
    is_playing: boolean;
    is_paused: boolean;
    is_compiling: boolean;
  } | null;
  profiling: boolean;
}

// ======================== Device Profiler Types ========================

export interface DeviceStatus {
  deviceModel: string;
  projectName: string;
  sdkVersion: string;
  capturing: boolean;
  frameCount: number;
  elapsed: number;
  currentFps: number;
}

export interface DiscoveredDevice {
  ip: string;
  port: number;
  status: DeviceStatus;
}

export interface RemoteSession {
  fileName: string;
  sizeBytes: number;
  created: string;
}

export interface RemoteStopCaptureResult {
  status: string;
  filePath: string;
  sessionName: string;
  frameCount: number;
  duration: number;
  screenshotCount: number;
}

export interface DeviceInfo {
  device_model: string;
  device_name: string;
  operating_system: string;
  processor_type: string;
  processor_count: number;
  processor_frequency: number;
  system_memory_mb: number;
  graphics_device_name: string;
  graphics_memory_mb: number;
  screen_width: number;
  screen_height: number;
  screen_dpi: number;
  quality_level: number;
  quality_name: string;
  unity_version: string;
  app_version: string;
  platform: string;
}

export interface TimelinePoint {
  time: number;
  value: number;
}

export interface FpsBucket {
  label: string;
  count: number;
  percentage: number;
}

export interface ModuleBreakdown {
  name: string;
  avg_ms: number;
  max_ms: number;
  percentage: number;
}

export interface SceneStats {
  scene_name: string;
  frame_count: number;
  avg_fps: number;
  avg_memory_mb: number;
  jank_count: number;
}

export interface DeviceProfileReport {
  session_name: string;
  source_file_path?: string | null;
  device_info: DeviceInfo;
  duration_seconds: number;
  total_frames: number;
  summary: {
    avg_fps: number;
    min_fps: number;
    max_fps: number;
    p1_fps: number;
    p5_fps: number;
    p50_fps: number;
    p95_fps: number;
    p99_fps: number;
    fps_stability: number;
    avg_cpu_ms: number;
    avg_gpu_ms: number;
    peak_memory_mb: number;
    avg_memory_mb: number;
    total_gc_alloc_mb: number;
    jank_count: number;
    severe_jank_count: number;
    jank_rate: number;
  };
  fps_analysis: {
    target_fps: number;
    frames_below_target: number;
    below_target_pct: number;
    frames_below_30: number;
    below_30_pct: number;
    fps_histogram: FpsBucket[];
    fps_timeline: TimelinePoint[];
  };
  memory_analysis: {
    peak_total_mb: number;
    avg_total_mb: number;
    peak_mono_mb: number;
    peak_gfx_mb: number;
    total_gc_alloc_mb: number;
    gc_alloc_per_frame_bytes: number;
    memory_timeline: TimelinePoint[];
    memory_trend: string;
    memory_growth_rate_mb_per_min: number;
  };
  rendering_analysis: {
    avg_draw_calls: number;
    max_draw_calls: number;
    avg_batches: number;
    avg_triangles: number;
    max_triangles: number;
    avg_set_pass: number;
    batching_efficiency: number;
  };
  module_analysis: {
    avg_render_ms: number;
    avg_scripts_ms: number;
    avg_physics_ms: number;
    avg_animation_ms: number;
    avg_ui_ms: number;
    avg_particle_ms: number;
    avg_loading_ms: number;
    avg_gc_ms: number;
    bottleneck: string;
    module_breakdown: ModuleBreakdown[];
  };
  jank_analysis: {
    total_jank_frames: number;
    severe_jank_frames: number;
    jank_rate_pct: number;
    severe_jank_rate_pct: number;
    worst_frame_ms: number;
    worst_frame_index: number;
    jank_timeline: TimelinePoint[];
  };
  thermal_analysis: {
    has_data: boolean;
    avg_temperature: number;
    max_temperature: number;
    battery_drain: number;
    temperature_timeline: TimelinePoint[];
    thermal_throttle_risk: string;
  };
  overdraw_analysis: {
    avg_overdraw: number;
    max_overdraw: number;
    sample_count: number;
  } | null;
  function_analysis: FunctionAnalysis | null;
  log_analysis: LogAnalysis | null;
  scene_breakdown: SceneStats[];
  overall_grade: string;
  screenshot_count: number;
  screenshot_frame_indices: number[];
}

// V2 Deep Profiling Types
export interface FunctionStats {
  name: string;
  category: string;
  avg_self_ms: number;
  total_self_ms: number;
  self_pct: number;
  avg_total_ms: number;
  total_total_ms: number;
  total_pct: number;
  avg_call_count: number;
  total_call_count: number;
  frames_called: number;
}

export interface CategoryBreakdown {
  category: string;
  avg_ms: number;
  total_ms: number;
  percentage: number;
  function_count: number;
}

export interface PerFrameFunction {
  name: string;
  category: string;
  self_ms: number;
  total_ms: number;
  call_count: number;
  depth: number;
  parent_index: number;
}

export interface PerFrameFunctions {
  frame_index: number;
  functions: PerFrameFunction[];
}

export interface FunctionAnalysis {
  has_data: boolean;
  total_sampled_frames: number;
  top_functions: FunctionStats[];
  category_breakdown: CategoryBreakdown[];
  per_frame_data: PerFrameFunctions[];
}

export interface LogSummaryEntry {
  message: string;
  count: number;
  first_frame: number;
  log_type: number;
}

export interface LogEntry {
  timestamp: number;
  frame_index: number;
  log_type: number;
  message: string;
  stack_trace: string;
}

export interface LogAnalysis {
  has_data: boolean;
  total_logs: number;
  error_count: number;
  warning_count: number;
  exception_count: number;
  top_errors: LogSummaryEntry[];
  top_warnings: LogSummaryEntry[];
}

export const api = {
  selectProject: (path: string) =>
    invoke<ProjectInfo>('select_project', { path }),

  getProjectInfo: () =>
    invoke<ProjectInfo | null>('get_project_info'),

  runAnalysis: () =>
    invoke<AnalysisStats>('run_analysis'),

  getAssetGraph: () =>
    invoke<FrontendGraph>('get_asset_graph'),

  getCodeGraph: () =>
    invoke<FrontendGraph>('get_code_graph'),

  getStats: () =>
    invoke<AnalysisStats>('get_stats'),

  getSuspectedRefs: () =>
    invoke<SuspectedReference[]>('get_suspected_refs'),

  promoteSuspectedRef: (id: string) =>
    invoke<boolean>('promote_suspected_ref', { id }),

  ignoreSuspectedRef: (id: string) =>
    invoke<boolean>('ignore_suspected_ref', { id }),

  getHardcodeFindings: () =>
    invoke<HardcodeFinding[]>('get_hardcode_findings'),

  saveSettings: (settings: AppSettings) =>
    invoke<AppSettings>('save_settings', { settings }),

  loadSettings: () =>
    invoke<AppSettings>('load_settings'),

  detectAiClis: () =>
    invoke<Record<string, CliStatus>>('detect_ai_clis'),

  openFileLocation: (filePath: string, projectPath: string) =>
    invoke<void>('open_file_location', { filePath, projectPath }),

  readImageBase64: (filePath: string, projectPath: string) =>
    invoke<string>('read_image_base64', { filePath, projectPath }),

  runAiAnalysis: (nodeId: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<string>('run_ai_analysis', { nodeId, cliName, model: model || null, thinking: thinking || null }),

  runDeepAiAnalysis: (nodeId: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<string>('run_deep_ai_analysis', { nodeId, cliName, model: model || null, thinking: thinking || null }),

  runAiBatchAnalysis: (cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<number>('run_ai_batch_analysis', { cliName, model: model || null, thinking: thinking || null }),

  updateNodeAiSummary: (nodeId: string, summary: string, analysisType: string) =>
    invoke<void>('update_node_ai_summary', { nodeId, summary, analysisType }),

  exportAnalysis: () =>
    invoke<string>('export_analysis'),

  hasAnalysisCache: () =>
    invoke<boolean>('has_analysis_cache'),

  saveAnalysisCache: () =>
    invoke<void>('save_analysis_cache'),

  // V2 APIs
  getOrphanNodes: () =>
    invoke<OrphanReport[]>('get_orphan_nodes'),

  getDuplicateResources: () =>
    invoke<DuplicateGroup[]>('get_duplicate_resources'),

  getHotspots: (threshold?: number) =>
    invoke<HotspotReport[]>('get_hotspots', { threshold: threshold || null }),

  getAssetMetrics: () =>
    invoke<AssetMetricsData[]>('get_asset_metrics'),

  runAiCodeReview: (nodeId: string, reviewType: string, responseLanguage: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<ReviewResult>('run_ai_code_review', { nodeId, reviewType, responseLanguage, cliName, model: model || null, thinking: thinking || null }),

  runAiProjectCodeReview: (reviewType: string, responseLanguage: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<ReviewResult[]>('run_ai_project_code_review', { reviewType, responseLanguage, cliName, model: model || null, thinking: thinking || null }),

  runAiAssetReview: (responseLanguage: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<ReviewResult>('run_ai_asset_review', { responseLanguage, cliName, model: model || null, thinking: thinking || null }),

  // Profiler APIs
  discoverUnity: () =>
    invoke<number>('discover_unity'),

  connectUnity: (port: number) =>
    invoke<boolean>('connect_unity', { port }),

  getUnityStatus: () =>
    invoke<UnityStatus>('get_unity_status'),

  disconnectUnity: () =>
    invoke<void>('disconnect_unity'),

  startProfiling: (sessionName: string) =>
    invoke<string>('start_profiling', { sessionName }),

  stopProfiling: () =>
    invoke<SessionMeta>('stop_profiling'),

  listProfilerSessions: () =>
    invoke<SessionMeta[]>('list_profiler_sessions'),

  getProfilerSession: (sessionId: string) =>
    invoke<ProfilerSession>('get_profiler_session', { sessionId }),

  deleteProfilerSession: (sessionId: string) =>
    invoke<void>('delete_profiler_session', { sessionId }),

  renameProfilerSession: (sessionId: string, newName: string) =>
    invoke<void>('rename_profiler_session', { sessionId, newName }),

  generateProfilerReport: (sessionId: string, cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<ProfilerReport>('generate_profiler_report', { sessionId, cliName, model: model || null, thinking: thinking || null }),

  generateDeepProfilerAnalysis: (sessionId: string, filePaths: string[], cliName: string, model?: string | null, thinking?: string | null) =>
    invoke<DeepAnalysisReport>('generate_deep_profiler_analysis', { sessionId, filePaths, cliName, model: model || null, thinking: thinking || null }),

  compareProfilerSessions: (sessionAId: string, sessionBId: string) =>
    invoke<ComparisonResult>('compare_profiler_sessions', { sessionAId, sessionBId }),

  exportProfilerReport: (sessionId: string, report: ProfilerReport) =>
    invoke<string>('export_profiler_report', { sessionId, report }),

  exportProfilerComparison: (result: ComparisonResult) =>
    invoke<string>('export_profiler_comparison', { result }),

  // Device Profiler APIs
  discoverDevices: (port?: number) =>
    invoke<DiscoveredDevice[]>('discover_devices', { port: port || null }),

  getDeviceStatus: (ip: string, port: number) =>
    invoke<DeviceStatus>('get_device_status', { ip, port }),

  listDeviceSessions: (ip: string, port: number) =>
    invoke<RemoteSession[]>('list_device_sessions', { ip, port }),

  downloadDeviceSession: (ip: string, port: number, fileName: string) =>
    invoke<string>('download_device_session', { ip, port, fileName }),

  remoteStartCapture: (ip: string, port: number, sessionName?: string) =>
    invoke<void>('remote_start_capture', { ip, port, sessionName: sessionName || null }),

  remoteStopCapture: (ip: string, port: number) =>
    invoke<RemoteStopCaptureResult>('remote_stop_capture', { ip, port }),

  importGaprofFile: (filePath: string) =>
    invoke<string>('import_gaprof_file', { filePath }),

  parseGaprofSession: (filePath: string) =>
    invoke<DeviceProfileReport>('parse_gaprof_session', { filePath }),

  generateDeviceReport: (filePath: string) =>
    invoke<DeviceProfileReport>('generate_device_report', { filePath }),

  getDeviceScreenshot: (filePath: string, frameIndex: number) =>
    invoke<string>('get_device_screenshot', { filePath, frameIndex }),

  exportDeviceReport: (report: DeviceProfileReport) =>
    invoke<string>('export_device_report', { report }),

  getFrameFunctions: (filePath: string, frameIndex: number) =>
    invoke<PerFrameFunctions | null>('get_frame_functions', { filePath, frameIndex }),

  getSessionLogs: (filePath: string, logTypeFilter?: number, limit?: number) =>
    invoke<LogEntry[]>('get_session_logs', { filePath, logTypeFilter: logTypeFilter ?? null, limit: limit ?? null }),

  runAiDeviceAnalysis: (filePath: string, cliName: string, model?: string, thinking?: string) =>
    invoke<DeviceAiAnalysis>('run_ai_device_analysis', { filePath, cliName, model: model ?? null, thinking: thinking ?? null }),
};

export interface DeviceAiAnalysis {
  session_name: string;
  overall_grade: string;
  analysis: string;
  timestamp: string;
}

export interface AnalysisProgress {
  phase: string;
  step: string;
  current: number;
  total: number;
  message: string;
}

export function listenProgress(callback: (progress: AnalysisProgress) => void): Promise<UnlistenFn> {
  return listen<AnalysisProgress>('analysis_progress', (event) => {
    callback(event.payload);
  });
}

export function listenAiLog(callback: (line: string) => void): Promise<UnlistenFn> {
  return listen<string>('ai_log', (event) => {
    callback(event.payload);
  });
}

export function listenProfilerFrame(callback: (frame: ProfilerFrame) => void): Promise<UnlistenFn> {
  return listen<ProfilerFrame>('profiler_frame', (event) => {
    callback(event.payload);
  });
}

export function listenProfilerAutoStop(callback: (sessionId: string) => void): Promise<UnlistenFn> {
  return listen<string>('profiler_auto_stop', (event) => {
    callback(event.payload);
  });
}
