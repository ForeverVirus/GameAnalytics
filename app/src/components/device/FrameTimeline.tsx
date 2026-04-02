import React, { useRef, useEffect, useState, useCallback } from 'react';
import type { TimelinePoint } from '../../api/tauri';

interface TimelineSeries {
  name: string;
  data: TimelinePoint[];
  color: string;
}

interface FrameTimelineProps {
  series: TimelineSeries[];
  height?: number;
  yLabel?: string;
  /** Threshold line (e.g. 16.67ms for 60fps) */
  threshold?: number;
  thresholdLabel?: string;
  /** Frame range selection callback */
  onRangeSelect?: (start: number, end: number) => void;
  /** Single frame click callback */
  onFrameClick?: (point: TimelinePoint) => void;
  /** Selected frame marker */
  selectedTime?: number | null;
}

const COLORS = ['#4fc3f7', '#81c784', '#ffb74d', '#e57373', '#ba68c8', '#4dd0e1', '#fff176', '#a1887f', '#90a4ae'];

export const FrameTimeline: React.FC<FrameTimelineProps> = ({
  series,
  height = 200,
  yLabel = 'ms',
  threshold,
  thresholdLabel,
  onRangeSelect,
  onFrameClick,
  selectedTime = null,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 800, height });
  const [hoverX, setHoverX] = useState<number | null>(null);
  const [selection, setSelection] = useState<{ start: number; end: number } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragStart = useRef<number | null>(null);

  const margin = { top: 20, right: 60, bottom: 30, left: 50 };

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(entries => {
      const entry = entries[0];
      if (entry) {
        setDimensions({ width: entry.contentRect.width, height });
      }
    });
    observer.observe(container);
    return () => observer.disconnect();
  }, [height]);

  // Draw
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
    const plotW = w - margin.left - margin.right;
    const plotH = h - margin.top - margin.bottom;

    // Clear
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, w, h);

    if (series.length === 0 || series.every(s => s.data.length === 0)) {
      ctx.fillStyle = '#666';
      ctx.font = '14px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('No timeline data', w / 2, h / 2);
      return;
    }

    // Compute ranges
    let minTime = Infinity, maxTime = -Infinity, maxVal = 0;
    for (const s of series) {
      for (const p of s.data) {
        if (p.time < minTime) minTime = p.time;
        if (p.time > maxTime) maxTime = p.time;
        if (p.value > maxVal) maxVal = p.value;
      }
    }
    if (threshold && threshold > maxVal) maxVal = threshold * 1.2;
    maxVal = maxVal * 1.1 || 1;
    const timeRange = maxTime - minTime || 1;

    const toX = (t: number) => margin.left + ((t - minTime) / timeRange) * plotW;
    const toY = (v: number) => margin.top + plotH - (v / maxVal) * plotH;

    // Selection highlight
    if (selection) {
      ctx.fillStyle = 'rgba(79, 195, 247, 0.1)';
      const sx = toX(selection.start);
      const ex = toX(selection.end);
      ctx.fillRect(sx, margin.top, ex - sx, plotH);
    }

    // Grid lines
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 0.5;
    const yTicks = 5;
    for (let i = 0; i <= yTicks; i++) {
      const y = margin.top + (plotH / yTicks) * i;
      ctx.beginPath();
      ctx.moveTo(margin.left, y);
      ctx.lineTo(w - margin.right, y);
      ctx.stroke();
      const val = maxVal - (maxVal / yTicks) * i;
      ctx.fillStyle = '#888';
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'right';
      ctx.fillText(val.toFixed(1), margin.left - 5, y + 3);
    }

    // Threshold line
    if (threshold) {
      const ty = toY(threshold);
      ctx.strokeStyle = '#f44336';
      ctx.lineWidth = 1;
      ctx.setLineDash([5, 3]);
      ctx.beginPath();
      ctx.moveTo(margin.left, ty);
      ctx.lineTo(w - margin.right, ty);
      ctx.stroke();
      ctx.setLineDash([]);
      if (thresholdLabel) {
        ctx.fillStyle = '#f44336';
        ctx.font = '10px sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText(thresholdLabel, w - margin.right + 3, ty + 3);
      }
    }

    if (selectedTime !== null && Number.isFinite(selectedTime)) {
      const sx = toX(selectedTime);
      if (sx >= margin.left && sx <= w - margin.right) {
        ctx.strokeStyle = '#ffd54f';
        ctx.lineWidth = 1;
        ctx.setLineDash([3, 3]);
        ctx.beginPath();
        ctx.moveTo(sx, margin.top);
        ctx.lineTo(sx, margin.top + plotH);
        ctx.stroke();
        ctx.setLineDash([]);
      }
    }

    // Draw series
    for (let si = 0; si < series.length; si++) {
      const s = series[si];
      if (s.data.length === 0) continue;
      ctx.strokeStyle = s.color || COLORS[si % COLORS.length];
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      for (let i = 0; i < s.data.length; i++) {
        const x = toX(s.data[i].time);
        const y = toY(s.data[i].value);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();

      // Fill area under curve
      ctx.fillStyle = (s.color || COLORS[si % COLORS.length]) + '20';
      ctx.lineTo(toX(s.data[s.data.length - 1].time), margin.top + plotH);
      ctx.lineTo(toX(s.data[0].time), margin.top + plotH);
      ctx.closePath();
      ctx.fill();
    }

    // Hover line
    if (hoverX !== null && hoverX >= margin.left && hoverX <= w - margin.right) {
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = 0.5;
      ctx.beginPath();
      ctx.moveTo(hoverX, margin.top);
      ctx.lineTo(hoverX, margin.top + plotH);
      ctx.stroke();

      // Tooltip values
      const hoverTime = minTime + ((hoverX - margin.left) / plotW) * timeRange;
      let tooltipY = margin.top + 15;
      for (let si = 0; si < series.length; si++) {
        const s = series[si];
        const closest = s.data.reduce((prev, curr) =>
          Math.abs(curr.time - hoverTime) < Math.abs(prev.time - hoverTime) ? curr : prev
        );
        ctx.fillStyle = s.color || COLORS[si % COLORS.length];
        ctx.font = '11px sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText(`${s.name}: ${closest.value.toFixed(2)} ${yLabel}`, hoverX + 8, tooltipY);
        tooltipY += 14;
      }
    }

    // Y label
    ctx.save();
    ctx.fillStyle = '#888';
    ctx.font = '11px sans-serif';
    ctx.textAlign = 'center';
    ctx.translate(12, margin.top + plotH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText(yLabel, 0, 0);
    ctx.restore();

    // Legend
    let lx = margin.left;
    for (let si = 0; si < series.length; si++) {
      const s = series[si];
      ctx.fillStyle = s.color || COLORS[si % COLORS.length];
      ctx.fillRect(lx, h - 12, 10, 10);
      ctx.fillStyle = '#ccc';
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(s.name, lx + 14, h - 3);
      lx += ctx.measureText(s.name).width + 30;
    }
  }, [series, dimensions, hoverX, threshold, thresholdLabel, selection, selectedTime, yLabel, margin.bottom, margin.left, margin.right, margin.top]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    setHoverX(x);
    if (isDragging && dragStart.current !== null) {
      // compute time from x for selection preview
    }
  }, [isDragging]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    dragStart.current = x;
    setIsDragging(true);
  }, []);

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    if (!isDragging || dragStart.current === null) return;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const endX = e.clientX - rect.left;
    const startX = dragStart.current;

    // All data for time range
    const allPoints = series.flatMap(s => s.data);
    if (allPoints.length === 0) return;
    let minTime = Infinity, maxTime = -Infinity;
    for (const p of allPoints) {
      if (p.time < minTime) minTime = p.time;
      if (p.time > maxTime) maxTime = p.time;
    }
    const plotW = dimensions.width - margin.left - margin.right;
    const timeRange = maxTime - minTime || 1;

    const toTime = (px: number) => minTime + ((px - margin.left) / plotW) * timeRange;
    const t1 = toTime(Math.min(startX, endX));
    const t2 = toTime(Math.max(startX, endX));

    if (Math.abs(endX - startX) < 5) {
      // Click
      const clickTime = toTime(endX);
      const primarySeries = series.find(s => s.data.length > 0);
      if (primarySeries) {
        const closest = primarySeries.data.reduce((prev, curr) =>
          Math.abs(curr.time - clickTime) < Math.abs(prev.time - clickTime) ? curr : prev
        );
        onFrameClick?.(closest);
      }
      setSelection(null);
    } else {
      setSelection({ start: t1, end: t2 });
      onRangeSelect?.(t1, t2);
    }

    setIsDragging(false);
    dragStart.current = null;
  }, [isDragging, series, dimensions, onRangeSelect, onFrameClick, margin.left, margin.right]);

  const handleMouseLeave = useCallback(() => {
    setHoverX(null);
    if (isDragging) {
      setIsDragging(false);
      dragStart.current = null;
    }
  }, [isDragging]);

  return (
    <div ref={containerRef} style={{ width: '100%', position: 'relative' }}>
      <canvas
        ref={canvasRef}
        style={{ cursor: 'crosshair', display: 'block', width: '100%' }}
        onMouseMove={handleMouseMove}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
      />
    </div>
  );
};

export default FrameTimeline;
