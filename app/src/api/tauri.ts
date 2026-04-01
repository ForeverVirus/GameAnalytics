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
