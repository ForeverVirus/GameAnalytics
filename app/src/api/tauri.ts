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
    invoke<void>('save_settings', { settings }),

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
};

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
