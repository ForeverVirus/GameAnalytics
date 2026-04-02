import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';
import FrameTimeline from '../FrameTimeline';
import MetricCards from '../MetricCards';

interface ReportTemperatureProps {
  report: DeviceProfileReport;
}

export const ReportTemperature: React.FC<ReportTemperatureProps> = ({ report }) => {
  const t = report.thermal_analysis;
  if (!t.has_data) {
    return <div style={{ padding: 40, textAlign: 'center', color: '#888' }}>无温度数据</div>;
  }

  const entries = [
    { label: '平均温度', value: `${t.avg_temperature.toFixed(1)}°C`, severity: t.avg_temperature > 42 ? 'critical' : t.avg_temperature > 38 ? 'warning' : 'normal' },
    { label: '最高温度', value: `${t.max_temperature.toFixed(1)}°C`, severity: t.max_temperature > 45 ? 'critical' : t.max_temperature > 40 ? 'warning' : 'normal' },
    { label: '降频风险', value: t.thermal_throttle_risk, severity: t.thermal_throttle_risk === 'low' ? 'normal' : t.thermal_throttle_risk === 'medium' ? 'warning' : 'critical' },
  ];

  const series = t.temperature_timeline.length > 0
    ? [{ name: '温度', data: t.temperature_timeline, color: '#ff5722' }]
    : [];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>温度变化量</h3>
      <MetricCards entries={entries} />
      {series.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>温度时间线 (°C)</div>
          <FrameTimeline series={series} height={220} yLabel="°C" threshold={40} thresholdLabel="降频警戒" />
        </div>
      )}
    </div>
  );
};

export default ReportTemperature;
