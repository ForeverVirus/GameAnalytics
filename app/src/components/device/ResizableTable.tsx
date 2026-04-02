import React, { useCallback, useMemo, useRef, useState } from 'react';

export interface ResizableColumn {
  key: string;
  label: React.ReactNode;
  width: number;
  minWidth?: number;
  align?: 'left' | 'right' | 'center';
  sortable?: boolean;
  onHeaderClick?: () => void;
}

interface ResizableTableProps {
  columns: ResizableColumn[];
  children: React.ReactNode;
  emptyState?: React.ReactNode;
  rowCount: number;
  maxHeight?: number;
  fontSize?: number;
}

type DragState = {
  key: string;
  startX: number;
  startWidth: number;
  minWidth: number;
} | null;

export default function ResizableTable({
  columns,
  children,
  emptyState,
  rowCount,
  maxHeight = 400,
  fontSize = 12,
}: ResizableTableProps) {
  const [widths, setWidths] = useState<Record<string, number>>(() =>
    Object.fromEntries(columns.map(column => [column.key, column.width]))
  );
  const dragStateRef = useRef<DragState>(null);

  const totalWidth = useMemo(
    () => columns.reduce((sum, column) => sum + (widths[column.key] ?? column.width), 0),
    [columns, widths]
  );

  const stopDrag = useCallback(() => {
    dragStateRef.current = null;
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', stopDrag);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleMouseMove = useCallback((event: MouseEvent) => {
    const drag = dragStateRef.current;
    if (!drag) return;
    const delta = event.clientX - drag.startX;
    const nextWidth = Math.max(drag.minWidth, drag.startWidth + delta);
    setWidths(prev => ({ ...prev, [drag.key]: nextWidth }));
  }, []);

  const startDrag = useCallback((event: React.MouseEvent, column: ResizableColumn) => {
    event.preventDefault();
    event.stopPropagation();
    dragStateRef.current = {
      key: column.key,
      startX: event.clientX,
      startWidth: widths[column.key] ?? column.width,
      minWidth: column.minWidth ?? 80,
    };
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', stopDrag);
  }, [handleMouseMove, stopDrag, widths]);

  React.useEffect(() => stopDrag, [stopDrag]);

  return (
    <div style={{ overflowX: 'auto', overflowY: 'auto', maxHeight }}>
      <table style={{ minWidth: totalWidth, borderCollapse: 'collapse', fontSize, tableLayout: 'fixed' }}>
        <colgroup>
          {columns.map(column => (
            <col key={column.key} style={{ width: widths[column.key] ?? column.width }} />
          ))}
        </colgroup>
        <thead>
          <tr style={{ background: '#0f3460' }}>
            {columns.map(column => {
              const align = column.align ?? 'left';
              return (
                <th
                  key={column.key}
                  onClick={column.onHeaderClick}
                  style={{
                    position: 'relative',
                    textAlign: align,
                    padding: '6px 8px',
                    borderBottom: '1px solid #333',
                    color: '#888',
                    fontSize: 11,
                    whiteSpace: 'nowrap',
                    userSelect: 'none',
                    cursor: column.onHeaderClick ? 'pointer' : 'default',
                  }}
                >
                  <div style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{column.label}</div>
                  <div
                    onMouseDown={(event) => startDrag(event, column)}
                    style={{
                      position: 'absolute',
                      top: 0,
                      right: -3,
                      width: 8,
                      height: '100%',
                      cursor: 'col-resize',
                      zIndex: 2,
                    }}
                  />
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {rowCount > 0 ? children : (
            <tr>
              <td colSpan={columns.length} style={{ textAlign: 'center', color: '#666', padding: 20 }}>
                {emptyState ?? '暂无数据'}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
