import { create } from 'zustand';
import { api, ProjectInfo, AnalysisStats, FrontendGraph, SuspectedReference, HardcodeFinding, AppSettings, listenProgress, listenAiLog } from '../api/tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';

interface AppStore {
  // Project state
  project: ProjectInfo | null;
  loading: boolean;
  error: string | null;

  // Analysis results
  stats: AnalysisStats | null;
  assetGraph: FrontendGraph | null;
  codeGraph: FrontendGraph | null;
  suspectedRefs: SuspectedReference[];
  hardcodeFindings: HardcodeFinding[];

  // Settings
  settings: AppSettings;

  // AI analysis (per-node)
  aiLoading: boolean;
  aiResult: string | null;
  aiError: string | null;
  aiNodeId: string | null;
  aiLiveLog: string[];

  // Progress modal
  showProgress: boolean;
  progressPhase: string;
  progressStep: string;
  progressCurrent: number;
  progressTotal: number;
  progressMessage: string;
  progressDone: boolean;

  // AI logs
  aiLogs: string[];

  // Actions
  selectProject: (path: string) => Promise<void>;
  loadProjectInfo: () => Promise<void>;
  runAnalysis: () => Promise<void>;
  loadAssetGraph: () => Promise<void>;
  loadCodeGraph: () => Promise<void>;
  loadSuspectedRefs: () => Promise<void>;
  loadHardcodeFindings: () => Promise<void>;
  promoteSuspectedRef: (id: string) => Promise<void>;
  ignoreSuspectedRef: (id: string) => Promise<void>;
  loadSettings: () => Promise<void>;
  saveSettings: (settings: AppSettings) => Promise<void>;
  openFileLocation: (filePath: string) => Promise<void>;
  runAiAnalysis: (nodeId: string) => Promise<void>;
  runDeepAiAnalysis: (nodeId: string) => Promise<void>;
  clearAiResult: () => void;
  clearError: () => void;
  startFullPipeline: (path: string, forceRefresh?: boolean) => Promise<void>;
  exportAnalysis: () => Promise<string | null>;
  dismissProgress: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  project: null,
  loading: false,
  error: null,
  stats: null,
  assetGraph: null,
  codeGraph: null,
  suspectedRefs: [],
  hardcodeFindings: [],
  settings: {
    ai_cli: 'claude',
    language: 'zh',
    scan_scope: 'full',
    hardcode_enabled: true,
    suspected_enabled: true,
    ai_model: null,
    ai_thinking: null,
  },
  aiLoading: false,
  aiResult: null,
  aiError: null,
  aiNodeId: null,
  aiLiveLog: [],

  // Progress modal state
  showProgress: false,
  progressPhase: 'scan',
  progressStep: '',
  progressCurrent: 0,
  progressTotal: 0,
  progressMessage: '',
  progressDone: false,
  aiLogs: [],

