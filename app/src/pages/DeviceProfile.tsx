import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../api/tauri';
import type { DeviceProfileReport, DeviceStatus, DiscoveredDevice, RemoteSession, TimelinePoint, FunctionAnalysis, FunctionStats, LogAnalysis, PerFrameFunctions, DeviceAiAnalysis } from '../api/tauri';
import { useAppStore } from '../store';
import AiChatPanel from '../components/device/AiChatPanel';
import ReportOverview from '../components/device/pages/ReportOverview';
import ReportRuntimeInfo from '../components/device/pages/ReportRuntimeInfo';
import ReportModuleOverview from '../components/device/pages/ReportModuleOverview';
import ReportCallStacks from '../components/device/pages/ReportCallStacks';
import ReportModuleRendering from '../components/device/pages/ReportModuleRendering';
import ReportModuleGPUSync from '../components/device/pages/ReportModuleGPUSync';
import ReportModuleScripting from '../components/device/pages/ReportModuleScripting';
import ReportModuleUI from '../components/device/pages/ReportModuleUI';
import ReportModuleLoading from '../components/device/pages/ReportModuleLoading';
import ReportModulePhysics from '../components/device/pages/ReportModulePhysics';
import ReportModuleAnimation from '../components/device/pages/ReportModuleAnimation';
import ReportModuleParticle from '../components/device/pages/ReportModuleParticle';
import ReportGPU from '../components/device/pages/ReportGPU';
import ReportJank from '../components/device/pages/ReportJank';
import ReportMemory from '../components/device/pages/ReportMemory';
import ReportBattery from '../components/device/pages/ReportBattery';
import ReportTemperature from '../components/device/pages/ReportTemperature';
import ReportLogs from '../components/device/pages/ReportLogs';
import ReportCustomModules from '../components/device/pages/ReportCustomModules';
import ReportScreenshots from '../components/device/pages/ReportScreenshots';
import ReportHistory from '../components/device/pages/ReportHistory';

type TabKey = 'device' | 'report';
type ReportPage = 'overview' | 'runtime_info' | 'module_overview' | 'call_stacks'
  | 'module_rendering' | 'module_gpu_sync' | 'module_scripting' | 'module_ui'
  | 'module_loading' | 'module_physics' | 'module_animation' | 'module_particles'
  | 'gpu' | 'jank' | 'memory' | 'battery' | 'temperature' | 'logs'
  | 'custom_modules' | 'screenshots' | 'history';

interface SidebarItem {
  key: ReportPage;
  label: string;
  icon: string;
}

interface SidebarGroup {
  label: string;
  items: SidebarItem[];
  collapsible?: boolean;
}

const sidebarGroups: SidebarGroup[] = [
  {
    label: '概览',
    items: [
      { key: 'overview', label: '性能简报', icon: '📊' },
      { key: 'runtime_info', label: '运行信息', icon: '📱' },
    ],
  },
  {
    label: 'CPU 分析',
    items: [
      { key: 'module_overview', label: '模块耗时统计', icon: '⚙️' },
      { key: 'call_stacks', label: 'CPU调用堆栈', icon: '📋' },
      { key: 'module_rendering', label: '渲染模块', icon: '🎨' },
      { key: 'module_gpu_sync', label: 'GPU同步模块', icon: '🔄' },
      { key: 'module_scripting', label: '逻辑代码模块', icon: '💻' },
      { key: 'module_ui', label: 'UI模块', icon: '🖼️' },
      { key: 'module_loading', label: '加载模块', icon: '📦' },
      { key: 'module_physics', label: '物理系统', icon: '⚡' },
      { key: 'module_animation', label: '动画模块', icon: '🎬' },
      { key: 'module_particles', label: '粒子系统', icon: '✨' },
      { key: 'custom_modules', label: '自定义模块', icon: '🔧' },
    ],
  },
  {
    label: '内存',
    collapsible: true,
    items: [
      { key: 'memory', label: '内存分析', icon: '🧠' },
    ],
  },
  {
    label: 'GPU',
    items: [
      { key: 'gpu', label: 'GPU分析', icon: '🖥️' },
    ],
  },
  {
    label: '稳定性',
    items: [
      { key: 'jank', label: '卡顿分析', icon: '⚡' },
      { key: 'logs', label: '运行日志', icon: '📋' },
    ],
  },
  {
    label: '设备',
    items: [
      { key: 'battery', label: '耗电量', icon: '🔋' },
      { key: 'temperature', label: '温度变化', icon: '🌡️' },
      { key: 'screenshots', label: '截图', icon: '📸' },
    ],
  },
  {
    label: '工具',
    items: [
      { key: 'history', label: '历史报告', icon: '📁' },
    ],
  },
];

type LiveDeviceFrame = {
  timestamp: number;
  fps: number;
  cpuTimeMs: number;
  gpuTimeMs: number;
  totalAllocated: number;
  drawCalls: number;
};

function StatCard({ label, value, color, sub }: { label: string; value: string; color?: string; sub?: string }) {
  return (
    <div className="card" style={{ padding: '12px 16px', minWidth: 120, textAlign: 'center' }}>
      <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 22, fontWeight: 'bold', color: color || '#00e0ff' }}>{value}</div>
      {sub && <div style={{ fontSize: 11, color: '#666', marginTop: 2 }}>{sub}</div>}
    </div>
  );
}

function GradeBadge({ grade }: { grade: string }) {
  const colorMap: Record<string, string> = { SSS: '#ff0', SS: '#ff0', S: '#0f0', A: '#0f0', B: '#0cf', C: '#fa0', D: '#f44' };
  const color = colorMap[grade] || '#888';
  return (
    <span style={{ display: 'inline-block', padding: '4px 16px', borderRadius: 8, fontWeight: 'bold', fontSize: 28, background: `${color}22`, color, border: `2px solid ${color}` }}>
      {grade}
    </span>
  );
}

