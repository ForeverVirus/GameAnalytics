import React from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';
import FrameTimeline from '../FrameTimeline';
import MetricCards from '../MetricCards';

interface ReportBatteryProps {
  report: DeviceProfileReport;
}

export const ReportBattery: React.FC<ReportBatteryProps> = ({ report }) => {
  const t = report.thermal_analysis;
  if (!t.has_data) {
    return <div style={{ padding: 40, textAlign: 'center', color: '#888' }}>无耗电量数据</div>;
  }

  const entries = [
    { label: '电量消耗', value: `${t.battery_drain.toFixed(1)}%`, severity: t.battery_drain > 20 ? 'critical' : t.battery_drain > 10 ? 'warning' : 'normal' },
    { label: '降频风险', value: t.thermal_throttle_risk, severity: t.thermal_throttle_risk === 'low' ? 'normal' : t.thermal_throttle_risk === 'medium' ? 'warning' : 'critical' },
  ];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>耗电量分析</h3>
      <MetricCards entries={entries} />
      <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
        <div style={{ fontSize: 13, color: '#888' }}>
          电量消耗: {t.battery_drain.toFixed(1)}% · 平均温度: {t.avg_temperature.toFixed(1)}°C · 降频风险: {t.thermal_throttle_risk}
        </div>
      </div>
    </div>
  );
};

export default ReportBattery;
