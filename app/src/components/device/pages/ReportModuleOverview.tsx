import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';
import FrameTimeline from '../FrameTimeline';

interface ReportModuleOverviewProps {
  report: DeviceProfileReport;
  onNavigate: (page: string) => void;
}

const modulePageMap: Record<string, string> = {
  '渲染模块': 'module_rendering',
  'GPU同步': 'module_gpu_sync',
  '逻辑代码': 'module_scripting',
  'UI模块': 'module_ui',
  '加载模块': 'module_loading',
  '物理系统': 'module_physics',
  '动画模块': 'module_animation',
  '粒子系统': 'module_particles',
};

export const ReportModuleOverview: React.FC<ReportModuleOverviewProps> = ({ report, onNavigate }) => {
  const modules = report.module_analysis.module_breakdown;
  const maxMs = Math.max(...modules.map(m => m.avg_ms), 0.01);

  const cpuTimeline = report.fps_analysis.fps_timeline.map(p => ({ time: p.time, value: 1000 / Math.max(p.value, 0.1) }));
  const series = [{ name: 'CPU Frame Time', data: cpuTimeline, color: '#4fc3f7' }];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>模块耗时统计</h3>

      {/* CPU frame time chart */}
      <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>CPU Frame Time (ms)</div>
        <FrameTimeline series={series} height={180} yLabel="ms" threshold={16.67} thresholdLabel="60fps" />
      </div>

      {/* Module breakdown */}
      <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 13, color: '#888', marginBottom: 12 }}>
          瓶颈: <span style={{ color: '#ff9800', fontWeight: 600 }}>{report.module_analysis.bottleneck}</span>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {modules.map(m => (
            <div key={m.name} onClick={() => { const page = modulePageMap[m.name]; if (page) onNavigate(page); }}
              style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: modulePageMap[m.name] ? 'pointer' : 'default', padding: '4px 0' }}>
              <span style={{ width: 80, fontSize: 12, color: '#aaa', textAlign: 'right' }}>{m.name}</span>
              <div style={{ flex: 1, background: '#1a1a2e', borderRadius: 4, height: 22, position: 'relative' }}>
                <div style={{ width: `${(m.avg_ms / maxMs) * 100}%`, height: '100%', background: 'linear-gradient(90deg, #00e0ff, #7c4dff)', borderRadius: 4, minWidth: 2 }} />
              </div>
              <span style={{ width: 70, fontSize: 12, color: '#ccc', textAlign: 'right' }}>{m.avg_ms.toFixed(2)}ms</span>
              <span style={{ width: 50, fontSize: 11, color: '#888', textAlign: 'right' }}>max {m.max_ms.toFixed(1)}</span>
              <span style={{ width: 40, fontSize: 11, color: '#666' }}>{m.percentage.toFixed(0)}%</span>
            </div>
          ))}
        </div>
      </div>

      {/* Stacked summary stats */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))', gap: 8 }}>
        <SummaryCard label="渲染" value={report.module_analysis.avg_render_ms} />
        <SummaryCard label="脚本" value={report.module_analysis.avg_scripts_ms} />
        <SummaryCard label="物理" value={report.module_analysis.avg_physics_ms} />
        <SummaryCard label="动画" value={report.module_analysis.avg_animation_ms} />
        <SummaryCard label="UI" value={report.module_analysis.avg_ui_ms} />
        <SummaryCard label="粒子" value={report.module_analysis.avg_particle_ms} />
        <SummaryCard label="加载" value={report.module_analysis.avg_loading_ms} />
        <SummaryCard label="GC" value={report.module_analysis.avg_gc_ms} />
      </div>
    </div>
  );
};

function SummaryCard({ label, value }: { label: string; value: number }) {
  return (
    <div style={{ background: '#16213e', borderRadius: 6, padding: '8px 12px', textAlign: 'center' }}>
      <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 18, fontWeight: 600, color: value > 5 ? '#ffb74d' : '#81c784' }}>{value.toFixed(2)}ms</div>
    </div>
  );
}

export default ReportModuleOverview;