function TimelineChart({ data, color, label, unit }: { data: TimelinePoint[]; color: string; label: string; unit: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap || data.length < 2) return;
    const width = wrap.clientWidth;
    const height = 100;
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, width, height);
    const values = data.map(item => item.value);
    const min = Math.min(...values);
    const max = Math.max(...values) || 1;
    const range = max - min || 1;

    ctx.beginPath();
    data.forEach((item, index) => {
      const x = (index / (data.length - 1)) * width;
      const y = height - ((item.value - min) / range) * (height - 10) - 5;
      if (index === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.stroke();
  }, [data, color]);

  return (
    <div className="card" style={{ padding: 12 }}>
      <div style={{ fontSize: 12, color: '#aaa', marginBottom: 6 }}>{label}</div>
      <div ref={wrapRef} style={{ width: '100%' }}>
        <canvas ref={ref} style={{ width: '100%', display: 'block' }} />
      </div>
      {data.length > 0 && (
        <div style={{ fontSize: 11, color: '#666', marginTop: 6 }}>
          latest: {data[data.length - 1].value.toFixed(1)}{unit}
        </div>
      )}
    </div>
  );
}

function HorizontalBar({ items }: { items: { name: string; avg_ms: number; percentage: number }[] }) {
  const maxMs = Math.max(...items.map(item => item.avg_ms), 0.01);
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      {items.map(item => (
        <div key={item.name} style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ width: 100, fontSize: 11, color: '#aaa', textAlign: 'right' }}>{item.name}</span>
          <div style={{ flex: 1, background: '#1a1a2e', borderRadius: 4, height: 16 }}>
            <div
              style={{
                width: `${(item.avg_ms / maxMs) * 100}%`,
                height: '100%',
                background: 'linear-gradient(90deg, #00e0ff, #7c4dff)',
                borderRadius: 4,
              }}
            />
          </div>
          <span style={{ width: 60, fontSize: 11, color: '#ccc' }}>{item.avg_ms.toFixed(2)}ms</span>
          <span style={{ width: 40, fontSize: 10, color: '#666' }}>{item.percentage.toFixed(0)}%</span>
        </div>
      ))}
    </div>
  );
}

function FpsHistogram({ buckets }: { buckets: { label: string; percentage: number }[] }) {
  const maxPct = Math.max(...buckets.map(bucket => bucket.percentage), 1);
  return (
    <div style={{ display: 'flex', alignItems: 'flex-end', gap: 4, height: 80 }}>
      {buckets.map(bucket => (
        <div key={bucket.label} style={{ flex: 1, textAlign: 'center' }}>
          <div
            style={{
              height: `${(bucket.percentage / maxPct) * 60}px`,
              background: '#00e0ff',
              borderRadius: '4px 4px 0 0',
              minHeight: 2,
            }}
          />
          <div style={{ fontSize: 9, color: '#888', marginTop: 2 }}>{bucket.label}</div>
          <div style={{ fontSize: 9, color: '#aaa' }}>{bucket.percentage.toFixed(0)}%</div>
        </div>
      ))}
    </div>
  );
}

