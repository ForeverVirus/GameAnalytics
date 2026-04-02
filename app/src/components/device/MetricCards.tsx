import React from 'react';
import type { MetricEntry } from '../../api/tauri';

interface MetricCardsProps {
  entries: MetricEntry[];
}

const severityColors: Record<string, string> = {
  normal: '#81c784',
  warning: '#ffb74d',
  critical: '#e57373',
};

export const MetricCards: React.FC<MetricCardsProps> = ({ entries }) => {
  return (
    <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
      {entries.map((e, i) => (
        <div key={i} style={{
          background: '#16213e',
          borderRadius: 6,
          padding: '10px 16px',
          minWidth: 120,
          border: `1px solid ${e.severity === 'critical' ? '#e57373' : e.severity === 'warning' ? '#ffb74d' : '#333'}`,
        }}>
          <div style={{ fontSize: 11, color: '#888', marginBottom: 4 }}>{e.label}</div>
          <div style={{ fontSize: 18, fontWeight: 700, color: severityColors[e.severity] || '#ccc' }}>
            {e.value}
          </div>
        </div>
      ))}
    </div>
  );
};

export default MetricCards;
