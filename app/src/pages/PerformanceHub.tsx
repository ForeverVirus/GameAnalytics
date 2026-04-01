import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import type { ProfilerFrame, SessionMeta, ComparisonMetric } from '../api/tauri';

// ──────────────── helper: mini sparkline via canvas ────────────────
function Sparkline({ data, color = '#00e0ff', height = 60 }: { data: number[]; color?: string; height?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const c = ref.current;
    const container = containerRef.current;
    if (!c || !container || data.length < 2) return;
    const width = container.clientWidth;
    c.width = width;
    c.height = height;
    const ctx = c.getContext('2d')!;
    ctx.clearRect(0, 0, width, height);
    const min = Math.min(...data);
    const max = Math.max(...data) || 1;
    const range = max - min || 1;

    // Fill gradient
    const grad = ctx.createLinearGradient(0, 0, 0, height);
    grad.addColorStop(0, color + '30');
    grad.addColorStop(1, color + '00');

    ctx.beginPath();
    data.forEach((v, i) => {
      const x = (i / (data.length - 1)) * width;
      const y = height - ((v - min) / range) * (height - 6) - 3;
      i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    });
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.stroke();

    // Fill area under the line
    ctx.lineTo(width, height);
    ctx.lineTo(0, height);
    ctx.closePath();
    ctx.fillStyle = grad;
    ctx.fill();
  }, [data, color, height]);
  return (
    <div ref={containerRef} style={{ width: '100%' }}>
      <canvas ref={ref} height={height} style={{ display: 'block', width: '100%' }} />
    </div>
  );
}

