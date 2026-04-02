import React, { useState, useMemo } from 'react';
import type { CallTreeNode } from '../../api/tauri';

interface CallTreeTableProps {
  nodes: CallTreeNode[];
  title?: string;
  /** Search query to highlight */
  searchQuery?: string;
}

const CategoryColors: Record<string, string> = {
  '渲染模块': '#4fc3f7',
  '用户脚本': '#81c784',
  '物理模块': '#ffb74d',
  '动画模块': '#e57373',
  'UI模块': '#ba68c8',
  '加载模块': '#4dd0e1',
  '粒子模块': '#fff176',
  '同步等待': '#90a4ae',
  '引擎开销': '#a1887f',
  'GC': '#ef5350',
  '其他': '#666',
  '自定义模块': '#7e57c2',
};

const CallTreeRow: React.FC<{
  node: CallTreeNode;
  depth: number;
  expanded: Set<string>;
  toggleExpand: (key: string) => void;
  searchQuery?: string;
}> = ({ node, depth, expanded, toggleExpand, searchQuery }) => {
  const key = `${node.name}-${depth}-${node.avg_self_ms.toFixed(3)}`;
  const isExpanded = expanded.has(key);
  const hasChildren = node.children.length > 0;

  const highlight = searchQuery && node.name.toLowerCase().includes(searchQuery.toLowerCase());

  return (
    <>
      <tr style={{
        background: highlight ? 'rgba(79, 195, 247, 0.15)' : depth % 2 === 0 ? '#1a1a2e' : '#16213e',
        fontSize: '12px',
      }}>
        <td style={{ paddingLeft: `${depth * 20 + 8}px`, whiteSpace: 'nowrap', maxWidth: 400, overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {hasChildren && (
            <span
              onClick={() => toggleExpand(key)}
              style={{ cursor: 'pointer', marginRight: 4, userSelect: 'none', display: 'inline-block', width: 12 }}
            >
              {isExpanded ? '▼' : '▶'}
            </span>
          )}
          {!hasChildren && <span style={{ display: 'inline-block', width: 16 }} />}
          <span style={{ color: CategoryColors[node.category] || '#ccc' }}>{node.name}</span>
        </td>
        <td style={{ textAlign: 'right', color: '#aaa', minWidth: 60 }}>{node.category}</td>
        <td style={{ textAlign: 'right', color: node.avg_self_ms > 1 ? '#ffb74d' : '#ccc', minWidth: 70 }}>{node.avg_self_ms.toFixed(3)}</td>
        <td style={{ textAlign: 'right', color: '#ccc', minWidth: 70 }}>{node.total_self_ms.toFixed(1)}</td>
        <td style={{ textAlign: 'right', minWidth: 50 }}>
          <span style={{ color: '#4fc3f7' }}>{node.self_pct.toFixed(1)}%</span>
        </td>
        <td style={{ textAlign: 'right', color: node.avg_total_ms > 2 ? '#e57373' : '#ccc', minWidth: 70 }}>{node.avg_total_ms.toFixed(3)}</td>
        <td style={{ textAlign: 'right', color: '#ccc', minWidth: 70 }}>{node.total_total_ms.toFixed(1)}</td>
        <td style={{ textAlign: 'right', minWidth: 50 }}>
          <span style={{ color: '#81c784' }}>{node.total_pct.toFixed(1)}%</span>
        </td>
        <td style={{ textAlign: 'right', color: '#aaa', minWidth: 50 }}>{node.call_count}</td>
        <td style={{ textAlign: 'right', color: '#aaa', minWidth: 50 }}>{node.calls_per_frame.toFixed(1)}</td>
      </tr>
      {isExpanded && node.children.map((child, i) => (
        <CallTreeRow
          key={`${child.name}-${i}`}
          node={child}
          depth={depth + 1}
          expanded={expanded}
          toggleExpand={toggleExpand}
          searchQuery={searchQuery}
        />
      ))}
    </>
  );
};

export const CallTreeTable: React.FC<CallTreeTableProps> = ({ nodes, title, searchQuery }) => {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sortCol, setSortCol] = useState<string>('total_self_ms');
  const [sortDesc, setSortDesc] = useState(true);

  const toggleExpand = (key: string) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const expandAll = () => {
    const keys = new Set<string>();
    const collect = (n: CallTreeNode, d: number) => {
      keys.add(`${n.name}-${d}-${n.avg_self_ms.toFixed(3)}`);
      n.children.forEach(c => collect(c, d + 1));
    };
    nodes.forEach(n => collect(n, 0));
    setExpanded(keys);
  };

  const collapseAll = () => setExpanded(new Set());

  const sortedNodes = useMemo(() => {
    const sorted = [...nodes];
    const key = sortCol as keyof CallTreeNode;
    sorted.sort((a, b) => {
      const va = a[key] as number;
      const vb = b[key] as number;
      return sortDesc ? vb - va : va - vb;
    });
    return sorted;
  }, [nodes, sortCol, sortDesc]);

  const handleSort = (col: string) => {
    if (sortCol === col) setSortDesc(!sortDesc);
    else { setSortCol(col); setSortDesc(true); }
  };

  const headerStyle: React.CSSProperties = {
    cursor: 'pointer', textAlign: 'right', padding: '6px 8px',
    borderBottom: '1px solid #333', fontSize: '11px', color: '#888',
    userSelect: 'none', whiteSpace: 'nowrap',
  };

  const sortArrow = (col: string) => sortCol === col ? (sortDesc ? ' ▼' : ' ▲') : '';

  return (
    <div style={{ overflow: 'auto', maxHeight: '500px' }}>
      {title && <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 8 }}>{title}</div>}
      <div style={{ display: 'flex', gap: 8, marginBottom: 6 }}>
        <button onClick={expandAll} style={{ fontSize: 11, cursor: 'pointer', background: '#333', color: '#aaa', border: '1px solid #555', borderRadius: 3, padding: '2px 8px' }}>全部展开</button>
        <button onClick={collapseAll} style={{ fontSize: 11, cursor: 'pointer', background: '#333', color: '#aaa', border: '1px solid #555', borderRadius: 3, padding: '2px 8px' }}>全部折叠</button>
      </div>
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ background: '#0f3460' }}>
            <th style={{ ...headerStyle, textAlign: 'left', paddingLeft: 8 }}>函数名</th>
            <th style={headerStyle}>类别</th>
            <th style={headerStyle} onClick={() => handleSort('avg_self_ms')}>Self(avg){sortArrow('avg_self_ms')}</th>
            <th style={headerStyle} onClick={() => handleSort('total_self_ms')}>Self(total){sortArrow('total_self_ms')}</th>
            <th style={headerStyle} onClick={() => handleSort('self_pct')}>Self%{sortArrow('self_pct')}</th>
            <th style={headerStyle} onClick={() => handleSort('avg_total_ms')}>Total(avg){sortArrow('avg_total_ms')}</th>
            <th style={headerStyle} onClick={() => handleSort('total_total_ms')}>Total(total){sortArrow('total_total_ms')}</th>
            <th style={headerStyle} onClick={() => handleSort('total_pct')}>Total%{sortArrow('total_pct')}</th>
            <th style={headerStyle} onClick={() => handleSort('call_count')}>Calls{sortArrow('call_count')}</th>
            <th style={headerStyle} onClick={() => handleSort('calls_per_frame')}>Calls/F{sortArrow('calls_per_frame')}</th>
          </tr>
        </thead>
        <tbody>
          {sortedNodes.map((node, i) => (
            <CallTreeRow
              key={`${node.name}-${i}`}
              node={node}
              depth={0}
              expanded={expanded}
              toggleExpand={toggleExpand}
              searchQuery={searchQuery}
            />
          ))}
          {sortedNodes.length === 0 && (
            <tr><td colSpan={10} style={{ textAlign: 'center', color: '#666', padding: 20 }}>无函数数据</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
};

export default CallTreeTable;
