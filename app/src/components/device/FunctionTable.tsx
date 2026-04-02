import React from 'react';
import type { ModuleFunctionStats } from '../../api/tauri';

interface FunctionTableProps {
  functions: ModuleFunctionStats[];
  title?: string;
  emptyMessage?: string;
}

export const FunctionTable: React.FC<FunctionTableProps> = ({ functions, title, emptyMessage = '无函数采样数据' }) => {
  return (
    <div style={{ overflow: 'auto', maxHeight: 400 }}>
      {title && <div style={{ fontSize: 13, fontWeight: 600, color: '#ccc', marginBottom: 6 }}>{title}</div>}
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
        <thead>
          <tr style={{ background: '#0f3460' }}>
            <th style={thStyle}>函数名</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Self(avg ms)</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Self%</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Total(avg ms)</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Total%</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Calls</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Calls/F</th>
            <th style={{ ...thStyle, textAlign: 'right' }}>Frames</th>
          </tr>
        </thead>
        <tbody>
          {functions.map((f, i) => (
            <tr key={i} style={{ background: i % 2 === 0 ? '#1a1a2e' : '#16213e' }}>
              <td style={{ ...tdStyle, maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f.name}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: f.avg_self_ms > 1 ? '#ffb74d' : '#ccc' }}>{f.avg_self_ms.toFixed(3)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#4fc3f7' }}>{f.self_pct.toFixed(1)}%</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: f.avg_total_ms > 2 ? '#e57373' : '#ccc' }}>{f.avg_total_ms.toFixed(3)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#81c784' }}>{f.total_pct.toFixed(1)}%</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.call_count}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.calls_per_frame.toFixed(1)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.frames_called}</td>
            </tr>
          ))}
          {functions.length === 0 && (
            <tr><td colSpan={8} style={{ textAlign: 'center', color: '#666', padding: 20 }}>{emptyMessage}</td></tr>
          )}
        </tbody>
      </table>
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
};

export default FunctionTable;
