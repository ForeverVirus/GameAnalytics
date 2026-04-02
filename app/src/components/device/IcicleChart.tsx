import React, { useRef, useEffect, useState, useCallback } from 'react';
import type { CallTreeNode } from '../../api/tauri';

interface IcicleChartProps {
  nodes: CallTreeNode[];
  height?: number;
  onSelect?: (node: CallTreeNode) => void;
}

const CategoryColors: Record<string, string> = {
  '渲染模块': '#2196f3',
  '用户脚本': '#4caf50',
  '物理模块': '#ff9800',
  '动画模块': '#f44336',
  'UI模块': '#9c27b0',
  '加载模块': '#00bcd4',
  '粒子模块': '#ffeb3b',
  '同步等待': '#607d8b',
  '引擎开销': '#795548',
  'GC': '#e53935',
  '其他': '#555',
  '自定义模块': '#673ab7',
};

interface IcicleRect {
  x: number;
  y: number;
  w: number;
  h: number;
  node: CallTreeNode;
  label: string;
}

export const IcicleChart: React.FC<IcicleChartProps> = ({ nodes, height = 400, onSelect }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 800, height });
  const [hoverRect, setHoverRect] = useState<IcicleRect | null>(null);
  const rectsRef = useRef<IcicleRect[]>([]);

  const rowHeight = 24;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(entries => {
      const entry = entries[0];
      if (entry) setDimensions({ width: entry.contentRect.width, height });
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = dimensions.width * dpr;
    canvas.height = dimensions.height * dpr;
    ctx.scale(dpr, dpr);
    canvas.style.width = `${dimensions.width}px`;
    canvas.style.height = `${dimensions.height}px`;

    const w = dimensions.width;
    const h = dimensions.height;

    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, w, h);

    if (nodes.length === 0) {
      ctx.fillStyle = '#666';
      ctx.font = '14px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('No call tree data for icicle chart', w / 2, h / 2);
      return;
    }

    // Compute total for width proportion
    const totalMs = nodes.reduce((s, n) => s + n.total_total_ms, 0) || 1;
    const rects: IcicleRect[] = [];

    const layout = (nodeList: CallTreeNode[], x0: number, y0: number, availW: number, parentTotal: number) => {
      let cx = x0;
      for (const node of nodeList) {
        const fraction = parentTotal > 0 ? node.total_total_ms / parentTotal : 0;
        const nodeW = Math.max(fraction * availW, 1);
        const label = nodeW > 60 ? node.name : nodeW > 30 ? node.name.substring(0, 6) + '..' : '';
        rects.push({ x: cx, y: y0, w: nodeW, h: rowHeight, node, label });
        if (node.children.length > 0 && nodeW > 2) {
          layout(node.children, cx, y0 + rowHeight, nodeW, node.total_total_ms);
        }
        cx += nodeW;
      }
    };

    layout(nodes, 0, 0, w, totalMs);
    rectsRef.current = rects;

    // Draw
    for (const r of rects) {
      const color = CategoryColors[r.node.category] || '#555';
      const isHovered = hoverRect && hoverRect.node === r.node;
      ctx.fillStyle = isHovered ? lightenColor(color, 30) : color;
      ctx.fillRect(r.x, r.y, r.w - 1, r.h - 1);

      if (r.label && r.w > 20) {
        ctx.fillStyle = '#fff';
        ctx.font = '10px sans-serif';
        ctx.textAlign = 'left';
        ctx.save();
        ctx.beginPath();
        ctx.rect(r.x + 2, r.y, r.w - 4, r.h);
        ctx.clip();
        ctx.fillText(r.label, r.x + 3, r.y + 15);
        ctx.restore();
      }
    }

    // Hover tooltip
    if (hoverRect) {
      const n = hoverRect.node;
      const tooltip = `${n.name}\nSelf: ${n.avg_self_ms.toFixed(3)}ms (${n.self_pct.toFixed(1)}%)\nTotal: ${n.avg_total_ms.toFixed(3)}ms (${n.total_pct.toFixed(1)}%)\nCalls: ${n.call_count}`;
      const lines = tooltip.split('\n');
      const tx = Math.min(hoverRect.x + hoverRect.w / 2, w - 180);
      const ty = Math.min(hoverRect.y + hoverRect.h + 5, h - lines.length * 14 - 10);

      ctx.fillStyle = 'rgba(0,0,0,0.85)';
      ctx.fillRect(tx - 2, ty - 2, 180, lines.length * 14 + 8);
      ctx.fillStyle = '#eee';
      ctx.font = '11px sans-serif';
      ctx.textAlign = 'left';
      lines.forEach((line, i) => {
        ctx.fillText(line, tx + 4, ty + 12 + i * 14);
      });
    }
  }, [nodes, dimensions, hoverRect]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const hit = rectsRef.current.find(r => x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h);
    setHoverRect(hit || null);
  }, []);

  const handleClick = useCallback(() => {
    if (hoverRect && onSelect) onSelect(hoverRect.node);
  }, [hoverRect, onSelect]);

  return (
    <div ref={containerRef} style={{ width: '100%' }}>
      <canvas
        ref={canvasRef}
        style={{ cursor: hoverRect ? 'pointer' : 'default', display: 'block', width: '100%' }}
        onMouseMove={handleMouseMove}
        onClick={handleClick}
        onMouseLeave={() => setHoverRect(null)}
      />
    </div>
  );
};

function lightenColor(hex: string, amount: number): string {
  const num = parseInt(hex.replace('#', ''), 16);
  const r = Math.min(255, (num >> 16) + amount);
  const g = Math.min(255, ((num >> 8) & 0xff) + amount);
  const b = Math.min(255, (num & 0xff) + amount);
  return `rgb(${r},${g},${b})`;
}

export default IcicleChart;
