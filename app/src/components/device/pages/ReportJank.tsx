import React, { useEffect, useMemo, useState } from 'react';
import { api } from '../../../api/tauri';
import type { DeviceProfileReport, PerFrameFunctions, TimelinePoint } from '../../../api/tauri';
import FrameTimeline from '../FrameTimeline';
import MetricCards from '../MetricCards';
import ResizableTable from '../ResizableTable';
import { getCachedFrameFunctions, setCachedFrameFunctions } from '../../../utils/deviceReportCache';

interface ReportJankProps {
  filePath: string;
  report: DeviceProfileReport;
}

export const ReportJank: React.FC<ReportJankProps> = ({ filePath, report }) => {
  const j = report.jank_analysis;
  const [selectedJankPoint, setSelectedJankPoint] = useState<TimelinePoint | null>(null);
  const [frameFunctions, setFrameFunctions] = useState<PerFrameFunctions | null>(null);
  const [loadingFrame, setLoadingFrame] = useState(false);

  const jankPoints = useMemo(() => j.jank_timeline.filter(p => p.value > 33), [j.jank_timeline]);

  const entries = [
    { label: '卡顿帧', value: `${j.total_jank_frames}`, severity: j.total_jank_frames === 0 ? 'normal' : j.total_jank_frames < 10 ? 'warning' : 'critical' },
    { label: '卡顿率', value: `${j.jank_rate_pct.toFixed(1)}%`, severity: j.jank_rate_pct < 1 ? 'normal' : j.jank_rate_pct < 5 ? 'warning' : 'critical' },
    { label: '严重卡顿', value: `${j.severe_jank_frames}`, severity: j.severe_jank_frames === 0 ? 'normal' : 'critical' },
    { label: '严重卡顿率', value: `${j.severe_jank_rate_pct.toFixed(1)}%`, severity: j.severe_jank_rate_pct < 0.5 ? 'normal' : 'critical' },
    { label: '最差帧', value: `${j.worst_frame_ms.toFixed(1)}ms`, severity: j.worst_frame_ms < 50 ? 'warning' : 'critical' },
    { label: '最差帧号', value: `#${j.worst_frame_index}`, severity: 'normal' },
  ];

  const series = j.jank_timeline.length > 0 ? [{ name: '帧耗时', data: j.jank_timeline, color: '#e57373' }] : [];

  useEffect(() => {
    const worst = jankPoints.find(p => p.frame_index === j.worst_frame_index) || jankPoints[0] || null;
    setSelectedJankPoint(worst);
  }, [j.worst_frame_index, jankPoints]);

  useEffect(() => {
    if (!filePath || selectedJankPoint?.frame_index === undefined) {
      setFrameFunctions(null);
      return;
    }

    const frameIndex = selectedJankPoint.frame_index;
    const cached = getCachedFrameFunctions(filePath, frameIndex, undefined, true);
    if (cached) {
      setFrameFunctions(cached);
      setLoadingFrame(false);
      return;
    }

    let cancelled = false;
    setLoadingFrame(true);
    api.getFrameFunctions(filePath, frameIndex, undefined, true)
      .then(data => {
        if (!cancelled) {
          if (data) {
            setCachedFrameFunctions(filePath, frameIndex, data, undefined, true);
          }
          setFrameFunctions(data);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setFrameFunctions(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingFrame(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [filePath, selectedJankPoint]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>卡顿分析</h3>

      <MetricCards entries={entries} />

      {series.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>卡顿时间线 (ms)</div>
          <FrameTimeline
            series={series}
            height={220}
            yLabel="ms"
            threshold={33.33}
            thresholdLabel="30fps"
            selectedTime={selectedJankPoint?.time ?? null}
            onFrameClick={(point) => setSelectedJankPoint(point)}
          />
        </div>
      )}

      {/* Jank frame list */}
      {jankPoints.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>卡顿帧列表 (耗时 &gt; 33ms)</div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4, maxHeight: 300, overflow: 'auto' }}>
            {jankPoints.map((p, i) => (
              <button key={i} onClick={() => setSelectedJankPoint(p)} style={{
                padding: '3px 8px', fontSize: 11, borderRadius: 4,
                background: p.value > 100 ? '#4a1111' : p.value > 50 ? '#3a1a00' : '#1a2a1a',
                color: p.value > 100 ? '#f44' : p.value > 50 ? '#fa0' : '#aaa',
                border: `1px solid ${p.value > 100 ? '#f44' : p.value > 50 ? '#fa0' : '#333'}33`,
                cursor: 'pointer',
              }}>
                #{p.frame_index ?? i}: {p.value.toFixed(1)}ms
              </button>
            ))}
          </div>
        </div>
      )}

      {selectedJankPoint && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>
            卡顿帧详情 #{selectedJankPoint.frame_index ?? 'N/A'} ({selectedJankPoint.value.toFixed(1)}ms)
          </div>
          {!filePath && (
            <div style={{ fontSize: 12, color: '#666' }}>当前历史报告没有源 `.gaprof` 路径，无法加载该帧函数详情。</div>
          )}
          {filePath && loadingFrame && <div style={{ fontSize: 12, color: '#888' }}>加载中...</div>}
          {filePath && !loadingFrame && frameFunctions && frameFunctions.functions.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {frameFunctions.frame_index !== selectedJankPoint.frame_index && (
                <div style={{ fontSize: 12, color: '#ffcc80' }}>
                  该卡顿帧没有精确函数样本，已回退到最近采样帧 #{frameFunctions.frame_index}。
                </div>
              )}
              <ResizableTable
                columns={[
                  { key: 'name', label: '函数名', width: 460, minWidth: 260 },
                  { key: 'category', label: '分类', width: 120, minWidth: 80 },
                  { key: 'self', label: 'Self', width: 100, minWidth: 80, align: 'right' },
                  { key: 'total', label: 'Total', width: 100, minWidth: 80, align: 'right' },
                  { key: 'calls', label: 'Calls', width: 90, minWidth: 70, align: 'right' },
                ]}
                rowCount={frameFunctions.functions.length}
                maxHeight={320}
              >
                  {frameFunctions.functions
                    .slice()
                    .sort((a, b) => b.self_ms - a.self_ms)
                    .slice(0, 30)
                    .map((fn, index) => (
                      <tr key={`${fn.name}-${index}`} style={{ background: index % 2 === 0 ? '#1a1a2e' : '#16213e' }}>
                        <td style={{ ...tdStyle, paddingLeft: 8 + fn.depth * 12, fontFamily: 'monospace', whiteSpace: 'nowrap' }} title={fn.name}>{fn.name}</td>
                        <td style={tdStyle}>{fn.category}</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.self_ms.toFixed(3)}ms</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.total_ms.toFixed(3)}ms</td>
                        <td style={{ ...tdStyle, textAlign: 'right' }}>{fn.call_count}</td>
                      </tr>
                    ))}
              </ResizableTable>
            </div>
          )}
          {filePath && !loadingFrame && (!frameFunctions || frameFunctions.functions.length === 0) && (
            <div style={{ fontSize: 12, color: '#666' }}>该卡顿点附近没有可用的函数采样数据。</div>
          )}
        </div>
      )}
    </div>
  );
};

const tdStyle: React.CSSProperties = {
  padding: '4px 8px',
  borderBottom: '1px solid #222',
  color: '#ccc',
};

export default ReportJank;