  selectProject: async (path) => {
    set({ loading: true, error: null });
    try {
      const project = await api.selectProject(path);
      set({ project, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadProjectInfo: async () => {
    try {
      const project = await api.getProjectInfo();
      set({ project });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  runAnalysis: async () => {
    set({ loading: true, error: null });
    try {
      const stats = await api.runAnalysis();
      set({ stats, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  loadAssetGraph: async () => {
    try {
      const assetGraph = await api.getAssetGraph();
      set({ assetGraph });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadCodeGraph: async () => {
    try {
      const codeGraph = await api.getCodeGraph();
      set({ codeGraph });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadSuspectedRefs: async () => {
    try {
      const suspectedRefs = await api.getSuspectedRefs();
      set({ suspectedRefs });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadHardcodeFindings: async () => {
    try {
      const hardcodeFindings = await api.getHardcodeFindings();
      set({ hardcodeFindings });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  promoteSuspectedRef: async (id) => {
    try {
      await api.promoteSuspectedRef(id);
      await get().loadSuspectedRefs();
      // Reload graphs so the new edge appears
      await get().loadAssetGraph();
      await get().loadCodeGraph();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  ignoreSuspectedRef: async (id) => {
    try {
      await api.ignoreSuspectedRef(id);
      await get().loadSuspectedRefs();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  loadSettings: async () => {
    try {
      const settings = await api.loadSettings();
      set({ settings });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  saveSettings: async (settings) => {
    try {
      await api.saveSettings(settings);
      set({ settings });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  clearError: () => set({ error: null }),

  openFileLocation: async (filePath: string) => {
    const { project } = get();
    if (!project) return;
    try {
      await api.openFileLocation(filePath, project.path);
    } catch (e) {
      set({ error: String(e) });
    }
  },

  runAiAnalysis: async (nodeId: string) => {
    const { settings } = get();
    set({ aiLoading: true, aiResult: null, aiError: null, aiNodeId: nodeId, aiLiveLog: [] });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const result = await api.runAiAnalysis(nodeId, settings.ai_cli, settings.ai_model, settings.ai_thinking);
      set({ aiResult: result, aiLoading: false, aiNodeId: null });
      try {
        await api.updateNodeAiSummary(nodeId, result, 'quick');
        await api.saveAnalysisCache();
        await get().loadAssetGraph();
        await get().loadCodeGraph();
      } catch (e) {
        console.warn('Failed to persist AI summary:', e);
      }
    } catch (e) {
      set({ aiError: String(e), aiLoading: false, aiNodeId: null });
    } finally {
      unlistenLog?.();
    }
  },

  runDeepAiAnalysis: async (nodeId: string) => {
    const { settings } = get();
    set({ aiLoading: true, aiResult: null, aiError: null, aiNodeId: nodeId, aiLiveLog: [] });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const result = await api.runDeepAiAnalysis(nodeId, settings.ai_cli, settings.ai_model, settings.ai_thinking);
      set({ aiResult: result, aiLoading: false, aiNodeId: null });
      try {
        await api.updateNodeAiSummary(nodeId, result, 'deep');
        await api.saveAnalysisCache();
        await get().loadAssetGraph();
        await get().loadCodeGraph();
      } catch (e) {
        console.warn('Failed to persist AI summary:', e);
      }
    } catch (e) {
      set({ aiError: String(e), aiLoading: false, aiNodeId: null });
    } finally {
      unlistenLog?.();
    }
  },

  clearAiResult: () => set({ aiResult: null, aiError: null }),

  startFullPipeline: async (path: string, forceRefresh?: boolean) => {
    let unlisten: UnlistenFn | null = null;
    let unlistenLog: UnlistenFn | null = null;
    try {
      // Show progress modal
      set({
        showProgress: true,
        progressPhase: 'scan',
        progressStep: '',
        progressCurrent: 0,
        progressTotal: 0,
        progressMessage: '准备中...',
        progressDone: false,
        aiLogs: [],
        error: null,
      });

      // Subscribe to progress events
      unlisten = await listenProgress((progress) => {
        set({
          progressPhase: progress.phase,
          progressStep: progress.step,
          progressCurrent: progress.current,
          progressTotal: progress.total,
          progressMessage: progress.message,
        });
      });

      // Subscribe to AI log events
      unlistenLog = await listenAiLog((line) => {
        set((state) => ({ aiLogs: [...state.aiLogs, line] }));
      });

      // Phase 1: Select project (also loads cache if available)
      const project = await api.selectProject(path);
      set({ project });

      // Check if cache was loaded (graph already has data)
      const hasCache = await api.hasAnalysisCache();
      if (hasCache && !forceRefresh) {
        // Cache was loaded in select_project — load results into frontend
        const stats = await api.getStats();
        set({ stats });
        set({
          progressDone: true,
          progressPhase: 'done',
          progressMessage: '已加载缓存分析结果',
        });
        return;
      }

      // Phase 2: Static scan
      const stats = await api.runAnalysis();
      set({ stats });

      // Auto-save cache for next time
      try {
        await api.saveAnalysisCache();
      } catch (e) {
        console.warn('Failed to save analysis cache:', e);
      }

      set({
        progressDone: true,
        progressPhase: 'done',
        progressMessage: '分析完成',
      });

    } catch (e) {
      set({
        error: String(e),
        progressDone: true,
        progressPhase: 'error',
        progressMessage: `分析出错: ${String(e)}`,
      });
    } finally {
      if (unlisten) unlisten();
      if (unlistenLog) unlistenLog();
    }
  },

  exportAnalysis: async () => {
    try {
      const exportPath = await api.exportAnalysis();
      return exportPath;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  dismissProgress: () => set({
    showProgress: false,
    progressPhase: 'scan',
    progressStep: '',
    progressCurrent: 0,
    progressTotal: 0,
    progressMessage: '',
    progressDone: false,
    aiLogs: [],
  }),
}));
