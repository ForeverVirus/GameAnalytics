import React from 'react';
import type { ModuleFunctionStats } from '../../api/tauri';
import ResizableTable from './ResizableTable';

interface FunctionTableProps {
  functions: ModuleFunctionStats[];
  title?: string;
  emptyMessage?: string;
}

export const FunctionTable: React.FC<FunctionTableProps> = ({ functions, title, emptyMessage = '无函数采样数据' }) => {
  const columns = [
    { key: 'name', label: '函数名', width: 420, minWidth: 220 },
    { key: 'avg_self_ms', label: 'Self(avg ms)', width: 110, minWidth: 90, align: 'right' as const },
    { key: 'self_pct', label: 'Self%', width: 90, minWidth: 70, align: 'right' as const },
    { key: 'avg_total_ms', label: 'Total(avg ms)', width: 120, minWidth: 100, align: 'right' as const },
    { key: 'total_pct', label: 'Total%', width: 90, minWidth: 70, align: 'right' as const },
    { key: 'call_count', label: 'Calls', width: 90, minWidth: 70, align: 'right' as const },
    { key: 'calls_per_frame', label: 'Calls/F', width: 90, minWidth: 70, align: 'right' as const },
    { key: 'frames_called', label: 'Frames', width: 90, minWidth: 70, align: 'right' as const },
  ];

  return (
    <div>
      {title && <div style={{ fontSize: 13, fontWeight: 600, color: '#ccc', marginBottom: 6 }}>{title}</div>}
      <ResizableTable columns={columns} rowCount={functions.length} maxHeight={400} emptyState={emptyMessage}>
          {functions.map((f, i) => (
            <tr key={i} style={{ background: i % 2 === 0 ? '#1a1a2e' : '#16213e' }}>
              <td style={{ ...tdStyle, fontFamily: 'monospace', whiteSpace: 'nowrap' }} title={f.name}>{f.name}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: f.avg_self_ms > 1 ? '#ffb74d' : '#ccc' }}>{f.avg_self_ms.toFixed(3)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#4fc3f7' }}>{f.self_pct.toFixed(1)}%</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: f.avg_total_ms > 2 ? '#e57373' : '#ccc' }}>{f.avg_total_ms.toFixed(3)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#81c784' }}>{f.total_pct.toFixed(1)}%</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.call_count}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.calls_per_frame.toFixed(1)}</td>
              <td style={{ ...tdStyle, textAlign: 'right', color: '#aaa' }}>{f.frames_called}</td>
            </tr>
          ))}
      </ResizableTable>
    </div>
  );
};

const tdStyle: React.CSSProperties = {
  padding: '4px 8px',
  borderBottom: '1px solid #222',
};

export default FunctionTable;