// ──────────────── Tab: Live Monitor (with integrated connection) ────────────────
function LiveMonitorTab() {
  const { t } = useTranslation();
  const unityStatus = useAppStore(s => s.unityStatus);
  const discoverUnity = useAppStore(s => s.discoverUnity);
  const connectUnity = useAppStore(s => s.connectUnity);
  const disconnectUnity = useAppStore(s => s.disconnectUnity);
  const profiling = useAppStore(s => s.profiling);
  const liveFrames = useAppStore(s => s.liveFrames);
  const startProfiling = useAppStore(s => s.startProfiling);
  const stopProfiling = useAppStore(s => s.stopProfiling);
  const refreshUnityStatus = useAppStore(s => s.refreshUnityStatus);
  const error = useAppStore(s => s.error);
  const [port, setPort] = useState('');
  const [discovering, setDiscovering] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [sessionName, setSessionName] = useState('');

  const handleDiscover = async () => {
    setDiscovering(true);
    try {
      const p = await discoverUnity();
      setPort(String(p));
    } catch { /* no-op */ }
    setDiscovering(false);
  };

  const handleConnect = async () => {
    const p = parseInt(port);
    if (!p) return;
    setConnecting(true);
    try {
      await connectUnity(p);
    } catch { /* no-op */ }
    setConnecting(false);
  };

  useEffect(() => {
    void refreshUnityStatus();
    const iv = setInterval(() => {
      void refreshUnityStatus();
    }, 3000);
    return () => clearInterval(iv);
  }, [refreshUnityStatus]);

  const handleStart = async () => {
    const name = sessionName.trim() || new Date().toISOString().slice(0, 19).replace(/[T:]/g, '-');
    await startProfiling(name);
  };

  const latest = liveFrames.length > 0 ? liveFrames[liveFrames.length - 1] : null;
  const fpsData = liveFrames.map(f => f.fps);
  const memData = liveFrames.map(f => f.total_memory);
  const dcData = liveFrames.map(f => f.draw_calls);

  const es = unityStatus.editor_state;

  return (
    <div className="perf-live">
      {/* ── Compact connection bar ── */}
      <div className="connect-bar">
        {!unityStatus.connected ? (
          <>
            <span className="connect-bar-label">Unity</span>
            <input type="number" placeholder={t('perf.portPlaceholder', '端口 (8090-8100)')} value={port} onChange={e => setPort(e.target.value)} className="input input-sm" style={{ width: 130 }} />
            <button className="btn btn-sm btn-primary" onClick={handleConnect} disabled={connecting || !port}>{connecting ? '...' : t('perf.connect', '连接')}</button>
            <button className="btn btn-sm" onClick={handleDiscover} disabled={discovering}>{discovering ? '...' : t('perf.autoDiscover', '自动发现')}</button>
            <span className="hint" style={{ marginLeft: 4 }}>{t('perf.connectHint', '确保 Unity 编辑器已安装并启动 Unity-Skills 插件')}</span>
          </>
        ) : (
          <>
            <span className="status-dot connected" />
            <span className="connect-bar-info">{t('perf.connected', '已连接')} — Port {unityStatus.port}</span>
            {es && (
              <div className="editor-state">
                <span className={`badge ${es.is_playing ? 'badge-green' : 'badge-gray'}`}>{es.is_playing ? '▶ Playing' : '⏸ Stopped'}</span>
                {es.is_paused && <span className="badge badge-yellow">Paused</span>}
                {es.is_compiling && <span className="badge badge-yellow">Compiling</span>}
              </div>
            )}
            <button className="btn btn-sm btn-danger" onClick={disconnectUnity} style={{ marginLeft: 'auto' }}>{t('perf.disconnect', '断开')}</button>
          </>
        )}
      </div>

      {error && (
        <div className="ai-error" style={{ marginBottom: 16 }}>
          {error}
        </div>
      )}

      <div className="live-controls">
        {!profiling ? (
          <>
            <input type="text" placeholder={t('perf.sessionName', '会话名称 (可选)')} value={sessionName} onChange={e => setSessionName(e.target.value)} className="input" />
            <button className="btn btn-primary" onClick={handleStart} disabled={!unityStatus.connected || !unityStatus.editor_state?.is_playing}>
              {t('perf.startProfile', '开始采集')}
            </button>
            {!unityStatus.connected && <span className="hint">{t('perf.needConnect', '请先连接 Unity')}</span>}
            {unityStatus.connected && !unityStatus.editor_state?.is_playing && <span className="hint">{t('perf.needPlay', '请在 Unity 中进入 Play 模式')}</span>}
          </>
        ) : (
          <>
            <span className="recording-indicator">● REC</span>
            <span className="frame-count">{liveFrames.length} {t('perf.frames', '帧')}</span>
            <button className="btn btn-danger" onClick={stopProfiling}>{t('perf.stopProfile', '停止采集')}</button>
          </>
        )}
      </div>

      {profiling && latest && (
        <div className="live-dashboard">
          <div className="live-stats-grid">
            <div className="live-stat">
              <span className="live-stat-value" style={{ color: latest.fps < 30 ? '#ff4444' : latest.fps < 60 ? '#ffaa00' : '#44ff44' }}>{latest.fps.toFixed(1)}</span>
              <span className="live-stat-label">FPS</span>
            </div>
            <div className="live-stat">
              <span className="live-stat-value">{latest.frame_time.toFixed(1)}</span>
              <span className="live-stat-label">Frame ms</span>
            </div>
            <div className="live-stat">
              <span className="live-stat-value">{latest.draw_calls}</span>
              <span className="live-stat-label">Draw Calls</span>
            </div>
            <div className="live-stat">
              <span className="live-stat-value">{latest.batches}</span>
              <span className="live-stat-label">Batches</span>
            </div>
            <div className="live-stat">
              <span className="live-stat-value">{(latest.total_memory / 1024 / 1024).toFixed(0)}</span>
              <span className="live-stat-label">Memory MB</span>
            </div>
            <div className="live-stat">
              <span className="live-stat-value">{(latest.triangles / 1000).toFixed(0)}K</span>
              <span className="live-stat-label">Triangles</span>
            </div>
          </div>
          <div className="live-charts">
            <div className="chart-card">
              <span className="chart-label">FPS</span>
              <Sparkline data={fpsData} color="#44ff44" height={70} />
            </div>
            <div className="chart-card">
              <span className="chart-label">Memory</span>
              <Sparkline data={memData} color="#ff8844" height={70} />
            </div>
            <div className="chart-card">
              <span className="chart-label">Draw Calls</span>
              <Sparkline data={dcData} color="#4488ff" height={70} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ──────────────── Tab: Sessions ────────────────
function SessionsTab({ onViewReport }: { onViewReport: (id: string) => void }) {
  const { t } = useTranslation();
  const sessions = useAppStore(s => s.profilerSessions);
  const loadSessions = useAppStore(s => s.loadProfilerSessions);
  const deleteSess = useAppStore(s => s.deleteProfilerSession);
  const renameSess = useAppStore(s => s.renameProfilerSession);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [newName, setNewName] = useState('');

  useEffect(() => { loadSessions(); }, [loadSessions]);

  const handleRename = async (id: string) => {
    if (newName.trim()) {
      await renameSess(id, newName.trim());
    }
    setRenaming(null);
    setNewName('');
  };

  return (
    <div className="perf-sessions">
      <h3>{t('perf.sessionsTitle', '历史会话')}</h3>
      {sessions.length === 0 ? (
        <p className="empty">{t('perf.noSessions', '暂无采集会话')}</p>
      ) : (
        <table className="data-table">
          <thead>
            <tr>
              <th>{t('perf.name', '名称')}</th>
              <th>{t('perf.date', '日期')}</th>
              <th>{t('perf.frameCount', '帧数')}</th>
              <th>{t('perf.duration', '时长')}</th>
              <th>{t('perf.avgFps', '平均FPS')}</th>
              <th>{t('perf.peakMem', '峰值内存')}</th>
              <th>{t('perf.actions', '操作')}</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map((s: SessionMeta) => (
              <tr key={s.id}>
                <td>
                  {renaming === s.id ? (
                    <span className="rename-row">
                      <input value={newName} onChange={e => setNewName(e.target.value)} className="input input-sm" onKeyDown={e => e.key === 'Enter' && handleRename(s.id)} />
                      <button className="btn btn-sm" onClick={() => handleRename(s.id)}>✓</button>
                    </span>
                  ) : s.name}
                </td>
                <td>{new Date(s.created_at).toLocaleString()}</td>
                <td>{s.frame_count}</td>
                <td>{s.duration_secs.toFixed(0)}s</td>
                <td style={{ color: s.avg_fps < 30 ? '#ff4444' : s.avg_fps < 60 ? '#ffaa00' : '#44ff44' }}>{s.avg_fps.toFixed(1)}</td>
                <td>{(s.peak_memory / 1024 / 1024).toFixed(0)} MB</td>
                <td className="actions-cell">
                  <button className="btn btn-sm" onClick={() => onViewReport(s.id)}>{t('perf.analyze', '分析')}</button>
                  <button className="btn btn-sm" onClick={() => { setRenaming(s.id); setNewName(s.name); }}>✏️</button>
                  <button className="btn btn-sm btn-danger" onClick={() => deleteSess(s.id)}>🗑</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

// ──────────────── Tab: Report ────────────────
function ReportTab() {
  const { t } = useTranslation();
  const sessions = useAppStore(s => s.profilerSessions);
  const report = useAppStore(s => s.currentReport);
  const deepReport = useAppStore(s => s.currentDeepReport);
  const loading = useAppStore(s => s.profilerLoading);
  const aiLog = useAppStore(s => s.aiLiveLog);
  const error = useAppStore(s => s.error);
  const genReport = useAppStore(s => s.generateProfilerReport);
  const genDeep = useAppStore(s => s.generateDeepAnalysis);
  const exportReport = useAppStore(s => s.exportProfilerReport);
  const selectedSession = useAppStore(s => s.selectedProfilerSessionId);
  const setSelectedSession = useAppStore(s => s.setSelectedProfilerSessionId);
  const [deepFiles, setDeepFiles] = useState('');

  const handleGenerate = () => {
    if (selectedSession) genReport(selectedSession);
  };

  const handleDeepAnalysis = () => {
    if (selectedSession && deepFiles.trim()) {
      const paths = deepFiles.split('\n').map(s => s.trim()).filter(Boolean);
      genDeep(selectedSession, paths);
    }
  };

  const handleExport = async () => {
    if (selectedSession) {
      const path = await exportReport(selectedSession);
      if (path) alert(t('perf.exported', '已导出: ') + path);
    }
  };

  return (
    <div className="perf-report">
      {error && !loading && (
        <div className="ai-error" style={{ marginBottom: 16 }}>
          {error}
        </div>
      )}

      <div className="report-controls">
        <select
          value={selectedSession ?? ''}
          onChange={e => setSelectedSession(e.target.value || null)}
          className="input"
        >
          <option value="">{t('perf.selectSession', '选择会话...')}</option>
          {sessions.map((s: SessionMeta) => <option key={s.id} value={s.id}>{s.name}</option>)}
        </select>
        <button className="btn btn-primary" onClick={handleGenerate} disabled={loading || !selectedSession}>
          {loading ? t('perf.analyzing', '分析中...') : t('perf.generateReport', '生成报告')}
        </button>
      </div>

      {loading && aiLog.length > 0 && (
        <pre className="ai-log-box">{aiLog.slice(-20).join('\n')}</pre>
      )}

      {report && (
        <div className="report-content">
          <div className="report-header">
            <div className={`health-score health-${report.health_score >= 80 ? 'good' : report.health_score >= 50 ? 'warn' : 'bad'}`}>
              <span className="score-value">{report.health_score}</span>
              <span className="score-label">{t('perf.healthScore', '健康分')}</span>
            </div>
            <p className="report-summary">{report.summary}</p>
            <button className="btn" onClick={handleExport}>{t('perf.export', '导出 Markdown')}</button>
          </div>

          {report.findings.length > 0 && (
            <div className="findings-list">
              <h4>{t('perf.findings', '发现问题')}</h4>
              {report.findings.map((f, i) => (
                <div key={i} className={`finding-card severity-${f.severity}`}>
                  <div className="finding-head">
                    <span className={`severity-badge ${f.severity}`}>{f.severity}</span>
                    <span className="finding-category">{f.category}</span>
                    <strong>{f.title}</strong>
                  </div>
                  <p>{f.description}</p>
                  <p className="finding-suggestion">💡 {f.suggestion}</p>
                  {f.metric_name && <span className="metric-tag">{f.metric_name}: {f.metric_value}</span>}
                </div>
              ))}
            </div>
          )}

          {report.optimization_plan && (
            <div className="optimization-plan">
              <h4>{t('perf.optimizationPlan', '优化方案')}</h4>
              <pre className="plan-content">{report.optimization_plan}</pre>
            </div>
          )}

          <div className="deep-analysis-section">
            <h4>{t('perf.deepAnalysis', '深度代码分析')}</h4>
            <textarea
              className="input deep-files-input"
              placeholder={t('perf.deepFilesHint', '输入源文件路径 (每行一个), 如:\nAssets/Scripts/GameManager.cs\nAssets/Scripts/Player.cs')}
              value={deepFiles}
              onChange={e => setDeepFiles(e.target.value)}
              rows={4}
            />
            <button className="btn btn-primary" onClick={handleDeepAnalysis} disabled={loading || !deepFiles.trim()}>
              {t('perf.runDeepAnalysis', '运行深度分析')}
            </button>
          </div>

          {deepReport && (
            <div className="deep-report-content">
              <h4>{t('perf.deepResults', '深度分析结果')}</h4>
              <p>{deepReport.summary}</p>
              {deepReport.source_findings.map((f, i) => (
                <div key={i} className="source-finding">
                  <div className="source-finding-head">
                    <code>{f.file_path}{f.line_number ? `:${f.line_number}` : ''}</code>
                    <span className="finding-category">{f.category}</span>
                    <span className="impact-badge">{f.estimated_impact}</span>
                  </div>
                  <p>{f.issue}</p>
                  <p className="finding-suggestion">💡 {f.suggestion}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ──────────────── Tab: Comparison ────────────────
function ComparisonTab() {
  const { t } = useTranslation();
  const sessions = useAppStore(s => s.profilerSessions);
  const result = useAppStore(s => s.comparisonResult);
  const loading = useAppStore(s => s.profilerLoading);
  const compare = useAppStore(s => s.compareProfilerSessions);
  const exportComp = useAppStore(s => s.exportComparison);
  const [sessionA, setSessionA] = useState('');
  const [sessionB, setSessionB] = useState('');

  const handleCompare = () => {
    if (sessionA && sessionB && sessionA !== sessionB) {
      compare(sessionA, sessionB);
    }
  };

  const handleExport = async () => {
    const path = await exportComp();
    if (path) alert(t('perf.exported', '已导出: ') + path);
  };

  return (
    <div className="perf-comparison">
      <div className="compare-controls">
        <select value={sessionA} onChange={e => setSessionA(e.target.value)} className="input">
          <option value="">{t('perf.sessionA', '会话 A...')}</option>
          {sessions.map((s: SessionMeta) => <option key={s.id} value={s.id}>{s.name}</option>)}
        </select>
        <span className="vs-label">VS</span>
        <select value={sessionB} onChange={e => setSessionB(e.target.value)} className="input">
          <option value="">{t('perf.sessionB', '会话 B...')}</option>
          {sessions.map((s: SessionMeta) => <option key={s.id} value={s.id}>{s.name}</option>)}
        </select>
        <button className="btn btn-primary" onClick={handleCompare} disabled={loading || !sessionA || !sessionB || sessionA === sessionB}>
          {t('perf.compare', '对比')}
        </button>
      </div>

      {result && (
        <div className="compare-result">
          <div className="compare-header">
            <strong>{result.session_a_name}</strong> vs <strong>{result.session_b_name}</strong>
            <button className="btn btn-sm" onClick={handleExport}>{t('perf.export', '导出 Markdown')}</button>
          </div>
          <p className="compare-verdict">{result.verdict}</p>
          <table className="data-table">
            <thead>
              <tr>
                <th>{t('perf.metric', '指标')}</th>
                <th>A</th>
                <th>B</th>
                <th>{t('perf.delta', '变化')}</th>
                <th>{t('perf.status', '状态')}</th>
              </tr>
            </thead>
            <tbody>
              {result.metrics.map((m: ComparisonMetric, i: number) => (
                <tr key={i}>
                  <td>{m.name}</td>
                  <td>{m.value_a.toFixed(1)} {m.unit}</td>
                  <td>{m.value_b.toFixed(1)} {m.unit}</td>
                  <td style={{ color: m.delta > 0 ? (m.improved ? '#44ff44' : '#ff4444') : (m.improved ? '#44ff44' : '#ff4444') }}>
                    {m.delta > 0 ? '+' : ''}{m.delta.toFixed(1)} ({m.delta_percent.toFixed(1)}%)
                  </td>
                  <td>{m.improved ? '✅' : '⚠️'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ──────────────── Main Hub ────────────────
export default function PerformanceHub() {
  const { t } = useTranslation();
  const tab = useAppStore(s => s.profilerTab);
  const setTab = useAppStore(s => s.setProfilerTab);
  const setSelectedProfilerSessionId = useAppStore(s => s.setSelectedProfilerSessionId);

  const handleViewReport = useCallback((sessionId: string) => {
    setSelectedProfilerSessionId(sessionId);
    setTab('report');
  }, [setSelectedProfilerSessionId, setTab]);

  const tabs = [
    { key: 'live', label: t('perf.tabLive', '实时监控'), icon: '📊' },
    { key: 'sessions', label: t('perf.tabSessions', '历史会话'), icon: '📁' },
    { key: 'report', label: t('perf.tabReport', '分析报告'), icon: '🔍' },
    { key: 'compare', label: t('perf.tabCompare', '报告对比'), icon: '⚖️' },
  ];

  return (
    <div className="page-root perf-hub">
      <h2 className="page-title">{t('perf.title', '性能分析工作站')}</h2>
      <div className="perf-tabs">
        {tabs.map(tb => (
          <button
            key={tb.key}
            className={`perf-tab${tab === tb.key ? ' active' : ''}`}
            onClick={() => setTab(tb.key)}
          >
            <span className="perf-tab-icon">{tb.icon}</span>
            {tb.label}
          </button>
        ))}
      </div>
      <div className="perf-tab-content">
        {(tab === 'connect' || tab === 'live') && <LiveMonitorTab />}
        {tab === 'sessions' && <SessionsTab onViewReport={handleViewReport} />}
        {tab === 'report' && <ReportTab />}
        {tab === 'compare' && <ComparisonTab />}
      </div>
    </div>
  );
}
