import React, { useState } from 'react';
import type { DeviceProfileReport } from '../../../api/tauri';

interface ReportLogsProps {
  report: DeviceProfileReport;
}

export const ReportLogs: React.FC<ReportLogsProps> = ({ report }) => {
  const log = report.log_analysis;
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filter, setFilter] = useState<'all' | 'errors' | 'warnings'>('all');

  if (!log?.has_data) {
    return <div style={{ padding: 40, textAlign: 'center', color: '#888' }}>无运行日志数据</div>;
  }

  const infoCount = log.info_count ?? 0;
  const infoEntries = log.top_info ?? [];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>运行日志</h3>

      {/* Summary */}
      <div style={{ display: 'flex', gap: 16, fontSize: 13, color: '#aaa' }}>
        <span>Total: <b>{log.total_logs}</b></span>
        <span style={{ color: '#4fc3f7' }}>Logs: <b>{infoCount}</b></span>
        <span style={{ color: '#f44' }}>Errors: <b>{log.error_count}</b></span>
        <span style={{ color: '#fa0' }}>Warnings: <b>{log.warning_count}</b></span>
        <span style={{ color: '#f44' }}>Exceptions: <b>{log.exception_count}</b></span>
      </div>

      {/* Filter */}
      <div style={{ display: 'flex', gap: 4 }}>
        {(['all', 'errors', 'warnings'] as const).map(f => (
          <button key={f} onClick={() => setFilter(f)}
            style={{ padding: '4px 12px', fontSize: 12, background: filter === f ? '#7c4dff' : '#1a1a2e', color: filter === f ? '#fff' : '#aaa', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            {f === 'all' ? '全部' : f === 'errors' ? '错误' : '警告'}
          </button>
        ))}
      </div>

      {(filter === 'all') && infoEntries.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: '#4fc3f7', marginBottom: 8 }}>普通日志 ({infoCount})</div>
          {infoEntries.map((entry, i) => (
            <div key={i} style={{ background: '#0c1a28', borderRadius: 6, padding: '6px 12px', marginBottom: 4, border: '1px solid #12324a' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontFamily: 'monospace', fontSize: 11, color: '#90caf9', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{entry.message}</span>
                <span style={{ fontSize: 11, color: '#4fc3f7', fontWeight: 'bold', marginLeft: 8 }}>×{entry.count}</span>
                <span style={{ fontSize: 10, color: '#888', marginLeft: 8 }}>frame #{entry.first_frame}</span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Errors */}
      {(filter === 'all' || filter === 'errors') && log.top_errors.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: '#f44', marginBottom: 8 }}>错误 ({log.error_count})</div>
          {log.top_errors.map((e, i) => (
            <div key={i} style={{ background: '#1a0a0a', borderRadius: 6, padding: '6px 12px', marginBottom: 4, cursor: 'pointer', border: '1px solid #331111' }}
              onClick={() => setExpanded(expanded === `err-${i}` ? null : `err-${i}`)}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontFamily: 'monospace', fontSize: 11, color: '#f88', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{e.message}</span>
                <span style={{ fontSize: 11, color: '#f44', fontWeight: 'bold', marginLeft: 8 }}>×{e.count}</span>
                <span style={{ fontSize: 10, color: '#888', marginLeft: 8 }}>frame #{e.first_frame}</span>
              </div>
              {expanded === `err-${i}` && (
                <div style={{ marginTop: 4, fontSize: 11, color: '#aaa', whiteSpace: 'pre-wrap', maxHeight: 200, overflow: 'auto' }}>{e.message}</div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Warnings */}
      {(filter === 'all' || filter === 'warnings') && log.top_warnings.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: '#fa0', marginBottom: 8 }}>警告 ({log.warning_count})</div>
          {log.top_warnings.map((w, i) => (
            <div key={i} style={{ background: '#1a1300', borderRadius: 6, padding: '6px 12px', marginBottom: 4, border: '1px solid #332200' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontFamily: 'monospace', fontSize: 11, color: '#fa0', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{w.message}</span>
                <span style={{ fontSize: 11, color: '#fa0', fontWeight: 'bold', marginLeft: 8 }}>×{w.count}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default ReportLogs;