function FunctionAnalysisSection({ analysis, filePath }: { analysis: FunctionAnalysis; filePath: string }) {
  const { t } = useTranslation();
  const [funcTab, setFuncTab] = useState<'overview' | 'functions' | 'frame'>('overview');
  const [selectedFrame, setSelectedFrame] = useState(0);
  const [frameFunctions, setFrameFunctions] = useState<PerFrameFunctions | null>(null);
  const [sortKey, setSortKey] = useState<'total_self_ms' | 'avg_self_ms' | 'avg_call_count'>('total_self_ms');
  const [loadingFrame, setLoadingFrame] = useState(false);

  const sortedFunctions = useMemo(() => {
    const fns = [...analysis.top_functions];
    fns.sort((a, b) => {
      if (sortKey === 'total_self_ms') return b.total_self_ms - a.total_self_ms;
      if (sortKey === 'avg_self_ms') return b.avg_self_ms - a.avg_self_ms;
      return b.avg_call_count - a.avg_call_count;
    });
    return fns;
  }, [analysis.top_functions, sortKey]);

  const loadFrame = useCallback(async (idx: number) => {
    if (!filePath) return;
    setLoadingFrame(true);
    try {
      const data = await api.getFrameFunctions(filePath, idx);
      setFrameFunctions(data);
    } catch {
      setFrameFunctions(null);
    } finally {
      setLoadingFrame(false);
    }
  }, [filePath]);

  const catColors: Record<string, string> = {
    '渲染模块': '#00bcd4', '用户脚本': '#ff9800', '物理模块': '#4caf50',
    '动画模块': '#e91e63', 'UI模块': '#9c27b0', '加载模块': '#607d8b',
    '粒子模块': '#ff5722', '同步等待': '#795548', '引擎开销': '#9e9e9e',
    'GC': '#f44336', '其他': '#455a64',
  };

  return (
    <div className="card" style={{ padding: 16 }}>
      <h3 style={{ margin: '0 0 12px', color: '#ff9800' }}>🔬 {t('device.functionAnalysis', '函数级分析 (深度采样)')}</h3>
      <div style={{ fontSize: 12, color: '#888', marginBottom: 12 }}>
        {t('device.sampledFrames', '采样帧数')}: {analysis.total_sampled_frames}
      </div>

      <div style={{ display: 'flex', gap: 4, marginBottom: 16 }}>
        {(['overview', 'functions', 'frame'] as const).map(key => (
          <button key={key} onClick={() => setFuncTab(key)}
            style={{ padding: '4px 16px', fontSize: 12, background: funcTab === key ? '#7c4dff' : '#1a1a2e', color: funcTab === key ? '#fff' : '#aaa', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            {key === 'overview' ? t('device.categoryOverview', '分类概览') : key === 'functions' ? t('device.topFunctions', 'Top 函数') : t('device.perFrame', '逐帧分析')}
          </button>
        ))}
      </div>

      {funcTab === 'overview' && (
        <div>
          <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
            {analysis.category_breakdown.map(cat => (
              <div key={cat.category} style={{ flex: '1 1 200px', background: '#0d0d1a', borderRadius: 8, padding: 12, borderLeft: `3px solid ${catColors[cat.category] || '#555'}` }}>
                <div style={{ fontWeight: 'bold', fontSize: 13, color: catColors[cat.category] || '#aaa' }}>{cat.category}</div>
                <div style={{ fontSize: 20, fontWeight: 'bold', color: '#fff', margin: '4px 0' }}>{cat.avg_ms.toFixed(2)}ms</div>
                <div style={{ fontSize: 11, color: '#888' }}>{cat.percentage.toFixed(1)}% · {cat.function_count} {t('device.functions', '个函数')}</div>
              </div>
            ))}
          </div>
          <div style={{ marginTop: 16 }}>
            <HorizontalBar items={analysis.category_breakdown.map(c => ({ name: c.category, avg_ms: c.avg_ms, percentage: c.percentage }))} />
          </div>
        </div>
      )}

      {funcTab === 'functions' && (
        <div>
          <div style={{ marginBottom: 8, display: 'flex', gap: 8 }}>
            <span style={{ fontSize: 11, color: '#888' }}>{t('device.sortBy', '排序')}:</span>
            {(['total_self_ms', 'avg_self_ms', 'avg_call_count'] as const).map(key => (
              <button key={key} onClick={() => setSortKey(key)}
                style={{ fontSize: 11, padding: '2px 8px', background: sortKey === key ? '#333' : 'transparent', color: sortKey === key ? '#fff' : '#888', border: '1px solid #333', borderRadius: 4, cursor: 'pointer' }}>
                {key === 'total_self_ms' ? 'Total Self' : key === 'avg_self_ms' ? 'Avg Self' : 'Calls'}
              </button>
            ))}
          </div>
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
              <thead>
                <tr style={{ color: '#888', borderBottom: '1px solid #333' }}>
                  <th style={{ textAlign: 'left', padding: '4px 6px' }}>{t('device.function', '函数')}</th>
                  <th style={{ textAlign: 'left', padding: '4px 6px' }}>{t('device.category', '分类')}</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Avg Self</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Total Self</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Self%</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Avg Total</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Calls/f</th>
                  <th style={{ textAlign: 'right', padding: '4px 6px' }}>Frames</th>
                </tr>
              </thead>
              <tbody>
                {sortedFunctions.map((fn, i) => (
                  <tr key={i} style={{ borderTop: '1px solid #1a1a2e' }}>
                    <td style={{ padding: '4px 6px', fontFamily: 'monospace', color: '#fff', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={fn.name}>{fn.name}</td>
                    <td style={{ padding: '4px 6px', color: catColors[fn.category] || '#aaa' }}>{fn.category}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: fn.avg_self_ms > 1 ? '#f44' : fn.avg_self_ms > 0.5 ? '#fa0' : '#aaa' }}>{fn.avg_self_ms.toFixed(3)}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: '#ccc' }}>{fn.total_self_ms.toFixed(1)}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: '#ccc' }}>{fn.self_pct.toFixed(1)}%</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: '#aaa' }}>{fn.avg_total_ms.toFixed(3)}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: '#aaa' }}>{fn.avg_call_count.toFixed(1)}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right', color: '#888' }}>{fn.frames_called}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {funcTab === 'frame' && (
        <div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 12 }}>
            <span style={{ fontSize: 12, color: '#aaa' }}>Frame:</span>
            <input type="range" min={0} max={Math.max(0, analysis.total_sampled_frames - 1)} value={selectedFrame}
              onChange={e => setSelectedFrame(Number(e.target.value))} style={{ flex: 1 }} />
            <input type="number" min={0} max={analysis.total_sampled_frames - 1} value={selectedFrame}
              onChange={e => setSelectedFrame(Number(e.target.value))} className="input" style={{ width: 80 }} />
            <button className="btn-secondary" onClick={() => void loadFrame(selectedFrame)} disabled={loadingFrame} style={{ fontSize: 11 }}>
              {loadingFrame ? '...' : t('device.loadFrame', '加载')}
            </button>
          </div>
          {frameFunctions && frameFunctions.functions.length > 0 ? (
            <div style={{ overflowX: 'auto' }}>
              <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
                <thead>
                  <tr style={{ color: '#888', borderBottom: '1px solid #333' }}>
                    <th style={{ textAlign: 'left', padding: '4px 6px' }}>{t('device.function', '函数')}</th>
                    <th style={{ textAlign: 'left', padding: '4px 6px' }}>{t('device.category', '分类')}</th>
                    <th style={{ textAlign: 'right', padding: '4px 6px' }}>Self ms</th>
                    <th style={{ textAlign: 'right', padding: '4px 6px' }}>Total ms</th>
                    <th style={{ textAlign: 'right', padding: '4px 6px' }}>Calls</th>
                    <th style={{ textAlign: 'right', padding: '4px 6px' }}>Depth</th>
                  </tr>
                </thead>
                <tbody>
                  {frameFunctions.functions
                    .slice()
                    .sort((a, b) => b.self_ms - a.self_ms)
                    .map((fn, i) => (
                      <tr key={i} style={{ borderTop: '1px solid #1a1a2e', paddingLeft: fn.depth * 12 }}>
                        <td style={{ padding: '4px 6px', paddingLeft: 6 + fn.depth * 12, fontFamily: 'monospace', color: '#fff', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={fn.name}>
                          {'  '.repeat(fn.depth)}{fn.name}
                        </td>
                        <td style={{ padding: '4px 6px', color: catColors[fn.category] || '#aaa' }}>{fn.category}</td>
                        <td style={{ padding: '4px 6px', textAlign: 'right', color: fn.self_ms > 2 ? '#f44' : fn.self_ms > 0.5 ? '#fa0' : '#aaa' }}>{fn.self_ms.toFixed(3)}</td>
                        <td style={{ padding: '4px 6px', textAlign: 'right', color: '#ccc' }}>{fn.total_ms.toFixed(3)}</td>
                        <td style={{ padding: '4px 6px', textAlign: 'right', color: '#aaa' }}>{fn.call_count}</td>
                        <td style={{ padding: '4px 6px', textAlign: 'right', color: '#888' }}>{fn.depth}</td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div style={{ color: '#888', fontSize: 12, textAlign: 'center', padding: 20 }}>
              {loadingFrame ? t('device.loading', '加载中...') : t('device.clickLoad', '点击"加载"查看该帧函数数据')}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function LogAnalysisSection({ analysis }: { analysis: LogAnalysis }) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<string | null>(null);

  return (
    <div className="card" style={{ padding: 16 }}>
      <h3 style={{ margin: '0 0 12px', color: '#f44336' }}>📋 {t('device.runtimeLogs', '运行时日志')}</h3>
      <div style={{ display: 'flex', gap: 16, marginBottom: 16, fontSize: 12 }}>
        <span style={{ color: '#aaa' }}>Total: <b>{analysis.total_logs}</b></span>
        <span style={{ color: '#f44' }}>Errors: <b>{analysis.error_count}</b></span>
        <span style={{ color: '#fa0' }}>Warnings: <b>{analysis.warning_count}</b></span>
        <span style={{ color: '#f44' }}>Exceptions: <b>{analysis.exception_count}</b></span>
      </div>

      {analysis.top_errors.length > 0 && (
        <>
          <h4 style={{ color: '#f44', margin: '8px 0', fontSize: 13 }}>{t('device.topErrors', 'Top 错误')}</h4>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {analysis.top_errors.map((e, i) => (
              <div key={i} style={{ background: '#1a0a0a', borderRadius: 6, padding: '6px 12px', cursor: 'pointer', border: '1px solid #331111' }}
                onClick={() => setExpanded(expanded === `err-${i}` ? null : `err-${i}`)}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontFamily: 'monospace', fontSize: 11, color: '#f88', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {e.message}
                  </span>
                  <span style={{ fontSize: 11, color: '#f44', fontWeight: 'bold', marginLeft: 8 }}>×{e.count}</span>
                  <span style={{ fontSize: 10, color: '#888', marginLeft: 8 }}>frame #{e.first_frame}</span>
                </div>
                {expanded === `err-${i}` && (
                  <div style={{ marginTop: 4, fontSize: 11, color: '#aaa', whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>
                    {e.message}
                  </div>
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {analysis.top_warnings.length > 0 && (
        <>
          <h4 style={{ color: '#fa0', margin: '12px 0 8px', fontSize: 13 }}>{t('device.topWarnings', 'Top 警告')}</h4>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {analysis.top_warnings.slice(0, 10).map((w, i) => (
              <div key={i} style={{ background: '#1a1300', borderRadius: 6, padding: '6px 12px', border: '1px solid #332200' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontFamily: 'monospace', fontSize: 11, color: '#fa0', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {w.message}
                  </span>
                  <span style={{ fontSize: 11, color: '#fa0', fontWeight: 'bold', marginLeft: 8 }}>×{w.count}</span>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function AiDeviceSection({ filePath, report }: { filePath: string; report: DeviceProfileReport }) {
  const { t } = useTranslation();
  const settings = useAppStore(s => s.settings);
  const [aiResult, setAiResult] = useState<DeviceAiAnalysis | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runAnalysis = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await api.runAiDeviceAnalysis(filePath, settings.ai_cli, settings.ai_model ?? undefined, settings.ai_thinking ?? undefined);
      setAiResult(result);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [filePath, settings.ai_cli, settings.ai_model, settings.ai_thinking]);

  return (
    <div className="card" style={{ padding: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h3 style={{ margin: 0, color: '#7c4dff' }}>🤖 {t('device.aiAnalysis', 'AI 智能分析')}</h3>
        <button className="btn-primary" onClick={() => void runAnalysis()} disabled={loading}
          style={{ fontSize: 12, padding: '6px 16px' }}>
          {loading ? t('device.analyzing', '分析中...') : t('device.runAiAnalysis', '运行 AI 分析')}
        </button>
      </div>
      <div style={{ fontSize: 11, color: '#888', marginBottom: 8 }}>
        CLI: {settings.ai_cli} {settings.ai_model ? `(${settings.ai_model})` : ''} · {t('device.grade', '评级')}: {report.overall_grade}
      </div>

      {error && (
        <div style={{ color: '#f44', background: '#1a0a0a', borderRadius: 6, padding: 8, fontSize: 12, marginBottom: 8 }}>
          {error}
        </div>
      )}

      {aiResult && (
        <div style={{ marginTop: 8 }}>
          <div style={{ fontSize: 11, color: '#666', marginBottom: 8 }}>
            {aiResult.timestamp}
          </div>
          <div style={{ background: '#0d0d1a', borderRadius: 8, padding: 16, fontSize: 13, lineHeight: 1.7, color: '#ddd', whiteSpace: 'pre-wrap', maxHeight: 600, overflow: 'auto' }}>
            {aiResult.analysis}
          </div>
        </div>
      )}
    </div>
  );
}

export default function DeviceProfile() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<TabKey>('device');
  const [devices, setDevices] = useState<DiscoveredDevice[]>([]);
  const [selected, setSelected] = useState<DiscoveredDevice | null>(null);
  const [deviceStatus, setDeviceStatus] = useState<DeviceStatus | null>(null);
  const [sessions, setSessions] = useState<RemoteSession[]>([]);
  const [liveFrames, setLiveFrames] = useState<LiveDeviceFrame[]>([]);
  const [report, setReport] = useState<DeviceProfileReport | null>(null);
  const [filePath, setFilePath] = useState('');
  const [manualIp, setManualIp] = useState('');
  const [manualPort, setManualPort] = useState('9527');
  const [sessionName, setSessionName] = useState('');
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [reportPage, setReportPage] = useState<ReportPage>('overview');
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [screenshotFrame, setScreenshotFrame] = useState<number | null>(null);
  const [screenshotData, setScreenshotData] = useState<string | null>(null);
  const previousCapturingRef = useRef(false);
  const suppressAutoOpenRef = useRef(false);

  const currentStatus = deviceStatus ?? selected?.status ?? null;
  const isCapturing = currentStatus?.capturing ?? false;

  const refreshStatus = useCallback(async (silent = true) => {
    if (!selected) return null;
    try {
      const status = await api.getDeviceStatus(selected.ip, selected.port);
      setDeviceStatus(status);
      setDevices(prev => prev.map(device => device.ip === selected.ip && device.port === selected.port ? { ...device, status } : device));
      return status;
    } catch (e) {
      setDeviceStatus(prev => prev ? { ...prev, capturing: false, currentFps: 0 } : prev);
      if (!silent) setError(String(e));
      return null;
    }
  }, [selected]);

  const refreshSessions = useCallback(async (silent = false) => {
    if (!selected) return [] as RemoteSession[];
    try {
      const list = await api.listDeviceSessions(selected.ip, selected.port);
      setSessions(list);
      return list;
    } catch (e) {
      if (!silent) setError(String(e));
      return [] as RemoteSession[];
    }
  }, [selected]);

  const openDownloadedReport = useCallback(async (downloadedPath: string) => {
    setLoading(true);
    setError(null);
    try {
      setFilePath(downloadedPath);
      const generated = await api.generateDeviceReport(downloadedPath);
      setReport(generated);
      try {
        await api.saveDeviceReport(downloadedPath);
      } catch {
        // History is project-scoped; ignore when no project is loaded.
      }
      setTab('report');
      setMessage(`${t('device.autoOpenedReport', '已自动打开报告: ')}${downloadedPath.split(/[\\/]/).pop()}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [t]);

  const openSavedReport = useCallback(async (reportId: string) => {
    setLoading(true);
    setError(null);
    try {
      const saved = await api.getSavedDeviceReport(reportId);
      setReport(saved);
      setFilePath(saved.source_file_path || '');
      setTab('report');
      setMessage(`${t('device.autoOpenedReport', '已打开历史报告: ')}${saved.session_name}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    if (!selected) {
      setDeviceStatus(null);
      setSessions([]);
      setLiveFrames([]);
      return;
    }
    setDeviceStatus(selected.status);
    void refreshStatus();
    void refreshSessions(true);
  }, [selected, refreshStatus, refreshSessions]);

  useEffect(() => {
    if (!selected) return;
    const timer = setInterval(() => { void refreshStatus(); }, 1500);
    return () => clearInterval(timer);
  }, [selected, refreshStatus]);

  useEffect(() => {
    if (!selected || typeof EventSource === 'undefined') {
      setLiveFrames([]);
      return;
    }
    const source = new EventSource(`http://${selected.ip}:${selected.port}/live`);
    source.onmessage = event => {
      try {
        const payload = JSON.parse(event.data) as LiveDeviceFrame;
        setLiveFrames(prev => {
          const next = [...prev, payload];
          if (next.length > 240) next.shift();
          return next;
        });
        setDeviceStatus(prev => prev ? { ...prev, capturing: true, currentFps: payload.fps, frameCount: prev.frameCount + 1, elapsed: Math.max(prev.elapsed, payload.timestamp) } : prev);
      } catch {
        // ignore malformed event
      }
    };
    source.onerror = () => {
      source.close();
      setDeviceStatus(prev => prev ? { ...prev, capturing: false } : prev);
    };
    return () => source.close();
  }, [selected]);

  const handleScan = async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const found = await api.discoverDevices();
      setDevices(found);
      if (found.length === 1) setSelected(found[0]);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleManualConnect = async () => {
    const ip = manualIp.trim();
    const port = Number.parseInt(manualPort, 10);
    if (!ip || Number.isNaN(port)) {
      setError(t('device.invalidEndpoint', '请输入有效的 IP 和端口'));
      return;
    }

    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const status = await api.getDeviceStatus(ip, port);
      const device: DiscoveredDevice = { ip, port, status };
      setSelected(device);
      setDevices(prev => {
        const exists = prev.some(item => item.ip === ip && item.port === port);
        return exists ? prev.map(item => item.ip === ip && item.port === port ? device : item) : [device, ...prev];
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleStartCapture = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    setLiveFrames([]);
    setDeviceStatus(prev => prev ? { ...prev, capturing: true, frameCount: 0, elapsed: 0 } : prev);
    try {
      await api.remoteStartCapture(selected.ip, selected.port, sessionName.trim() || undefined);
      await refreshStatus(false);
    } catch (e) {
      setError(String(e));
      setDeviceStatus(prev => prev ? { ...prev, capturing: false } : prev);
    } finally {
      setBusy(false);
    }
  };

  const handleStopCapture = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    suppressAutoOpenRef.current = true;
    try {
      const stopResult = await api.remoteStopCapture(selected.ip, selected.port);
      setDeviceStatus(prev => prev ? { ...prev, capturing: false } : prev);
      const sessionsAfterStop = await refreshSessions(true);
      const stopFileName = stopResult.filePath.split(/[\\/]/).pop() || '';
      const fallbackFile = sessionsAfterStop.slice().sort((a, b) => b.created.localeCompare(a.created))[0]?.fileName || '';
      const targetFile = stopFileName || fallbackFile;
      if (!targetFile) {
        setMessage(t('device.reportPending', '采集已结束，但还未找到报告文件，请稍后刷新会话列表。'));
        return;
      }
      const downloadedPath = await api.downloadDeviceSession(selected.ip, selected.port, targetFile);
      await openDownloadedReport(downloadedPath);
    } catch (e) {
      setError(String(e));
    } finally {
      window.setTimeout(() => {
        suppressAutoOpenRef.current = false;
      }, 1500);
      setBusy(false);
    }
  };

  const handleDownload = async (fileName: string) => {
    if (!selected) return;
    setDownloading(fileName);
    setError(null);
    try {
      const downloadedPath = await api.downloadDeviceSession(selected.ip, selected.port, fileName);
      setMessage(`${t('device.downloaded', '已下载: ')}${downloadedPath}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setDownloading(null);
    }
  };

  const handleImport = async () => {
    try {
      setError(null);
      setMessage(null);
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selectedPath = await open({
        title: t('device.selectGaprof', '选择 .gaprof 文件'),
        filters: [{ name: 'GAProf', extensions: ['gaprof'] }],
      });
      if (!selectedPath) return;
      await openDownloadedReport(selectedPath as string);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleExport = async () => {
    if (!report) return;
    try {
      setError(null);
      const path = await api.exportDeviceReport(report);
      setMessage(`${t('device.exported', '已导出: ')}${path}`);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleScreenshot = async (frameIndex: number) => {
    if (!filePath) return;
    setScreenshotFrame(frameIndex);
    try {
      const base64 = await api.getDeviceScreenshot(filePath, frameIndex);
      setScreenshotData(base64);
    } catch (e) {
      setError(String(e));
      setScreenshotData(null);
    }
  };

  const fpsTimeline = useMemo(() => liveFrames.map(frame => ({ time: frame.timestamp, value: frame.fps })), [liveFrames]);
  const cpuTimeline = useMemo(() => liveFrames.map(frame => ({ time: frame.timestamp, value: frame.cpuTimeMs })), [liveFrames]);
  const gpuTimeline = useMemo(() => liveFrames.map(frame => ({ time: frame.timestamp, value: frame.gpuTimeMs })), [liveFrames]);
  const memoryTimeline = useMemo(() => liveFrames.map(frame => ({ time: frame.timestamp, value: frame.totalAllocated / 1024 / 1024 })), [liveFrames]);

  useEffect(() => {
    const wasCapturing = previousCapturingRef.current;
    const nowCapturing = !!currentStatus?.capturing;
    previousCapturingRef.current = nowCapturing;

    if (!selected || suppressAutoOpenRef.current || !wasCapturing || nowCapturing) {
      return;
    }

    let cancelled = false;
    const tryOpenLatestReport = async () => {
      const latestSessions = await refreshSessions(true);
      const latest = latestSessions.slice().sort((a, b) => b.created.localeCompare(a.created))[0];
      if (!latest || cancelled) return;
      try {
        const downloadedPath = await api.downloadDeviceSession(selected.ip, selected.port, latest.fileName);
        if (cancelled) return;
        await openDownloadedReport(downloadedPath);
      } catch {
        // Device may already be offline when auto-stop happens; avoid noisy UI errors here.
      }
    };

    void tryOpenLatestReport();
    return () => {
      cancelled = true;
    };
  }, [currentStatus?.capturing, openDownloadedReport, refreshSessions, selected]);

  return (
    <div className="page-container" style={{ padding: 24 }}>
      <div style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
        <button onClick={() => setTab('device')} className={tab === 'device' ? 'btn-primary' : 'btn-secondary'}>
          {t('device.deviceTab', '📡 设备连接')}
        </button>
        <button onClick={() => setTab('report')} className={tab === 'report' ? 'btn-primary' : 'btn-secondary'}>
          {t('device.reportTab', '📊 性能报告')}
        </button>
      </div>

      {error && <div className="ai-error" style={{ marginBottom: 16 }}>{error}</div>}
      {message && <div className="export-toast" style={{ marginBottom: 16 }}>{message}</div>}

      {tab === 'device' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div className="card" style={{ padding: 16 }}>
            <h3 style={{ margin: '0 0 12px', color: '#00e0ff' }}>📡 {t('device.scan', '设备发现')}</h3>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
              <button className="btn-primary" onClick={handleScan} disabled={busy}>
                {busy ? t('device.scanning', '扫描中...') : t('device.scanNetwork', '扫描局域网')}
              </button>
              <input type="text" value={manualIp} onChange={e => setManualIp(e.target.value)} placeholder={t('device.manualIp', '手动输入设备 IP')} className="input" style={{ width: 180 }} />
              <input type="number" value={manualPort} onChange={e => setManualPort(e.target.value)} placeholder={t('device.manualPort', '端口')} className="input" style={{ width: 100 }} />
              <button className="btn-secondary" onClick={handleManualConnect} disabled={busy}>
                {t('device.manualConnect', '手动连接')}
              </button>
            </div>

            {devices.length > 0 && (
              <div style={{ marginTop: 12, display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                {devices.map(device => (
                  <div
                    key={`${device.ip}:${device.port}`}
                    className="card"
                    onClick={() => setSelected(device)}
                    style={{ padding: '8px 16px', cursor: 'pointer', border: selected?.ip === device.ip ? '1px solid #00e0ff' : '1px solid transparent' }}
                  >
                    <div style={{ fontWeight: 'bold' }}>{device.status.deviceModel}</div>
                    <div style={{ fontSize: 11, color: '#888' }}>{device.ip}:{device.port}</div>
                    <div style={{ fontSize: 11, color: '#aaa' }}>{device.status.projectName}</div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {selected && (
            <div className="card" style={{ padding: 16 }}>
              <h3 style={{ margin: '0 0 12px', color: '#00e0ff' }}>
                📱 {currentStatus?.deviceModel || selected.status.deviceModel} — {selected.ip}
              </h3>
              <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 12 }}>
                <StatCard label="FPS" value={(currentStatus?.currentFps ?? 0).toFixed(0)} />
                <StatCard label={t('device.frames', '帧数')} value={`${currentStatus?.frameCount ?? 0}`} />
                <StatCard label={t('device.elapsed', '时长')} value={`${(currentStatus?.elapsed ?? 0).toFixed(0)}s`} />
                <StatCard label={t('device.status', '状态')} value={isCapturing ? '● REC' : '○ IDLE'} color={isCapturing ? '#f44' : '#888'} />
              </div>

              <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 16 }}>
                <input type="text" placeholder={t('device.sessionName', '会话名称 (可选)')} value={sessionName} onChange={e => setSessionName(e.target.value)} className="input" style={{ width: 220 }} />
                {!isCapturing ? (
                  <button className="btn-primary" onClick={handleStartCapture} disabled={busy}>
                    ▶ {t('device.startCapture', '开始录制')}
                  </button>
                ) : (
                  <button className="btn-danger" onClick={handleStopCapture} disabled={busy}>
                    ⏹ {t('device.stopCapture', '停止录制')}
                  </button>
                )}
                <button className="btn-secondary" onClick={() => void refreshSessions()} disabled={busy}>
                  ↻ {t('device.refreshSessions', '刷新会话')}
                </button>
              </div>

              {sessions.length > 0 && (
                <div>
                  <h4 style={{ color: '#aaa', margin: '0 0 8px' }}>{t('device.sessions', '设备上的会话')}</h4>
                  <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                    <thead>
                      <tr style={{ color: '#888', fontSize: 12 }}>
                        <th style={{ textAlign: 'left', padding: '4px 8px' }}>{t('device.fileName', '文件名')}</th>
                        <th style={{ textAlign: 'right', padding: '4px 8px' }}>{t('device.size', '大小')}</th>
                        <th style={{ textAlign: 'right', padding: '4px 8px' }}>{t('device.created', '创建时间')}</th>
                        <th style={{ textAlign: 'center', padding: '4px 8px' }} />
                      </tr>
                    </thead>
                    <tbody>
                      {sessions.map(session => (
                        <tr key={session.fileName} style={{ borderTop: '1px solid #222' }}>
                          <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontSize: 12 }}>{session.fileName}</td>
                          <td style={{ padding: '6px 8px', textAlign: 'right', color: '#aaa', fontSize: 12 }}>{(session.sizeBytes / 1024 / 1024).toFixed(1)} MB</td>
                          <td style={{ padding: '6px 8px', textAlign: 'right', color: '#888', fontSize: 12 }}>{session.created}</td>
                          <td style={{ padding: '6px 8px', textAlign: 'center' }}>
                            <button className="btn-secondary" onClick={() => void handleDownload(session.fileName)} disabled={downloading === session.fileName} style={{ fontSize: 11, padding: '2px 12px' }}>
                              {downloading === session.fileName ? '...' : `⬇ ${t('device.download', '下载')}`}
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              {liveFrames.length > 1 && (
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 12, marginTop: 16 }}>
                  <TimelineChart data={fpsTimeline} color="#00e0ff" label="FPS" unit=" fps" />
                  <TimelineChart data={cpuTimeline} color="#ff9800" label="CPU" unit=" ms" />
                  <TimelineChart data={gpuTimeline} color="#9c27b0" label="GPU" unit=" ms" />
                  <TimelineChart data={memoryTimeline} color="#4caf50" label={t('device.memTimeline', '内存时间线')} unit=" MB" />
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {tab === 'report' && (
        <div>
          {!report ? (
            <div className="card" style={{ padding: 40, textAlign: 'center' }}>
              <h3 style={{ color: '#aaa', margin: '0 0 16px' }}>📂 {t('device.importTitle', '导入性能数据')}</h3>
              <p style={{ color: '#666', marginBottom: 20 }}>
                {t('device.importDesc', '从设备下载或手动导入 .gaprof 文件，获取完整的性能分析报告')}
              </p>
              <button className="btn-primary" onClick={handleImport} style={{ fontSize: 16, padding: '12px 32px' }}>
                📁 {t('device.selectFile', '选择文件')}
              </button>
            </div>
          ) : (
            <div style={{ display: 'flex', gap: 0, height: 'calc(100vh - 160px)' }}>
              {/* Sidebar */}
              <div style={{ width: 200, background: '#0d0d1a', borderRight: '1px solid #222', overflowY: 'auto', flexShrink: 0, paddingTop: 8 }}>
                {/* Device info header */}
                <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', marginBottom: 4 }}>
                  <div style={{ fontSize: 12, fontWeight: 600, color: '#ccc', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{report.session_name}</div>
                  <div style={{ fontSize: 10, color: '#888' }}>{report.device_info.device_model}</div>
                  <div style={{ display: 'flex', gap: 6, marginTop: 4, fontSize: 10, color: '#666' }}>
                    <span>{report.overall_grade}</span>
                    <span>{report.summary.avg_fps.toFixed(0)} fps</span>
                    <span>{report.total_frames}f</span>
                  </div>
                </div>

                {sidebarGroups.map(group => (
                  <div key={group.label} style={{ marginBottom: 2 }}>
                    <div style={{ padding: '4px 12px', fontSize: 10, color: '#666', textTransform: 'uppercase', letterSpacing: 1 }}>{group.label}</div>
                    {group.items.map(item => (
                      <div key={item.key} onClick={() => setReportPage(item.key)}
                        style={{
                          padding: '5px 12px', fontSize: 12, cursor: 'pointer', display: 'flex', gap: 6, alignItems: 'center',
                          background: reportPage === item.key ? '#16213e' : 'transparent',
                          color: reportPage === item.key ? '#4fc3f7' : '#aaa',
                          borderLeft: reportPage === item.key ? '2px solid #4fc3f7' : '2px solid transparent',
                        }}>
                        <span style={{ fontSize: 12 }}>{item.icon}</span>
                        <span>{item.label}</span>
                      </div>
                    ))}
                  </div>
                ))}

                {/* Toolbar */}
                <div style={{ padding: '12px', borderTop: '1px solid #222', marginTop: 8 }}>
                  <button onClick={handleImport} style={{ width: '100%', padding: '4px 0', fontSize: 11, background: '#1a1a2e', color: '#aaa', border: '1px solid #333', borderRadius: 4, cursor: 'pointer', marginBottom: 4 }}>
                    📁 加载其它
                  </button>
                  <button onClick={handleExport} style={{ width: '100%', padding: '4px 0', fontSize: 11, background: '#1a1a2e', color: '#aaa', border: '1px solid #333', borderRadius: 4, cursor: 'pointer', marginBottom: 4 }}>
                    📤 导出报告
                  </button>
                  <button onClick={() => setShowAiPanel(!showAiPanel)} style={{ width: '100%', padding: '4px 0', fontSize: 11, background: showAiPanel ? '#7c4dff' : '#1a1a2e', color: showAiPanel ? '#fff' : '#aaa', border: '1px solid #333', borderRadius: 4, cursor: 'pointer' }}>
                    🤖 AI 分析
                  </button>
                </div>
              </div>

              {/* Main content */}
              <div style={{ flex: 1, overflowY: 'auto', padding: 16 }}>
                {reportPage === 'overview' && <ReportOverview report={report} onNavigate={(p) => setReportPage(p as ReportPage)} />}
                {reportPage === 'runtime_info' && <ReportRuntimeInfo report={report} />}
                {reportPage === 'module_overview' && <ReportModuleOverview report={report} onNavigate={(p) => setReportPage(p as ReportPage)} />}
                {reportPage === 'call_stacks' && <ReportCallStacks filePath={filePath} />}
                {reportPage === 'module_rendering' && <ReportModuleRendering filePath={filePath} />}
                {reportPage === 'module_gpu_sync' && <ReportModuleGPUSync filePath={filePath} />}
                {reportPage === 'module_scripting' && <ReportModuleScripting filePath={filePath} />}
                {reportPage === 'module_ui' && <ReportModuleUI filePath={filePath} />}
                {reportPage === 'module_loading' && <ReportModuleLoading filePath={filePath} />}
                {reportPage === 'module_physics' && <ReportModulePhysics filePath={filePath} />}
                {reportPage === 'module_animation' && <ReportModuleAnimation filePath={filePath} />}
                {reportPage === 'module_particles' && <ReportModuleParticle filePath={filePath} />}
                {reportPage === 'gpu' && <ReportGPU filePath={filePath} />}
                {reportPage === 'jank' && <ReportJank filePath={filePath} report={report} />}
                {reportPage === 'memory' && <ReportMemory filePath={filePath} report={report} />}
                {reportPage === 'battery' && <ReportBattery report={report} />}
                {reportPage === 'temperature' && <ReportTemperature report={report} />}
                {reportPage === 'logs' && <ReportLogs report={report} />}
                {reportPage === 'custom_modules' && <ReportCustomModules filePath={filePath} />}
                {reportPage === 'screenshots' && <ReportScreenshots filePath={filePath} report={report} />}
                {reportPage === 'history' && <ReportHistory onLoadReport={(reportId) => { void openSavedReport(reportId); }} />}
              </div>

              {/* AI Chat Panel */}
              {showAiPanel && (
                <div style={{ width: 360, borderLeft: '1px solid #222', flexShrink: 0 }}>
                  <AiChatPanel filePath={filePath} context={`Report grade: ${report.overall_grade}, Avg FPS: ${report.summary.avg_fps.toFixed(1)}, Peak Mem: ${report.summary.peak_memory_mb.toFixed(0)}MB, Jank: ${report.summary.jank_count}`} />
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
