import React, { useEffect, useState, useCallback } from 'react';
import { api } from '../../../api/tauri';
import type { ReportMeta } from '../../../api/tauri';
import { formatBeijingDateTime } from '../../../utils/time';
import { clearCachedReportHistory, getCachedReportHistory, setCachedReportHistory } from '../../../utils/deviceReportCache';
import { useAppStore } from '../../../store';

interface ReportHistoryProps {
  onLoadReport: (report: ReportMeta) => void;
}

export const ReportHistory: React.FC<ReportHistoryProps> = ({ onLoadReport }) => {
  const [reports, setReports] = useState<ReportMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const projectPath = useAppStore(s => s.project?.path);

  const refresh = useCallback((force = false) => {
    const cached = getCachedReportHistory(projectPath);
    if (!force && cached) {
      setReports(cached);
      setLoading(false);
      return;
    }

    setLoading(true);
    api.listDeviceReports()
      .then(data => {
        setCachedReportHistory(projectPath, data);
        setReports(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [projectPath]);

  useEffect(() => { refresh(); }, [refresh]);

  const handleDelete = async (id: string) => {
    try {
      await api.deleteDeviceReport(id);
      clearCachedReportHistory(projectPath);
      refresh(true);
    } catch {
      // ignore
    }
  };

  if (loading) return <div style={{ padding: 20, color: '#888' }}>加载历史报告...</div>;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>历史报告</h3>
        <button onClick={() => refresh(true)}
          style={{ padding: '4px 12px', fontSize: 12, background: '#1a1a2e', color: '#4fc3f7', border: '1px solid #333', borderRadius: 4, cursor: 'pointer' }}>
          ↻ 刷新
        </button>
      </div>

      {reports.length === 0 ? (
        <div style={{ padding: 40, textAlign: 'center', color: '#888' }}>暂无保存的报告</div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {reports.map(r => (
            <div key={r.id} style={{ background: '#16213e', borderRadius: 8, padding: 12, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 13, fontWeight: 600, color: '#ccc' }}>{r.session_name}</div>
                <div style={{ fontSize: 11, color: '#888', marginTop: 2 }}>
                  {r.device_model} · {formatBeijingDateTime(r.timestamp)} · {r.total_frames} 帧 · {r.duration_seconds.toFixed(1)}s
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <span style={{ fontSize: 16, fontWeight: 'bold', color: r.overall_grade === 'S' || r.overall_grade === 'A' ? '#81c784' : r.overall_grade === 'B' ? '#4fc3f7' : '#ffb74d' }}>
                  {r.overall_grade}
                </span>
                <button onClick={() => onLoadReport(r)}
                  style={{ padding: '4px 10px', fontSize: 11, background: '#0f3460', color: '#4fc3f7', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
                  加载
                </button>
                <button onClick={() => void handleDelete(r.id)}
                  style={{ padding: '4px 10px', fontSize: 11, background: '#3a1111', color: '#e57373', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
                  删除
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default ReportHistory;
