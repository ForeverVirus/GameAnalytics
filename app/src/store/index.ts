import { create } from 'zustand';
import { api, ProjectInfo, AnalysisStats, FrontendGraph, SuspectedReference, HardcodeFinding, AppSettings, listenProgress, listenAiLog, listenProfilerFrame, listenProfilerAutoStop, OrphanReport, DuplicateGroup, HotspotReport, AssetMetricsData, ReviewResult, ProfilerFrame, SessionMeta, ProfilerReport, DeepAnalysisReport, ComparisonResult, UnityStatus } from '../api/tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';

let profilerFrameUnlisten: UnlistenFn | null = null;
let profilerAutoStopUnlisten: UnlistenFn | null = null;

function clearProfilerListeners() {
  profilerFrameUnlisten?.();
  profilerAutoStopUnlisten?.();
  profilerFrameUnlisten = null;
  profilerAutoStopUnlisten = null;
}

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

  // V2: Redundancy
  orphans: OrphanReport[];
  duplicates: DuplicateGroup[];
  hotspots: HotspotReport[];
  loadOrphans: () => Promise<void>;
  loadDuplicates: () => Promise<void>;
  loadHotspots: (threshold?: number) => Promise<void>;

  // V2: Asset Metrics
  assetMetrics: AssetMetricsData[];
  loadAssetMetrics: () => Promise<void>;

  // V2: Code Review
  reviewResults: ReviewResult[];
  reviewLoading: boolean;
  runCodeReview: (nodeId: string, reviewType: string) => Promise<void>;
  runProjectCodeReview: (reviewType: string) => Promise<void>;
  clearReviewResults: () => void;

  // V2: Asset Review
  assetReviewResult: ReviewResult | null;
  assetReviewLoading: boolean;
  runAssetReview: () => Promise<void>;

  // V3: Profiler
  unityStatus: UnityStatus;
  profiling: boolean;
  liveFrames: ProfilerFrame[];
  profilerSessions: SessionMeta[];
  currentReport: ProfilerReport | null;
  currentDeepReport: DeepAnalysisReport | null;
  comparisonResult: ComparisonResult | null;
  profilerTab: string;
  profilerLoading: boolean;
  selectedProfilerSessionId: string | null;

  discoverUnity: () => Promise<number>;
  connectUnity: (port: number) => Promise<boolean>;
  disconnectUnity: () => Promise<void>;
  refreshUnityStatus: () => Promise<void>;
  startProfiling: (sessionName: string) => Promise<string>;
  stopProfiling: () => Promise<SessionMeta | null>;
  loadProfilerSessions: () => Promise<void>;
  deleteProfilerSession: (sessionId: string) => Promise<void>;
  renameProfilerSession: (sessionId: string, newName: string) => Promise<void>;
  generateProfilerReport: (sessionId: string) => Promise<void>;
  generateDeepAnalysis: (sessionId: string, filePaths: string[]) => Promise<void>;
  compareProfilerSessions: (sessionAId: string, sessionBId: string) => Promise<void>;
  exportProfilerReport: (sessionId: string) => Promise<string | null>;
  exportComparison: () => Promise<string | null>;
  setProfilerTab: (tab: string) => void;
  setSelectedProfilerSessionId: (sessionId: string | null) => void;
  clearProfilerReport: () => void;
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
      const persisted = await api.saveSettings(settings);
      set({ settings: persisted });
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

  // V2: Redundancy
  orphans: [],
  duplicates: [],
  hotspots: [],
  loadOrphans: async () => {
    try {
      const orphans = await api.getOrphanNodes();
      set({ orphans });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  loadDuplicates: async () => {
    try {
      const duplicates = await api.getDuplicateResources();
      set({ duplicates });
    } catch (e) {
      set({ error: String(e) });
    }
  },
  loadHotspots: async (threshold?: number) => {
    try {
      const hotspots = await api.getHotspots(threshold);
      set({ hotspots });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  // V2: Asset Metrics
  assetMetrics: [],
  loadAssetMetrics: async () => {
    try {
      const assetMetrics = await api.getAssetMetrics();
      set({ assetMetrics });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  // V2: Code Review
  reviewResults: [],
  reviewLoading: false,
  runCodeReview: async (nodeId: string, reviewType: string) => {
    const { settings } = get();
    set({ reviewLoading: true, aiLiveLog: [], error: null });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const result = await api.runAiCodeReview(
        nodeId,
        reviewType,
        settings.language,
        settings.ai_cli,
        settings.ai_model,
        settings.ai_thinking
      );
      set((s) => ({
        reviewResults: [...s.reviewResults, result],
        reviewLoading: false,
        error: null,
      }));
    } catch (e) {
      set({ error: String(e), reviewLoading: false });
    } finally {
      unlistenLog?.();
    }
  },
  runProjectCodeReview: async (reviewType: string) => {
    const { settings } = get();
    set({ reviewLoading: true, aiLiveLog: [], error: null });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const results = await api.runAiProjectCodeReview(
        reviewType,
        settings.language,
        settings.ai_cli,
        settings.ai_model,
        settings.ai_thinking
      );
      set((s) => ({
        reviewResults: [...s.reviewResults, ...results],
        reviewLoading: false,
        error: null,
      }));
    } catch (e) {
      set({ error: String(e), reviewLoading: false });
    } finally {
      unlistenLog?.();
    }
  },
  clearReviewResults: () => set({ reviewResults: [] }),

  // V2: Asset Review
  assetReviewResult: null,
  assetReviewLoading: false,
  runAssetReview: async () => {
    const { settings } = get();
    set({ assetReviewLoading: true, aiLiveLog: [], error: null });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const result = await api.runAiAssetReview(
        settings.language,
        settings.ai_cli,
        settings.ai_model,
        settings.ai_thinking
      );
      set({ assetReviewResult: result, assetReviewLoading: false, error: null });
    } catch (e) {
      set({ error: String(e), assetReviewLoading: false });
    } finally {
      unlistenLog?.();
    }
  },

  // V3: Profiler
  unityStatus: { connected: false, port: null, editor_state: null, profiling: false },
  profiling: false,
  liveFrames: [],
  profilerSessions: [],
  currentReport: null,
  currentDeepReport: null,
  comparisonResult: null,
  profilerTab: 'live',
  profilerLoading: false,
  selectedProfilerSessionId: null,

  discoverUnity: async () => {
    const port = await api.discoverUnity();
    return port;
  },

  connectUnity: async (port: number) => {
    const ok = await api.connectUnity(port);
    if (ok) {
      const status = await api.getUnityStatus();
      set({ unityStatus: status, profiling: status.profiling });
    }
    return ok;
  },

  disconnectUnity: async () => {
    if (get().profiling) {
      await get().stopProfiling();
    }
    clearProfilerListeners();
    await api.disconnectUnity();
    set({
      profiling: false,
      unityStatus: { connected: false, port: null, editor_state: null, profiling: false }
    });
  },

  refreshUnityStatus: async () => {
    try {
      const status = await api.getUnityStatus();
      set({ unityStatus: status, profiling: status.profiling });
    } catch {
      set({ profiling: false, unityStatus: { connected: false, port: null, editor_state: null, profiling: false } });
    }
  },

  startProfiling: async (sessionName: string) => {
    clearProfilerListeners();
    set((s) => ({
      profiling: true,
      liveFrames: [],
      error: null,
      unityStatus: { ...s.unityStatus, profiling: true }
    }));
    try {
      profilerFrameUnlisten = await listenProfilerFrame((frame) => {
        set((s) => {
          const frames = [...s.liveFrames, frame];
          // Keep rolling window of 600 frames (~5 min at 2Hz)
          if (frames.length > 600) frames.shift();
          return { liveFrames: frames };
        });
      });
      profilerAutoStopUnlisten = await listenProfilerAutoStop(() => {
        clearProfilerListeners();
        void get().stopProfiling();
      });
      const sessionId = await api.startProfiling(sessionName);
      return sessionId;
    } catch (e) {
      clearProfilerListeners();
      set((s) => ({
        profiling: false,
        error: String(e),
        unityStatus: { ...s.unityStatus, profiling: false }
      }));
      await get().refreshUnityStatus();
      throw e;
    }
  },

  stopProfiling: async () => {
    clearProfilerListeners();
    try {
      const meta = await api.stopProfiling();
      set((s) => ({
        profiling: false,
        error: null,
        selectedProfilerSessionId: meta.id,
        unityStatus: { ...s.unityStatus, profiling: false }
      }));
      await get().loadProfilerSessions();
      await get().refreshUnityStatus();
      return meta;
    } catch (e) {
      set((s) => ({
        profiling: false,
        error: String(e),
        unityStatus: { ...s.unityStatus, profiling: false }
      }));
      return null;
    }
  },

  loadProfilerSessions: async () => {
    try {
      const sessions = await api.listProfilerSessions();
      set({ profilerSessions: sessions });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteProfilerSession: async (sessionId: string) => {
    try {
      await api.deleteProfilerSession(sessionId);
      await get().loadProfilerSessions();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  renameProfilerSession: async (sessionId: string, newName: string) => {
    try {
      await api.renameProfilerSession(sessionId, newName);
      await get().loadProfilerSessions();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  generateProfilerReport: async (sessionId: string) => {
    const { settings } = get();
    set({ profilerLoading: true, currentReport: null, aiLiveLog: [], error: null, selectedProfilerSessionId: sessionId });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const report = await api.generateProfilerReport(sessionId, settings.ai_cli, settings.ai_model, settings.ai_thinking);
      set({ currentReport: report, profilerLoading: false, error: null });
    } catch (e) {
      set({ error: String(e), profilerLoading: false });
    } finally {
      unlistenLog?.();
    }
  },

  generateDeepAnalysis: async (sessionId: string, filePaths: string[]) => {
    const { settings } = get();
    set({ profilerLoading: true, currentDeepReport: null, aiLiveLog: [], error: null, selectedProfilerSessionId: sessionId });
    let unlistenLog: UnlistenFn | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        set((s) => ({ aiLiveLog: [...s.aiLiveLog, line] }));
      });
      const report = await api.generateDeepProfilerAnalysis(sessionId, filePaths, settings.ai_cli, settings.ai_model, settings.ai_thinking);
      set({ currentDeepReport: report, profilerLoading: false, error: null });
    } catch (e) {
      set({ error: String(e), profilerLoading: false });
    } finally {
      unlistenLog?.();
    }
  },

  compareProfilerSessions: async (sessionAId: string, sessionBId: string) => {
    set({ profilerLoading: true, comparisonResult: null });
    try {
      const result = await api.compareProfilerSessions(sessionAId, sessionBId);
      set({ comparisonResult: result, profilerLoading: false });
    } catch (e) {
      set({ error: String(e), profilerLoading: false });
    }
  },

  exportProfilerReport: async (sessionId: string) => {
    const { currentReport } = get();
    if (!currentReport) return null;
    try {
      const path = await api.exportProfilerReport(sessionId, currentReport);
      return path;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  exportComparison: async () => {
    const { comparisonResult } = get();
    if (!comparisonResult) return null;
    try {
      const path = await api.exportProfilerComparison(comparisonResult);
      return path;
    } catch (e) {
      set({ error: String(e) });
      return null;
    }
  },

  setProfilerTab: (tab: string) => set({ profilerTab: tab }),
  setSelectedProfilerSessionId: (sessionId) => set({ selectedProfilerSessionId: sessionId }),

  clearProfilerReport: () => set({ currentReport: null, currentDeepReport: null, comparisonResult: null }),
}));
