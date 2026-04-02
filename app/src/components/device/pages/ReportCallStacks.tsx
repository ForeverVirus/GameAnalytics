import React, { useEffect, useState } from 'react';
import { api } from '../../../api/tauri';
import type { CallTreeNode, FunctionSearchResult } from '../../../api/tauri';
import CallTreeTable from '../CallTreeTable';
import IcicleChart from '../IcicleChart';

interface ReportCallStacksProps {
  filePath: string;
}

export const ReportCallStacks: React.FC<ReportCallStacksProps> = ({ filePath }) => {
  const [direction, setDirection] = useState<'forward' | 'reverse'>('forward');
  const [nodes, setNodes] = useState<CallTreeNode[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<FunctionSearchResult[]>([]);
  const [viewMode, setViewMode] = useState<'tree' | 'icicle'>('tree');

  useEffect(() => {
    setLoading(true);
    api.getCallTree(filePath, undefined, undefined, undefined, direction)
      .then(data => { setNodes(data); setLoading(false); })
      .catch(() => setLoading(false));
  }, [filePath, direction]);

  const handleSearch = () => {
    if (!searchQuery.trim()) { setSearchResults([]); return; }
    api.searchDeviceFunctions(filePath, searchQuery)
      .then(setSearchResults)
      .catch(() => setSearchResults([]));
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ margin: 0, color: '#e0e0e0', fontSize: 16 }}>CPU 调用堆栈</h3>
        <div style={{ display: 'flex', gap: 8 }}>
          <button onClick={() => setViewMode('tree')}
            style={{ padding: '4px 12px', fontSize: 12, background: viewMode === 'tree' ? '#7c4dff' : '#1a1a2e', color: viewMode === 'tree' ? '#fff' : '#aaa', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            表格
          </button>
          <button onClick={() => setViewMode('icicle')}
            style={{ padding: '4px 12px', fontSize: 12, background: viewMode === 'icicle' ? '#7c4dff' : '#1a1a2e', color: viewMode === 'icicle' ? '#fff' : '#aaa', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            火焰图
          </button>
        </div>
      </div>

      {/* Direction + search */}
      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <div style={{ display: 'flex', gap: 4 }}>
          <button onClick={() => setDirection('forward')}
            style={{ padding: '4px 12px', fontSize: 12, background: direction === 'forward' ? '#0f3460' : '#1a1a2e', color: direction === 'forward' ? '#4fc3f7' : '#888', border: '1px solid #333', borderRadius: 4, cursor: 'pointer' }}>
            正向分析 (Caller→Callee)
          </button>
          <button onClick={() => setDirection('reverse')}
            style={{ padding: '4px 12px', fontSize: 12, background: direction === 'reverse' ? '#0f3460' : '#1a1a2e', color: direction === 'reverse' ? '#4fc3f7' : '#888', border: '1px solid #333', borderRadius: 4, cursor: 'pointer' }}>
            反向分析 (Callee→Caller)
          </button>
        </div>
        <div style={{ flex: 1, display: 'flex', gap: 4 }}>
          <input type="text" value={searchQuery} onChange={e => setSearchQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleSearch()}
            placeholder="搜索函数名..."
            style={{ flex: 1, padding: '4px 10px', fontSize: 12, background: '#1a1a2e', color: '#ccc', border: '1px solid #333', borderRadius: 4, outline: 'none' }} />
          <button onClick={handleSearch}
            style={{ padding: '4px 12px', fontSize: 12, background: '#0f3460', color: '#4fc3f7', border: 'none', borderRadius: 4, cursor: 'pointer' }}>
            搜索
          </button>
        </div>
      </div>

      {/* Search results */}
      {searchResults.length > 0 && (
        <div style={{ background: '#16213e', borderRadius: 6, padding: 12, maxHeight: 200, overflow: 'auto' }}>
          <div style={{ fontSize: 12, color: '#888', marginBottom: 8 }}>搜索结果 ({searchResults.length})</div>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
            <thead>
              <tr style={{ color: '#666' }}>
                <th style={{ textAlign: 'left', padding: '2px 6px' }}>函数名</th>
                <th style={{ textAlign: 'left', padding: '2px 6px' }}>分类</th>
                <th style={{ textAlign: 'right', padding: '2px 6px' }}>Avg Self</th>
                <th style={{ textAlign: 'right', padding: '2px 6px' }}>Avg Total</th>
                <th style={{ textAlign: 'right', padding: '2px 6px' }}>Calls/F</th>
                <th style={{ textAlign: 'right', padding: '2px 6px' }}>Frames</th>
              </tr>
            </thead>
            <tbody>
              {searchResults.map((r, i) => (
                <tr key={i} style={{ borderTop: '1px solid #222' }}>
                  <td style={{ padding: '3px 6px', color: '#ccc', fontFamily: 'monospace', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{r.name}</td>
                  <td style={{ padding: '3px 6px', color: '#888' }}>{r.category}</td>
                  <td style={{ padding: '3px 6px', textAlign: 'right', color: '#aaa' }}>{r.avg_self_ms.toFixed(3)}</td>
                  <td style={{ padding: '3px 6px', textAlign: 'right', color: '#aaa' }}>{r.avg_total_ms.toFixed(3)}</td>
                  <td style={{ padding: '3px 6px', textAlign: 'right', color: '#888' }}>{r.call_count}</td>
                  <td style={{ padding: '3px 6px', textAlign: 'right', color: '#888' }}>{r.frames_called}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Main content */}
      {loading ? (
        <div style={{ padding: 20, color: '#888', textAlign: 'center' }}>加载调用树...</div>
      ) : viewMode === 'tree' ? (
        <CallTreeTable nodes={nodes} title={direction === 'forward' ? '正向调用树' : '反向调用树'} searchQuery={searchQuery} />
      ) : (
        <div style={{ background: '#16213e', borderRadius: 8, padding: 12 }}>
          <IcicleChart nodes={nodes} height={400} />
        </div>
      )}
    </div>
  );
};

export default ReportCallStacks;
