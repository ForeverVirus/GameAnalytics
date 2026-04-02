import React, { useState, useMemo } from 'react';
import type { CallTreeNode } from '../../api/tauri';
import ResizableTable from './ResizableTable';

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
        <td style={{ paddingLeft: `${depth * 20 + 8}px`, whiteSpace: 'nowrap', color: CategoryColors[node.category] || '#ccc' }} title={node.name}>
          {hasChildren && (
            <span
              onClick={() => toggleExpand(key)}
              style={{ cursor: 'pointer', marginRight: 4, userSelect: 'none', display: 'inline-block', width: 12 }}
            >
              {isExpanded ? '▼' : '▶'}
            </span>
          )}
          {!hasChildren && <span style={{ display: 'inline-block', width: 16 }} />}
          <span>{node.name}</span>
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

  const sortArrow = (col: string) => sortCol === col ? (sortDesc ? ' ▼' : ' ▲') : '';
  const columns = [
    { key: 'name', label: '函数名', width: 500, minWidth: 260 },
    { key: 'category', label: '类别', width: 120, minWidth: 80, align: 'right' as const },
    { key: 'avg_self_ms', label: `Self(avg)${sortArrow('avg_self_ms')}`, width: 110, minWidth: 90, align: 'right' as const, onHeaderClick: () => handleSort('avg_self_ms') },
    { key: 'total_self_ms', label: `Self(total)${sortArrow('total_self_ms')}`, width: 110, minWidth: 90, align: 'right' as const, onHeaderClick: () => handleSort('total_self_ms') },
    { key: 'self_pct', label: `Self%${sortArrow('self_pct')}`, width: 90, minWidth: 70, align: 'right' as const, onHeaderClick: () => handleSort('self_pct') },
    { key: 'avg_total_ms', label: `Total(avg)${sortArrow('avg_total_ms')}`, width: 110, minWidth: 90, align: 'right' as const, onHeaderClick: () => handleSort('avg_total_ms') },
    { key: 'total_total_ms', label: `Total(total)${sortArrow('total_total_ms')}`, width: 110, minWidth: 90, align: 'right' as const, onHeaderClick: () => handleSort('total_total_ms') },
    { key: 'total_pct', label: `Total%${sortArrow('total_pct')}`, width: 90, minWidth: 70, align: 'right' as const, onHeaderClick: () => handleSort('total_pct') },
    { key: 'call_count', label: `Calls${sortArrow('call_count')}`, width: 90, minWidth: 70, align: 'right' as const, onHeaderClick: () => handleSort('call_count') },
    { key: 'calls_per_frame', label: `Calls/F${sortArrow('calls_per_frame')}`, width: 90, minWidth: 70, align: 'right' as const, onHeaderClick: () => handleSort('calls_per_frame') },
  ];

  return (
    <div>
      {title && <div style={{ fontSize: 14, fontWeight: 600, color: '#ccc', marginBottom: 8 }}>{title}</div>}
      <div style={{ display: 'flex', gap: 8, marginBottom: 6 }}>
        <button onClick={expandAll} style={{ fontSize: 11, cursor: 'pointer', background: '#333', color: '#aaa', border: '1px solid #555', borderRadius: 3, padding: '2px 8px' }}>全部展开</button>
        <button onClick={collapseAll} style={{ fontSize: 11, cursor: 'pointer', background: '#333', color: '#aaa', border: '1px solid #555', borderRadius: 3, padding: '2px 8px' }}>全部折叠</button>
      </div>
      <ResizableTable columns={columns} rowCount={sortedNodes.length} maxHeight={500} emptyState="无函数数据">
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
      </ResizableTable>
    </div>
  );
};

export default CallTreeTable;
