import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';

interface ReportRuntimeInfoProps {
  report: DeviceProfileReport;
}

export const ReportRuntimeInfo: React.FC<ReportRuntimeInfoProps> = ({ report }) => {
  const d = report.device_info;
  const rows: [string, string][] = [
    ['设备型号', d.device_model],
    ['操作系统', d.operating_system],
    ['CPU', d.processor_type],
    ['CPU 核心数', `${d.processor_count}`],
    ['CPU 频率', `${d.processor_frequency} MHz`],
    ['系统内存', `${d.system_memory_mb} MB`],
    ['GPU', d.graphics_device_name],
    ['显存', `${d.graphics_memory_mb} MB`],
    ['图形API', d.graphics_device_name],
    ['分辨率', `${d.screen_width}×${d.screen_height}`],
    ['Unity 版本', d.unity_version],
    ['平台', d.platform],
    ['应用', d.app_version],
    ['会话名称', report.session_name],
    ['时长', `${report.duration_seconds.toFixed(1)}s`],
    ['总帧数', `${report.total_frames}`],
    ['截图数', `${report.screenshot_count}`],
    ['评级', report.overall_grade],
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>运行信息</h3>
      <div style={{ background: '#16213e', borderRadius: 8, overflow: 'hidden' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <tbody>
            {rows.map(([label, value]) => (
              <tr key={label} style={{ borderBottom: '1px solid #1a1a2e' }}>
                <td style={{ padding: '8px 16px', color: '#888', width: 150 }}>{label}</td>
                <td style={{ padding: '8px 16px', color: '#e0e0e0' }}>{value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {report.scene_breakdown.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 12 }}>场景分布</div>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
            <thead>
              <tr style={{ color: '#888', borderBottom: '1px solid #333' }}>
                <th style={{ textAlign: 'left', padding: '4px 8px' }}>场景</th>
                <th style={{ textAlign: 'right', padding: '4px 8px' }}>帧数</th>
                <th style={{ textAlign: 'right', padding: '4px 8px' }}>Avg FPS</th>
                <th style={{ textAlign: 'right', padding: '4px 8px' }}>Avg 内存</th>
                <th style={{ textAlign: 'right', padding: '4px 8px' }}>卡顿</th>
              </tr>
            </thead>
            <tbody>
              {report.scene_breakdown.map(s => (
                <tr key={s.scene_name} style={{ borderBottom: '1px solid #222' }}>
                  <td style={{ padding: '6px 8px', color: '#ccc' }}>{s.scene_name}</td>
                  <td style={{ padding: '6px 8px', textAlign: 'right', color: '#aaa' }}>{s.frame_count}</td>
                  <td style={{ padding: '6px 8px', textAlign: 'right', color: s.avg_fps >= 55 ? '#81c784' : '#ffb74d' }}>{s.avg_fps.toFixed(1)}</td>
                  <td style={{ padding: '6px 8px', textAlign: 'right', color: '#aaa' }}>{s.avg_memory_mb.toFixed(0)} MB</td>
                  <td style={{ padding: '6px 8px', textAlign: 'right', color: s.jank_count > 0 ? '#ffb74d' : '#888' }}>{s.jank_count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default ReportRuntimeInfo;
