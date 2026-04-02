import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';
import MetricCards from '../MetricCards';

interface ReportOverviewProps {
  report: DeviceProfileReport;
  onNavigate: (page: string) => void;
}

function GradeBadge({ grade }: { grade: string }) {
  const colorMap: Record<string, string> = { SSS: '#ff0', SS: '#ff0', S: '#0f0', A: '#0f0', B: '#0cf', C: '#fa0', D: '#f44' };
  const color = colorMap[grade] || '#888';
  return (
    <span style={{ display: 'inline-block', padding: '6px 24px', borderRadius: 8, fontWeight: 'bold', fontSize: 36, background: `${color}22`, color, border: `2px solid ${color}` }}>
      {grade}
    </span>
  );
}

export const ReportOverview: React.FC<ReportOverviewProps> = ({ report, onNavigate }) => {
  const s = report.summary;
  const entries = [
    { label: '平均 FPS', value: s.avg_fps.toFixed(1), severity: s.avg_fps >= 55 ? 'normal' : s.avg_fps >= 30 ? 'warning' : 'critical' },
    { label: 'P5 FPS', value: s.p5_fps.toFixed(0), severity: s.p5_fps >= 30 ? 'normal' : 'critical' },
    { label: '平均 CPU', value: `${s.avg_cpu_ms.toFixed(1)}ms`, severity: s.avg_cpu_ms < 16 ? 'normal' : s.avg_cpu_ms < 33 ? 'warning' : 'critical' },
    { label: '平均 GPU', value: `${s.avg_gpu_ms.toFixed(1)}ms`, severity: s.avg_gpu_ms < 16 ? 'normal' : s.avg_gpu_ms < 33 ? 'warning' : 'critical' },
    { label: '峰值内存', value: `${s.peak_memory_mb.toFixed(0)}MB`, severity: s.peak_memory_mb < 1024 ? 'normal' : s.peak_memory_mb < 2048 ? 'warning' : 'critical' },
    { label: 'GC Alloc', value: `${s.total_gc_alloc_mb.toFixed(1)}MB`, severity: s.total_gc_alloc_mb < 10 ? 'normal' : 'warning' },
    { label: '卡顿帧', value: `${s.jank_count}`, severity: s.jank_count === 0 ? 'normal' : s.jank_count < 10 ? 'warning' : 'critical' },
    { label: '稳定性', value: `${(s.fps_stability * 100).toFixed(0)}%`, severity: s.fps_stability > 0.9 ? 'normal' : s.fps_stability > 0.7 ? 'warning' : 'critical' },
  ];

  const moduleItems = report.module_analysis.module_breakdown;
  const maxModuleMs = Math.max(...moduleItems.map(m => m.avg_ms), 0.01);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {/* Grade + summary */}
      <div style={{ display: 'flex', gap: 24, alignItems: 'center', background: '#16213e', borderRadius: 8, padding: 20 }}>
        <GradeBadge grade={report.overall_grade} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 18, fontWeight: 600, color: '#e0e0e0' }}>{report.session_name}</div>
          <div style={{ fontSize: 12, color: '#888', marginTop: 4 }}>
            {report.device_info.device_model} · {report.device_info.operating_system}
          </div>
          <div style={{ fontSize: 12, color: '#666', marginTop: 2 }}>
            {report.total_frames} 帧 · {report.duration_seconds.toFixed(1)}s · {report.device_info.unity_version}
          </div>
        </div>
      </div>

      {/* Key metrics */}
      <MetricCards entries={entries} />

      {/* Module breakdown bar chart */}
      <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 12 }}>模块耗时分布</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {moduleItems.map(m => (
            <div key={m.name} style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}
              onClick={() => {
                const moduleMap: Record<string, string> = {
                  '渲染模块': 'module_rendering', 'GPU同步': 'module_gpu_sync', '逻辑代码': 'module_scripting',
                  'UI模块': 'module_ui', '加载模块': 'module_loading', '物理系统': 'module_physics',
                  '动画模块': 'module_animation', '粒子系统': 'module_particles',
                };
                const page = moduleMap[m.name];
                if (page) onNavigate(page);
              }}>
              <span style={{ width: 80, fontSize: 12, color: '#aaa', textAlign: 'right' }}>{m.name}</span>
              <div style={{ flex: 1, background: '#1a1a2e', borderRadius: 4, height: 18 }}>
                <div style={{ width: `${(m.avg_ms / maxModuleMs) * 100}%`, height: '100%', background: 'linear-gradient(90deg, #00e0ff, #7c4dff)', borderRadius: 4, minWidth: 2 }} />
              </div>
              <span style={{ width: 65, fontSize: 12, color: '#ccc', textAlign: 'right' }}>{m.avg_ms.toFixed(2)}ms</span>
              <span style={{ width: 40, fontSize: 11, color: '#666' }}>{m.percentage.toFixed(0)}%</span>
            </div>
          ))}
        </div>
        <div style={{ marginTop: 8, fontSize: 12, color: '#888' }}>
          瓶颈: <span style={{ color: '#ff9800', fontWeight: 600 }}>{report.module_analysis.bottleneck}</span>
        </div>
      </div>

      {/* FPS distribution mini chart */}
      <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 12 }}>FPS 分布</div>
        <div style={{ display: 'flex', alignItems: 'flex-end', gap: 4, height: 80 }}>
          {report.fps_analysis.fps_histogram.map(b => {
            const maxPct = Math.max(...report.fps_analysis.fps_histogram.map(x => x.percentage), 1);
            return (
              <div key={b.label} style={{ flex: 1, textAlign: 'center' }}>
                <div style={{ height: `${(b.percentage / maxPct) * 60}px`, background: '#00e0ff', borderRadius: '4px 4px 0 0', minHeight: 2 }} />
                <div style={{ fontSize: 9, color: '#888', marginTop: 2 }}>{b.label}</div>
                <div style={{ fontSize: 9, color: '#aaa' }}>{b.percentage.toFixed(0)}%</div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Quick links */}
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {[
          { label: '📊 CPU调用堆栈', page: 'call_stacks' },
          { label: '🧠 内存分析', page: 'memory' },
          { label: '⚡ 卡顿分析', page: 'jank' },
          { label: '🎨 渲染模块', page: 'module_rendering' },
          { label: '📋 运行日志', page: 'logs' },
        ].map(link => (
          <button key={link.page} onClick={() => onNavigate(link.page)}
            style={{ padding: '6px 14px', fontSize: 12, background: '#1a1a2e', color: '#4fc3f7', border: '1px solid #333', borderRadius: 6, cursor: 'pointer' }}>
            {link.label}
          </button>
        ))}
      </div>
    </div>
  );
};

export default ReportOverview;
