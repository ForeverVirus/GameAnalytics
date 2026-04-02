import React, { useEffect, useState } from 'react';
import { api, listenAiLog } from '../../api/tauri';
import type { ModulePageAnalysis, TimelinePoint, DeviceAiAnalysis, PerFrameFunctions } from '../../api/tauri';
import { useAppStore } from '../../store';
import FrameTimeline from './FrameTimeline';
import MetricCards from './MetricCards';
import FunctionTable from './FunctionTable';

interface ModulePageProps {
  filePath: string;
  moduleName: string;
}

export const ModulePage: React.FC<ModulePageProps> = ({ filePath, moduleName }) => {
  const [analysis, setAnalysis] = useState<ModulePageAnalysis | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [aiResult, setAiResult] = useState<string | null>(null);
  const [selectedPoint, setSelectedPoint] = useState<TimelinePoint | null>(null);
  const [selectedFrameFunctions, setSelectedFrameFunctions] = useState<PerFrameFunctions | null>(null);
  const [selectedFrameLoading, setSelectedFrameLoading] = useState(false);
  const [selectedFrameLoaded, setSelectedFrameLoaded] = useState(false);
  const settings = useAppStore(s => s.settings);
  const globalAiLoading = useAppStore(s => s.aiLoading);
  const globalAiNodeId = useAppStore(s => s.aiNodeId);
  const aiLiveLog = useAppStore(s => s.aiLiveLog);

  useEffect(() => {
    setLoading(true);
    setError(null);
    setSelectedPoint(null);
    setSelectedFrameFunctions(null);
    setSelectedFrameLoaded(false);
    api.getModuleAnalysis(filePath, moduleName)
      .then(data => { setAnalysis(data); setLoading(false); })
      .catch(err => { setError(String(err)); setLoading(false); });
  }, [filePath, moduleName]);

  if (loading) return <div style={{ padding: 20, color: '#888' }}>加载 {moduleName} 模块分析...</div>;
  if (error) return <div style={{ padding: 20, color: '#e57373' }}>加载失败: {error}</div>;
  if (!analysis) return null;

  const moduleCategoryIds: Partial<Record<string, number[]>> = {
    rendering: [0],
    gpu_sync: [7],
    scripting: [1],
    ui: [4],
    loading: [5],
    physics: [2],
    animation: [3],
    particles: [6],
    gpu: [0, 7],
  };

  const selectedFrameIndex = selectedPoint?.frame_index;
  const supportsFrameFunctions = analysis.function_sampling_enabled && moduleCategoryIds[moduleName] !== undefined;
  const aiTaskId = `device-module::${analysis.module_label}`;
  const aiTaskRunning = globalAiLoading && globalAiNodeId === aiTaskId;
  const topFunctionsEmptyMessage =
    analysis.function_sampling_enabled
        ? '本模块在当前报告中没有函数采样样本'
        : '本次采集未启用深度采样，无法显示函数采样数据';

  const handleAiAnalysis = async () => {
    if (!settings.ai_cli || globalAiLoading) return;
    setAiResult(null);
    useAppStore.setState({
      aiLoading: true,
      aiNodeId: aiTaskId,
      aiLiveLog: [],
      aiError: null,
      error: null,
    });
    let unlistenLog: (() => void) | null = null;
    try {
      unlistenLog = await listenAiLog((line) => {
        useAppStore.setState((state) => ({ aiLiveLog: [...state.aiLiveLog, line] }));
      });
      const result: DeviceAiAnalysis = await api.runAiModuleAnalysis(
        filePath,
        moduleName,
        settings.ai_cli,
        settings.ai_model || undefined,
        settings.ai_thinking || undefined,
      );
      setAiResult(result.analysis);
    } catch (err) {
      setAiResult(`分析失败: ${err}`);
      useAppStore.setState({ error: String(err) });
    } finally {
      unlistenLog?.();
      useAppStore.setState({ aiLoading: false, aiNodeId: null });
    }
  };

  const handleFrameClick = async (point: TimelinePoint) => {
    setSelectedPoint(point);
    setSelectedFrameLoaded(false);
    setSelectedFrameFunctions(null);

    if (!filePath || !supportsFrameFunctions || point.frame_index === undefined) {
      return;
    }

    setSelectedFrameLoading(true);
    try {
      const data = await api.getFrameFunctions(filePath, point.frame_index, moduleCategoryIds[moduleName], true);
      setSelectedFrameFunctions(data);
    } finally {
      setSelectedFrameLoading(false);
      setSelectedFrameLoaded(true);
    }
  };

  const mainSeries = [{
    name: analysis.module_label,
    data: analysis.timeline,
    color: '#4fc3f7',
  }];

  const subSeries = analysis.sub_timelines.map((st, i) => ({
    name: st.name,
    data: st.timeline,
    color: ['#81c784', '#ffb74d', '#e57373', '#ba68c8'][i % 4],
  }));

  const allSeries = [...mainSeries, ...subSeries];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>{analysis.module_label}</h3>
        <div style={{ display: 'flex', gap: 16, fontSize: 13, color: '#aaa' }}>
          <span>平均: <b style={{ color: analysis.avg_module_ms > 5 ? '#ffb74d' : '#81c784' }}>{analysis.avg_module_ms.toFixed(2)}ms</b></span>
          <span>最大: <b style={{ color: analysis.max_module_ms > 16 ? '#e57373' : '#ccc' }}>{analysis.max_module_ms.toFixed(2)}ms</b></span>
          <span>占比: <b style={{ color: '#4fc3f7' }}>{analysis.percentage_of_total.toFixed(1)}%</b></span>
          <span>采样: {analysis.sampled_frames}帧</span>
        </div>
      </div>

      {/* Metrics */}
      <MetricCards entries={analysis.metrics.entries} />

      {/* Timeline Chart */}
      <div style={{ background: '#16213e', borderRadius: 6, padding: 12 }}>
        <div style={{ fontSize: 12, color: '#888', marginBottom: 6 }}>时间线 (ms)</div>
        <FrameTimeline
          series={allSeries}
          height={220}
          yLabel="ms"
          threshold={16.67}
          thresholdLabel="60fps"
          selectedTime={selectedPoint?.time ?? null}
          onFrameClick={(point) => { void handleFrameClick(point); }}
        />
      </div>

      <div style={{ background: '#16213e', borderRadius: 6, padding: 12 }}>
        <div style={{ fontSize: 12, color: '#888', marginBottom: 6 }}>选中帧</div>
        {selectedPoint ? (
          <div style={{ display: 'flex', gap: 20, flexWrap: 'wrap', fontSize: 12 }}>
            <span style={{ color: '#ccc' }}>
              帧号: <b style={{ color: '#ffd54f' }}>{selectedFrameIndex ?? 'N/A'}</b>
            </span>
            <span style={{ color: '#ccc' }}>
              时间: <b style={{ color: '#4fc3f7' }}>{selectedPoint.time.toFixed(3)}s</b>
            </span>
            <span style={{ color: '#ccc' }}>
              {analysis.module_label}: <b style={{ color: '#81c784' }}>{selectedPoint.value.toFixed(2)}ms</b>
            </span>
          </div>
        ) : (
          <div style={{ fontSize: 12, color: '#666' }}>点击上方时间线中的任一点，查看对应采样帧。</div>
        )}
      </div>

      {/* Sub-timelines summary */}
      {analysis.sub_timelines.length > 0 && (
        <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
          {analysis.sub_timelines.map((st, i) => (
            <div key={i} style={{ background: '#16213e', borderRadius: 6, padding: '8px 14px', minWidth: 130 }}>
              <div style={{ fontSize: 11, color: '#888' }}>{st.name}</div>
              <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc' }}>{st.avg_ms.toFixed(2)}ms</div>
              <div style={{ fontSize: 10, color: '#666' }}>max: {st.max_ms.toFixed(2)}ms</div>
            </div>
          ))}
        </div>
      )}

      {/* Top Functions */}
      {!analysis.function_sampling_enabled && (
        <div style={{ background: '#2a1f12', color: '#ffcc80', border: '1px solid #5d4037', borderRadius: 6, padding: 12, fontSize: 12 }}>
          本次报告未启用深度采样，模块函数排行和逐帧函数详情不可用。请在 Unity SDK 配置中开启 `enableDeepProfiling` 后重新采集。
        </div>
      )}

      <FunctionTable functions={analysis.top_functions} title="函数开销排行" emptyMessage={topFunctionsEmptyMessage} />

      {selectedPoint && (
        <div style={{ background: '#16213e', borderRadius: 6, padding: 12 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: '#ccc', marginBottom: 8 }}>
            选中帧函数详情
          </div>
          {selectedFrameLoading && <div style={{ fontSize: 12, color: '#888' }}>加载中...</div>}
          {!selectedFrameLoading && !supportsFrameFunctions && (
            <div style={{ fontSize: 12, color: '#666' }}>
              本次报告没有该模块的逐帧函数采样数据。
            </div>
          )}
          {!selectedFrameLoading && supportsFrameFunctions && selectedFrameFunctions && selectedFrameFunctions.functions.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {selectedFrameFunctions.frame_index !== selectedFrameIndex && (
                <div style={{ fontSize: 12, color: '#ffcc80' }}>
                  选中帧 #{selectedFrameIndex} 没有匹配样本，已回退到最近的采样帧 #{selectedFrameFunctions.frame_index}。
                </div>
              )}
              <div style={{ overflow: 'auto', maxHeight: 320 }}>
              <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
                <thead>
                  <tr style={{ background: '#0f3460' }}>
                    <th style={thStyle}>函数名</th>
                    <th style={thStyle}>分类</th>
                    <th style={{ ...thStyle, textAlign: 'right' }}>Self (ms)</th>
                    <th style={{ ...thStyle, textAlign: 'right' }}>Total (ms)</th>
                    <th style={{ ...thStyle, textAlign: 'right' }}>Calls</th>
                    <th style={{ ...thStyle, textAlign: 'right' }}>Depth</th>
                  </tr>
                </thead>
                <tbody>
                  {selectedFrameFunctions.functions
                    .slice()
                    .sort((a, b) => b.self_ms - a.self_ms)
                    .map((fn, i) => (
                      <tr key={`${fn.name}-${i}`} style={{ background: i % 2 === 0 ? '#1a1a2e' : '#16213e' }}>
                        <td style={{ ...tdStyle, fontFamily: 'monospace', paddingLeft: 8 + fn.depth * 12 }}>{fn.name}</td>
                        <td style={tdStyle}>{fn.category}</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.self_ms.toFixed(3)}</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.total_ms.toFixed(3)}</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.call_count}</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.depth}</td>
                      </tr>
                    ))}
                </tbody>
              </table>
              </div>
            </div>
          )}
          {!selectedFrameLoading && supportsFrameFunctions && selectedFrameLoaded && (!selectedFrameFunctions || selectedFrameFunctions.functions.length === 0) && (
            <div style={{ fontSize: 12, color: '#666' }}>
              该帧在当前模块下没有函数采样样本。
            </div>
          )}
        </div>
      )}

      {/* AI 解读 */}
      <div style={{ background: '#16213e', borderRadius: 6, padding: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: aiResult ? 10 : 0 }}>
          <span style={{ fontSize: 13, color: '#b388ff', fontWeight: 600 }}>🤖 AI 解读</span>
          <button
            onClick={handleAiAnalysis}
            disabled={globalAiLoading || !settings.ai_cli}
            style={{
              padding: '4px 14px', fontSize: 12, borderRadius: 4, cursor: globalAiLoading ? 'wait' : 'pointer',
              background: globalAiLoading ? '#333' : '#7c4dff', color: '#fff', border: 'none',
              opacity: !settings.ai_cli ? 0.4 : 1,
            }}
          >
            {aiTaskRunning ? '分析中...' : '分析此模块'}
          </button>
        </div>
        {aiTaskRunning && aiLiveLog.length > 0 && (
          <div style={{ marginBottom: 10, maxHeight: 180, overflowY: 'auto', background: '#0d0d1a', border: '1px solid #2b2b44', borderRadius: 6, padding: 10 }}>
            {aiLiveLog.map((line, index) => (
              <div key={index} style={{ fontSize: 11, color: '#8db2d1', fontFamily: 'monospace', whiteSpace: 'pre-wrap', lineHeight: 1.5 }}>
                {line}
              </div>
            ))}
          </div>
        )}
        {aiResult && (
          <div style={{ fontSize: 12, color: '#ccc', whiteSpace: 'pre-wrap', lineHeight: 1.6, maxHeight: 400, overflowY: 'auto' }}>
            {aiResult}
          </div>
        )}
        {!aiResult && !aiTaskRunning && (
          <div style={{ fontSize: 11, color: '#666', marginTop: 4 }}>
            {settings.ai_cli ? '点击按钮获取 AI 对此模块的性能解读和优化建议' : '请先在设置中配置 AI CLI'}
          </div>
        )}
      </div>
    </div>
  );
};

const thStyle: React.CSSProperties = {
  padding: '6px 8px',
  textAlign: 'left',
  borderBottom: '1px solid #333',
  color: '#888',
  fontSize: 11,
  whiteSpace: 'nowrap',
};

const tdStyle: React.CSSProperties = {
  padding: '4px 8px',
  borderBottom: '1px solid #222',
  color: '#ccc',
};

export default ModulePage;
