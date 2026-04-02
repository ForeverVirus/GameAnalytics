import React, { useEffect, useState } from 'react';
import { api } from '../../../api/tauri';
import type { DeviceProfileReport, ResourceMemoryAnalysis } from '../../../api/tauri';
import FrameTimeline from '../FrameTimeline';
import MetricCards from '../MetricCards';

interface ReportMemoryProps {
  filePath: string;
  report: DeviceProfileReport;
}

type MemoryTab = 'overview' | 'mono' | 'gfx' | 'gc' | 'resource';

export const ReportMemory: React.FC<ReportMemoryProps> = ({ filePath, report }) => {
  const [tab, setTab] = useState<MemoryTab>('overview');
  const [resourceData, setResourceData] = useState<ResourceMemoryAnalysis | null>(null);
  const [loadingResource, setLoadingResource] = useState(false);

  const m = report.memory_analysis;

  useEffect(() => {
    if (tab === 'resource' && !resourceData) {
      setLoadingResource(true);
      api.getResourceMemoryAnalysis(filePath)
        .then(data => { setResourceData(data); setLoadingResource(false); })
        .catch(() => setLoadingResource(false));
    }
  }, [tab, filePath, resourceData]);

  const tabs: { key: MemoryTab; label: string }[] = [
    { key: 'overview', label: '总览' },
    { key: 'mono', label: 'Mono内存' },
    { key: 'gfx', label: 'GFX内存' },
    { key: 'gc', label: 'GC Alloc' },
    { key: 'resource', label: '资源内存' },
  ];

  const entries = [
    { label: '峰值总内存', value: `${m.peak_total_mb.toFixed(0)}MB`, severity: m.peak_total_mb > 2048 ? 'critical' : m.peak_total_mb > 1024 ? 'warning' : 'normal' },
    { label: '平均总内存', value: `${m.avg_total_mb.toFixed(0)}MB`, severity: 'normal' },
    { label: 'Mono峰值', value: `${m.peak_mono_mb.toFixed(0)}MB`, severity: m.peak_mono_mb > 256 ? 'critical' : m.peak_mono_mb > 128 ? 'warning' : 'normal' },
    { label: 'GFX峰值', value: `${m.peak_gfx_mb.toFixed(0)}MB`, severity: m.peak_gfx_mb > 512 ? 'critical' : m.peak_gfx_mb > 256 ? 'warning' : 'normal' },
    { label: 'GC总量', value: `${m.total_gc_alloc_mb.toFixed(1)}MB`, severity: m.total_gc_alloc_mb > 50 ? 'critical' : m.total_gc_alloc_mb > 10 ? 'warning' : 'normal' },
    { label: 'GC/帧', value: `${m.gc_alloc_per_frame_bytes.toFixed(0)}B`, severity: m.gc_alloc_per_frame_bytes > 1024 ? 'warning' : 'normal' },
    { label: '趋势', value: m.memory_trend, severity: m.memory_trend === 'stable' ? 'normal' : 'warning' },
    { label: '增长速率', value: `${m.memory_growth_rate_mb_per_min.toFixed(1)}MB/min`, severity: m.memory_growth_rate_mb_per_min > 1 ? 'critical' : m.memory_growth_rate_mb_per_min > 0.1 ? 'warning' : 'normal' },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>内存分析</h3>

      {/* Tabs */}
      <div style={{ display: 'flex', gap: 4 }}>
        {tabs.map(t => (
          <button key={t.key} onClick={() => setTab(t.key)}
            style={{ padding: '4px 14px', fontSize: 12, background: tab === t.key ? '#7c4dff' : '#1a1a2e', color: tab === t.key ? '#fff' : '#aaa', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'overview' && (
        <>
          <MetricCards entries={entries} />
          <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
            <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>内存时间线 (MB)</div>
            <FrameTimeline
              series={[{ name: '总内存', data: m.memory_timeline, color: '#7c4dff' }]}
              height={220} yLabel="MB"
            />
          </div>
        </>
      )}

      {tab === 'mono' && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>Mono 内存时间线 (MB)</div>
          <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 12 }}>
            峰值: {m.peak_mono_mb.toFixed(0)}MB
          </div>
          <FrameTimeline
            series={[{ name: 'Mono', data: m.memory_timeline, color: '#ff9800' }]}
            height={220} yLabel="MB"
          />
        </div>
      )}

      {tab === 'gfx' && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>GFX 内存时间线 (MB)</div>
          <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 12 }}>
            峰值: {m.peak_gfx_mb.toFixed(0)}MB
          </div>
          <FrameTimeline
            series={[{ name: 'GFX', data: m.memory_timeline, color: '#00bcd4' }]}
            height={220} yLabel="MB"
          />
        </div>
      )}

      {tab === 'gc' && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>GC Alloc (MB)</div>
          <div style={{ display: 'flex', gap: 16, marginBottom: 12, fontSize: 13, color: '#aaa' }}>
            <span>总GC: {m.total_gc_alloc_mb.toFixed(1)}MB</span>
            <span>每帧: {m.gc_alloc_per_frame_bytes.toFixed(0)}B</span>
          </div>
          <FrameTimeline
            series={[{ name: 'GC Alloc', data: m.memory_timeline, color: '#f44336' }]}
            height={220} yLabel="MB"
          />
        </div>
      )}

      {tab === 'resource' && (
        <div>
          {loadingResource ? (
            <div style={{ padding: 20, color: '#888', textAlign: 'center' }}>加载资源内存数据...</div>
          ) : resourceData ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              {/* Resource memory timelines */}
              <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
                <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>资源内存总览 (MB)</div>
                <FrameTimeline
                  series={[
                    { name: '总内存', data: resourceData.total_memory_timeline, color: '#7c4dff' },
                    { name: 'Mono', data: resourceData.mono_memory_timeline, color: '#ff9800' },
                    { name: 'GFX', data: resourceData.gfx_memory_timeline, color: '#00bcd4' },
                  ]}
                  height={220} yLabel="MB"
                />
              </div>

              {/* Per-resource type breakdown */}
              {resourceData.resource_types.map(rt => (
                <div key={rt.type_name} style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
                    <div style={{ fontSize: 13, fontWeight: 600, color: '#ccc' }}>{rt.type_name}</div>
                    <div style={{ fontSize: 12, color: '#888' }}>Peak: {rt.peak_mb.toFixed(1)}MB · Avg: {rt.avg_mb.toFixed(1)}MB</div>
                  </div>
                  {rt.timeline.length > 0 && (
                    <FrameTimeline
                      series={[{ name: rt.type_name, data: rt.timeline, color: '#4fc3f7' }]}
                      height={120} yLabel="MB"
                    />
                  )}
                  {rt.top_instances.length > 0 && (
                    <div style={{ marginTop: 8 }}>
                      <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>Top 资源实例</div>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                        {rt.top_instances.map((inst, i) => (
                          <div key={i} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, padding: '2px 4px', background: i % 2 === 0 ? '#1a1a2e' : 'transparent', borderRadius: 2 }}>
                            <span style={{ color: '#aaa', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '70%' }}>{inst.name}</span>
                            <span style={{ color: '#ccc' }}>{inst.size_label}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          ) : (
            <div style={{ padding: 20, color: '#888', textAlign: 'center' }}>无资源内存数据</div>
          )}
        </div>
      )}
    </div>
  );
};

export default ReportMemory;
