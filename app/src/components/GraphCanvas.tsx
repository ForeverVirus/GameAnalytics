import { useEffect, useRef, useState } from 'react';
import * as d3 from 'd3';
import type { FrontendNode, FrontendEdge } from '../api/tauri';

interface Props {
  nodes: FrontendNode[];
  edges: FrontendEdge[];
  onNodeClick?: (node: FrontendNode) => void;
  legendItems?: { label: string; color: string; icon: string }[];
  centerNodeId?: string | null;
}

interface SimNode extends d3.SimulationNodeDatum {
  id: string;
  name: string;
  node_type: string;
  asset_kind: string | null;
}

interface SimLink extends d3.SimulationLinkDatum<SimNode> {
  edge_type: string;
  reference_class: string;
}

export const NODE_COLORS: Record<string, string> = {
  Asset: '#5ce0d2',
  CodeFile: '#6384ff',
  Class: '#a78bfa',
  Method: '#f59e42',
  Module: '#5ce0d2',
  Interface: '#ff6b8a',
  Directory: '#8892a4',
  SceneObject: '#ffb74d',
  MemberVariable: '#6b7b8d',
};

export const ASSET_KIND_COLORS: Record<string, string> = {
  Texture: '#4fc3f7',
  Prefab: '#81c784',
  Material: '#ce93d8',
  Audio: '#ffb74d',
  Scene: '#ff8a65',
  Shader: '#fff176',
  Animation: '#f06292',
  Script: '#6384ff',
};

function getNodeColor(node: { node_type: string; asset_kind: string | null }): string {
  if (node.node_type === 'Asset' && node.asset_kind && ASSET_KIND_COLORS[node.asset_kind]) {
    return ASSET_KIND_COLORS[node.asset_kind];
  }
  return NODE_COLORS[node.node_type] ?? '#5ce0d2';
}

export const NODE_RADIUS: Record<string, number> = {
  Directory: 12,
  Asset: 8,
  CodeFile: 10,
  Class: 10,
  Method: 6,
  Module: 12,
  Interface: 9,
  SceneObject: 8,
  MemberVariable: 5,
};

const DRAG_THRESHOLD = 4;
const CENTER_R = 20;

export default function GraphCanvas({ nodes, edges, onNodeClick, legendItems, centerNodeId }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const onNodeClickRef = useRef(onNodeClick);
  onNodeClickRef.current = onNodeClick;
  const [canvasReady, setCanvasReady] = useState(false);

  // Detect first valid layout size
  useEffect(() => {
    if (canvasReady) return;
    const el = containerRef.current;
    if (!el) return;
    if (el.clientWidth > 0 && el.clientHeight > 0) { setCanvasReady(true); return; }
    const ro = new ResizeObserver(() => {
      if (el.clientWidth > 0 && el.clientHeight > 0) { setCanvasReady(true); ro.disconnect(); }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [canvasReady]);

  useEffect(() => {
    if (!canvasRef.current || !containerRef.current || nodes.length === 0 || !canvasReady) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d')!;
    const container = containerRef.current;
    let width = container.clientWidth;
    let height = container.clientHeight;
    if (width === 0 || height === 0) return;

    let dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;

    // Resize callback set by each layout mode
    let onResize: (() => void) | null = null;
    const ro = new ResizeObserver(() => {
      const w = container.clientWidth, h = container.clientHeight;
      if (w <= 0 || h <= 0 || (w === width && h === height)) return;
      width = w; height = h;
      dpr = window.devicePixelRatio || 1;
      canvas.width = w * dpr;
      canvas.height = h * dpr;
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      if (onResize) onResize();
    });
    ro.observe(container);

    const simNodes: SimNode[] = nodes.map(n => ({ ...n, x: undefined, y: undefined }));
    const nodeMap = new Map(simNodes.map(n => [n.id, n]));

    let transform = d3.zoomIdentity;
    let hoveredNode: SimNode | null = null;

    function toWorldCoords(event: MouseEvent): [number, number] {
      const rect = canvas.getBoundingClientRect();
      const px = event.clientX - rect.left;
      const py = event.clientY - rect.top;
      return [(px - transform.x) / transform.k, (py - transform.y) / transform.k];
    }

    function findNodeAt(wx: number, wy: number): SimNode | null {
      for (let i = simNodes.length - 1; i >= 0; i--) {
        const n = simNodes[i];
        if (n.x == null || n.y == null) continue;
        const r = (n.id === centerNodeId ? CENTER_R : (NODE_RADIUS[n.node_type] ?? 8)) * 1.5;
        const dx = n.x - wx, dy = n.y - wy;
        if (dx * dx + dy * dy < r * r) return n;
      }
      return null;
    }

    // ===== RADIAL LAYOUT MODE =====
    if (centerNodeId && nodeMap.has(centerNodeId)) {
      const centerId = centerNodeId;
      const centerNode = nodeMap.get(centerId)!;
      let cx = width / 2, cy = height / 2;
      centerNode.x = cx;
      centerNode.y = cy;

      // Classify neighbors
      const inIds = new Set<string>();
      const outIds = new Set<string>();
      for (const edge of edges) {
        if (edge.target === centerId && edge.source !== centerId) inIds.add(edge.source);
        if (edge.source === centerId && edge.target !== centerId) outIds.add(edge.target);
      }

      // Left: incoming-only; Right: outgoing (includes bidirectional)
      const allLeftNodes: SimNode[] = [];
      const allRightNodes: SimNode[] = [];
      for (const id of inIds) {
        if (!outIds.has(id)) {
          const n = nodeMap.get(id);
          if (n) allLeftNodes.push(n);
        }
      }
      for (const id of outIds) {
        const n = nodeMap.get(id);
        if (n) allRightNodes.push(n);
      }

      // Cap displayed nodes per side for performance and readability
      const MAX_PER_SIDE = 50;
      const totalLeft = allLeftNodes.length;
      const totalRight = allRightNodes.length;
      const leftNodes = allLeftNodes.slice(0, MAX_PER_SIDE);
      const rightNodes = allRightNodes.slice(0, MAX_PER_SIDE);

      // All edges between displayed (capped) nodes only
      const displayedIds = new Set([centerId, ...leftNodes.map(n => n.id), ...rightNodes.map(n => n.id)]);
      interface RadialEdge { source: SimNode; target: SimNode; edge_type: string }
      const relevantEdges: RadialEdge[] = [];
      for (const edge of edges) {
        const s = nodeMap.get(edge.source);
        const t = nodeMap.get(edge.target);
        if (s && t && displayedIds.has(edge.source) && displayedIds.has(edge.target)) {
          relevantEdges.push({ source: s, target: t, edge_type: edge.edge_type });
        }
      }

      // Position nodes: semicircle fan layout for many nodes, column for few
      let R = Math.min(width * 0.36, height * 0.4, 400);
      const FAN_THRESHOLD = 15; // switch to fan layout above this count

      function positionFan(arr: SimNode[], side: 'left' | 'right') {
        if (arr.length === 0) return;
        if (arr.length <= FAN_THRESHOLD) {
          // Column layout
          const spacing = Math.min(36, (height - 120) / arr.length);
          const xPos = side === 'left' ? cx - R : cx + R;
          arr.forEach((n, i) => {
            n.x = xPos;
            n.y = cy + (i - (arr.length - 1) / 2) * spacing;
          });
        } else {
          // Semicircle fan: spread from -80deg to +80deg (top to bottom)
          const startAngle = side === 'left' ? Math.PI - (80 * Math.PI / 180) : -(80 * Math.PI / 180);
          const endAngle = side === 'left' ? Math.PI + (80 * Math.PI / 180) : (80 * Math.PI / 180);
          const step = (endAngle - startAngle) / Math.max(arr.length - 1, 1);
          // Use multiple rings if too crowded
          const maxPerRing = Math.max(30, Math.floor(2 * Math.PI * R * 0.8 / 26));
          const rings = Math.ceil(arr.length / maxPerRing);
          arr.forEach((n, i) => {
            const ring = Math.floor(i / maxPerRing);
            const idxInRing = i % maxPerRing;
            const ringCount = Math.min(maxPerRing, arr.length - ring * maxPerRing);
            const ringR = R + ring * 60;
            const s = (endAngle - startAngle) / Math.max(ringCount - 1, 1);
            const angle = startAngle + idxInRing * s;
            n.x = cx + ringR * Math.cos(angle);
            n.y = cy + ringR * Math.sin(angle);
          });
        }
      }

      function repositionRadial() {
        cx = width / 2; cy = height / 2;
        R = Math.min(width * 0.36, height * 0.4, 400);
        centerNode.x = cx; centerNode.y = cy;
        positionFan(leftNodes, 'left');
        positionFan(rightNodes, 'right');
      }
      repositionRadial();

      function drawRadial() {
        ctx.save();
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, width, height);
        ctx.translate(transform.x, transform.y);
        ctx.scale(transform.k, transform.k);

        // Section labels
        ctx.font = '12px system-ui, -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        ctx.fillStyle = '#5a6577';
        if (leftNodes.length > 0) {
          const minY = Math.min(...leftNodes.map(n => n.y!));
          const avgX = leftNodes.reduce((s, n) => s + n.x!, 0) / leftNodes.length;
          const leftLabel = totalLeft > leftNodes.length
            ? `\u2190 \u88ab\u5f15\u7528 (${leftNodes.length}/${totalLeft})`
            : `\u2190 \u88ab\u5f15\u7528 (${totalLeft})`;
          ctx.fillText(leftLabel, avgX, minY - 28);
        }
        if (rightNodes.length > 0) {
          const minY = Math.min(...rightNodes.map(n => n.y!));
          const avgX = rightNodes.reduce((s, n) => s + n.x!, 0) / rightNodes.length;
          const rightLabel = totalRight > rightNodes.length
            ? `\u5f15\u7528 \u2192 (${rightNodes.length}/${totalRight})`
            : `\u5f15\u7528 \u2192 (${totalRight})`;
          ctx.fillText(rightLabel, avgX, minY - 28);
        }

        // Viewport culling helpers
        const vx0 = -transform.x / transform.k;
        const vy0 = -transform.y / transform.k;
        const vx1 = (width - transform.x) / transform.k;
        const vy1 = (height - transform.y) / transform.k;
        const MARGIN = 100;
        function inView(x: number, y: number) {
          return x >= vx0 - MARGIN && x <= vx1 + MARGIN && y >= vy0 - MARGIN && y <= vy1 + MARGIN;
        }

        const allNeighbors = [...leftNodes, ...rightNodes];
        const nodeCount = allNeighbors.length;

        // --- Batch non-center edges first (single path, lower alpha) ---
        ctx.beginPath();
        ctx.strokeStyle = '#4a6a8a';
        ctx.lineWidth = 1;
        ctx.globalAlpha = 0.5;
        for (const edge of relevantEdges) {
          const isCenter = edge.source.id === centerId || edge.target.id === centerId;
          if (isCenter) continue;
          const sx = edge.source.x!, sy = edge.source.y!;
          const tx = edge.target.x!, ty = edge.target.y!;
          if (!inView(sx, sy) && !inView(tx, ty)) continue;
          const dx = tx - sx, dy = ty - sy;
          const mx = (sx + tx) / 2, my = (sy + ty) / 2;
          const cpx = mx + dy * 0.3, cpy = my - dx * 0.3;
          ctx.moveTo(sx, sy);
          ctx.quadraticCurveTo(cpx, cpy, tx, ty);
        }
        ctx.stroke();
        ctx.globalAlpha = 1;

        // --- Center edges: individual draw with arrowheads ---
        const showEdgeLabels = nodeCount <= 30 && transform.k > 0.5;
        for (const edge of relevantEdges) {
          const isCenter = edge.source.id === centerId || edge.target.id === centerId;
          if (!isCenter) continue;
          const sx = edge.source.x!, sy = edge.source.y!;
          const tx = edge.target.x!, ty = edge.target.y!;
          if (!inView(sx, sy) && !inView(tx, ty)) continue;
          const dx = tx - sx, dy = ty - sy;
          const mx = (sx + tx) / 2, my = (sy + ty) / 2;
          const cpx = mx + dy * 0.15, cpy = my - dx * 0.15;

          const sr = edge.source.id === centerId ? CENTER_R : (NODE_RADIUS[edge.source.node_type] ?? 8);
          const tr = edge.target.id === centerId ? CENTER_R : (NODE_RADIUS[edge.target.node_type] ?? 8);
          const departAngle = Math.atan2(cpy - sy, cpx - sx);
          const approachAngle = Math.atan2(ty - cpy, tx - cpx);
          const x1 = sx + sr * Math.cos(departAngle);
          const y1 = sy + sr * Math.sin(departAngle);
          const x2 = tx - (tr + 2) * Math.cos(approachAngle);
          const y2 = ty - (tr + 2) * Math.sin(approachAngle);

          ctx.beginPath();
          ctx.strokeStyle = '#4a6888';
          ctx.lineWidth = 1.5;
          ctx.globalAlpha = 0.8;
          ctx.moveTo(x1, y1);
          ctx.quadraticCurveTo(cpx, cpy, x2, y2);
          ctx.stroke();

          // Arrowhead
          const hl = 7;
          ctx.beginPath();
          ctx.fillStyle = '#4a6888';
          ctx.moveTo(x2, y2);
          ctx.lineTo(x2 - hl * Math.cos(approachAngle - Math.PI / 6), y2 - hl * Math.sin(approachAngle - Math.PI / 6));
          ctx.lineTo(x2 - hl * Math.cos(approachAngle + Math.PI / 6), y2 - hl * Math.sin(approachAngle + Math.PI / 6));
          ctx.closePath();
          ctx.fill();
          ctx.globalAlpha = 1;

          // Edge type label (only when few nodes)
          if (showEdgeLabels) {
            ctx.font = '9px system-ui';
            ctx.fillStyle = '#5a6577';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'bottom';
            ctx.fillText(edge.edge_type, cpx, cpy - 4);
          }
        }

        // Center node
        ctx.beginPath();
        ctx.arc(cx, cy, CENTER_R, 0, Math.PI * 2);
        ctx.fillStyle = getNodeColor(centerNode);
        ctx.fill();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 2.5;
        ctx.stroke();

        ctx.font = 'bold 13px system-ui';
        ctx.fillStyle = '#e0e8f0';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'bottom';
        const cLabel = centerNode.name.length > 35 ? centerNode.name.slice(0, 33) + '\u2026' : centerNode.name;
        ctx.fillText(cLabel, cx, cy - CENTER_R - 6);
        ctx.font = '10px system-ui';
        ctx.fillStyle = '#5a6577';
        ctx.textBaseline = 'top';
        ctx.fillText(centerNode.node_type, cx, cy + CENTER_R + 4);

        // --- Neighbor nodes: batch by color key, LOD labels ---
        const showLabels = nodeCount <= 80 || transform.k > 0.8;
        // Batch nodes by color for fewer state changes
        const byColor = new Map<string, { color: string; nodes: SimNode[] }>();
        for (const node of allNeighbors) {
          if (node.x == null || node.y == null) continue;
          if (!inView(node.x, node.y)) continue;
          const c = getNodeColor(node);
          if (!byColor.has(c)) byColor.set(c, { color: c, nodes: [] });
          byColor.get(c)!.nodes.push(node);
        }
        for (const [, { color, nodes: batch }] of byColor) {
          const r = NODE_RADIUS[batch[0].node_type] ?? 8;
          ctx.fillStyle = color;
          ctx.globalAlpha = 0.85;
          ctx.beginPath();
          for (const node of batch) {
            if (hoveredNode?.id === node.id) continue; // draw hovered separately
            ctx.moveTo(node.x! + r, node.y!);
            ctx.arc(node.x!, node.y!, r, 0, Math.PI * 2);
          }
          ctx.fill();
          ctx.globalAlpha = 1;
        }

        // Hovered node highlight
        if (hoveredNode && hoveredNode.x != null && hoveredNode.y != null) {
          const hr = (NODE_RADIUS[hoveredNode.node_type] ?? 8) * 1.3;
          ctx.beginPath();
          ctx.arc(hoveredNode.x, hoveredNode.y, hr, 0, Math.PI * 2);
          ctx.fillStyle = getNodeColor(hoveredNode);
          ctx.fill();
          ctx.strokeStyle = '#ffffff';
          ctx.lineWidth = 1.5;
          ctx.stroke();
        }

        // Labels (LOD: skip when too many + zoomed out)
        if (showLabels) {
          ctx.font = '11px system-ui';
          ctx.textBaseline = 'middle';
          for (const node of allNeighbors) {
            if (node.x == null || node.y == null) continue;
            if (!inView(node.x, node.y)) continue;
            const r = NODE_RADIUS[node.node_type] ?? 8;
            const isHovered = hoveredNode?.id === node.id;
            ctx.fillStyle = isHovered ? '#e0e8f0' : '#8892a4';
            const label = node.name.length > 30 ? node.name.slice(0, 28) + '\u2026' : node.name;
            if (node.x < cx) {
              ctx.textAlign = 'right';
              ctx.fillText(label, node.x - r - 6, node.y);
            } else {
              ctx.textAlign = 'left';
              ctx.fillText(label, node.x + r + 6, node.y);
            }
          }
        } else if (hoveredNode && hoveredNode.x != null && hoveredNode.y != null) {
          // Even when labels hidden, show hovered node label
          const r = NODE_RADIUS[hoveredNode.node_type] ?? 8;
          ctx.font = '11px system-ui';
          ctx.fillStyle = '#e0e8f0';
          ctx.textBaseline = 'middle';
          const label = hoveredNode.name.length > 30 ? hoveredNode.name.slice(0, 28) + '\u2026' : hoveredNode.name;
          if (hoveredNode.x < cx) {
            ctx.textAlign = 'right';
            ctx.fillText(label, hoveredNode.x - r - 6, hoveredNode.y);
          } else {
            ctx.textAlign = 'left';
            ctx.fillText(label, hoveredNode.x + r + 6, hoveredNode.y);
          }
        }

        ctx.restore();
      }

      drawRadial();

      // Zoom with filter (don't capture mousedown on nodes)
      const zoom = d3.zoom<HTMLCanvasElement, unknown>()
        .scaleExtent([0.3, 3])
        .filter((event) => {
          if (event.type !== 'mousedown') return true;
          const [wx, wy] = toWorldCoords(event as MouseEvent);
          return !findNodeAt(wx, wy);
        })
        .on('zoom', (event) => {
          transform = event.transform;
          drawRadial();
        });
      // Remove any previous zoom handlers, reset transform, then attach new zoom
      d3.select(canvas).on('.zoom', null);
      d3.select(canvas).call(zoom);
      d3.select(canvas).call(zoom.transform, d3.zoomIdentity);

      // Mouse handlers for radial mode
      let clickX = 0, clickY = 0;

      function handleMouseDown(event: MouseEvent) {
        if (event.button !== 0) return;
        clickX = event.clientX;
        clickY = event.clientY;
      }

      function handleMouseMove(event: MouseEvent) {
        const [wx, wy] = toWorldCoords(event);
        const node = findNodeAt(wx, wy);
        if (node !== hoveredNode) {
          hoveredNode = node;
          canvas.style.cursor = node ? 'pointer' : 'grab';
          drawRadial();
        }
      }

      function handleMouseUp(event: MouseEvent) {
        const dx = event.clientX - clickX;
        const dy = event.clientY - clickY;
        if (dx * dx + dy * dy < DRAG_THRESHOLD * DRAG_THRESHOLD) {
          const [wx, wy] = toWorldCoords(event);
          const node = findNodeAt(wx, wy);
          if (node) {
            const original = nodes.find(n => n.id === node.id);
            if (original && onNodeClickRef.current) onNodeClickRef.current(original);
          }
        }
      }

      canvas.addEventListener('mousedown', handleMouseDown);
      canvas.addEventListener('mousemove', handleMouseMove);
      canvas.addEventListener('mouseup', handleMouseUp);

      onResize = () => { repositionRadial(); drawRadial(); };

      return () => {
        ro.disconnect();
        d3.select(canvas).on('.zoom', null);
        canvas.removeEventListener('mousedown', handleMouseDown);
        canvas.removeEventListener('mousemove', handleMouseMove);
        canvas.removeEventListener('mouseup', handleMouseUp);
      };
    }

    // ===== FORCE LAYOUT MODE =====
    const simLinks: SimLink[] = edges
      .filter(e => nodeMap.has(e.source) && nodeMap.has(e.target))
      .map(e => ({
        source: e.source,
        target: e.target,
        edge_type: e.edge_type,
        reference_class: e.reference_class,
      }));

    let simSettled = false;
    let dragNode: SimNode | null = null;
    let dragStartX = 0, dragStartY = 0;
    let isDragging = false;

    // --- Cluster assignment: group nodes by file (first :: segment) ---
    const clusterMap = new Map<string, SimNode[]>();
    for (const node of simNodes) {
      const parts = node.id.split('::');
      const cluster = parts[0]; // file-level cluster
      if (!clusterMap.has(cluster)) clusterMap.set(cluster, []);
      clusterMap.get(cluster)!.push(node);
    }
    // Pre-compute cluster centers for clustering force
    const clusterCenters = new Map<string, { x: number; y: number }>();
    const clusterKeys = [...clusterMap.keys()];
    const clusterCount = clusterKeys.length;
    // Arrange clusters in a spiral for initial spread
    clusterKeys.forEach((key, i) => {
      const angle = i * 2.4; // golden angle for even spiral distribution
      const radius = Math.sqrt(i + 1) * Math.min(width, height) * 0.12;
      clusterCenters.set(key, {
        x: width / 2 + radius * Math.cos(angle),
        y: height / 2 + radius * Math.sin(angle),
      });
    });
    // Assign initial positions near cluster center
    for (const [cluster, members] of clusterMap) {
      const center = clusterCenters.get(cluster)!;
      members.forEach((n, i) => {
        const a = (i / members.length) * Math.PI * 2;
        const r = Math.min(30, members.length * 3);
        n.x = center.x + r * Math.cos(a);
        n.y = center.y + r * Math.sin(a);
      });
    }
    // Node-to-cluster map for fast lookup
    const nodeCluster = new Map<string, string>();
    for (const node of simNodes) {
      nodeCluster.set(node.id, node.id.split('::')[0]);
    }

    function getConnectedEdgeIndices(nodeId: string): Set<number> {
      const indices = new Set<number>();
      for (let i = 0; i < simLinks.length; i++) {
        const s = (simLinks[i].source as SimNode).id;
        const t = (simLinks[i].target as SimNode).id;
        if (s === nodeId || t === nodeId) indices.add(i);
      }
      return indices;
    }

    // Throttled draw for smooth zoom
    let drawPending = false;
    function requestDraw() {
      if (drawPending) return;
      drawPending = true;
      requestAnimationFrame(() => { drawPending = false; drawForce(); });
    }

    function drawForce() {
      ctx.save();
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);
      ctx.translate(transform.x, transform.y);
      ctx.scale(transform.k, transform.k);

      const vl = -transform.x / transform.k;
      const vt = -transform.y / transform.k;
      const vr = vl + width / transform.k;
      const vb = vt + height / transform.k;
      const margin = 80;

      const highlightEdges = hoveredNode ? getConnectedEdgeIndices(hoveredNode.id) : new Set<number>();
      const hoveredColor = hoveredNode ? getNodeColor(hoveredNode) : '#5ce0d2';

      const totalEdges = simLinks.length;
      const totalNodes = simNodes.length;
      // LOD: skip non-highlighted edges when zoomed out on large graphs
      const showEdges = totalEdges < 2000 || transform.k > 0.15;

      // Normal edges (with sampling for very large graphs)
      if (showEdges) {
        ctx.strokeStyle = '#4a6a8a';
        ctx.lineWidth = 1;
        ctx.globalAlpha = totalEdges > 3000 ? 0.6 : 0.85;
        ctx.beginPath();
        // Sample edges if too many — show every Nth edge at low zoom
        const edgeSkip = (totalEdges > 5000 && transform.k < 0.5) ? Math.ceil(totalEdges / 3000) : 1;
        for (let i = 0; i < simLinks.length; i += edgeSkip) {
          if (highlightEdges.has(i)) continue;
          const s = simLinks[i].source as SimNode;
          const t = simLinks[i].target as SimNode;
          if (s.x == null || s.y == null || t.x == null || t.y == null) continue;
          if (Math.max(s.x!, t.x!) < vl - margin || Math.min(s.x!, t.x!) > vr + margin) continue;
          if (Math.max(s.y!, t.y!) < vt - margin || Math.min(s.y!, t.y!) > vb + margin) continue;
          ctx.moveTo(s.x!, s.y!);
          ctx.lineTo(t.x!, t.y!);
        }
        ctx.stroke();
        ctx.globalAlpha = 1;
      }

      // Highlighted edges (always show)
      if (highlightEdges.size > 0) {
        ctx.strokeStyle = hoveredColor;
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.8;
        ctx.beginPath();
        for (const i of highlightEdges) {
          const s = simLinks[i].source as SimNode;
          const t = simLinks[i].target as SimNode;
          if (s.x == null || s.y == null || t.x == null || t.y == null) continue;
          ctx.moveTo(s.x!, s.y!);
          ctx.lineTo(t.x!, t.y!);
        }
        ctx.stroke();
        ctx.globalAlpha = 1;
      }

      // Nodes batched by type
      const visibleNodes: SimNode[] = [];
      for (const node of simNodes) {
        if (node.x == null || node.y == null) continue;
        if (node.x < vl - margin || node.x > vr + margin) continue;
        if (node.y < vt - margin || node.y > vb + margin) continue;
        visibleNodes.push(node);
      }

      const colorGroups = new Map<string, { color: string; nodes: SimNode[] }>();
      for (const node of visibleNodes) {
        const c = getNodeColor(node);
        if (!colorGroups.has(c)) colorGroups.set(c, { color: c, nodes: [] });
        colorGroups.get(c)!.nodes.push(node);
      }

      ctx.globalAlpha = 0.8;
      for (const [, { color, nodes: group }] of colorGroups) {
        const r = NODE_RADIUS[group[0].node_type] ?? 8;
        ctx.fillStyle = color;
        ctx.beginPath();
        for (const node of group) {
          ctx.moveTo(node.x! + r, node.y!);
          ctx.arc(node.x!, node.y!, r, 0, Math.PI * 2);
        }
        ctx.fill();
      }
      ctx.globalAlpha = 1;

      // Hovered node
      if (hoveredNode && hoveredNode.x != null && hoveredNode.y != null) {
        const r = (NODE_RADIUS[hoveredNode.node_type] ?? 8) * 1.4;
        ctx.beginPath();
        ctx.arc(hoveredNode.x, hoveredNode.y, r, 0, Math.PI * 2);
        ctx.fillStyle = getNodeColor(hoveredNode);
        ctx.fill();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      // Labels (LOD: tiered by zoom and node count)
      if (transform.k > 0.3) {
        const fontSize = Math.max(8, Math.min(12, 10 / Math.sqrt(transform.k)));
        ctx.font = `${fontSize}px system-ui, -apple-system, sans-serif`;
        ctx.textBaseline = 'middle';

        if (transform.k > 0.7 && visibleNodes.length < 500) {
          // High zoom + few visible: show all labels
          ctx.fillStyle = '#8892a4';
          for (const node of visibleNodes) {
            const label = node.name.length > 24 ? node.name.slice(0, 22) + '\u2026' : node.name;
            const r = NODE_RADIUS[node.node_type] ?? 8;
            ctx.fillText(label, node.x! + r + 4, node.y!);
          }
        } else if (transform.k > 0.4) {
          // Medium zoom: only show labels for large node types (files, classes)
          ctx.fillStyle = '#8892a4';
          const importantTypes = new Set(['CodeFile', 'Class', 'Interface', 'Module', 'Asset', 'Directory']);
          for (const node of visibleNodes) {
            if (!importantTypes.has(node.node_type)) continue;
            const label = node.name.length > 24 ? node.name.slice(0, 22) + '\u2026' : node.name;
            const r = NODE_RADIUS[node.node_type] ?? 8;
            ctx.fillText(label, node.x! + r + 4, node.y!);
          }
        }
        // Always show hovered label
        if (hoveredNode && hoveredNode.x != null) {
          ctx.fillStyle = '#c0cad8';
          const label = hoveredNode.name.length > 30 ? hoveredNode.name.slice(0, 28) + '\u2026' : hoveredNode.name;
          const r = NODE_RADIUS[hoveredNode.node_type] ?? 8;
          ctx.fillText(label, hoveredNode.x + r + 4, hoveredNode.y!);
        }
      }

      ctx.restore();
    }

    // Zoom
    const zoom = d3.zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.02, 4])
      .filter((event) => {
        if (event.type !== 'mousedown') return true;
        const [wx, wy] = toWorldCoords(event as MouseEvent);
        return !findNodeAt(wx, wy);
      })
      .on('zoom', (event) => {
        transform = event.transform;
        requestDraw();
      });
    // Remove any previous zoom handlers, reset transform, then attach new zoom
    d3.select(canvas).on('.zoom', null);
    d3.select(canvas).call(zoom);
    d3.select(canvas).call(zoom.transform, d3.zoomIdentity);

    // Force simulation with cluster-aware forces
    const nodeCount = simNodes.length;
    const edgeCount = simLinks.length;
    // Very strong repulsion to overcome link pull
    const chargeStrength = nodeCount > 2000 ? -300 : nodeCount > 500 ? -600 : -1000;
    const linkDist = nodeCount > 2000 ? 150 : nodeCount > 500 ? 200 : 300;
    const alphaDecay = nodeCount > 2000 ? 0.04 : nodeCount > 500 ? 0.03 : 0.02;

    // Degree map for adaptive link strength
    const degreeMap = new Map<string, number>();
    for (const link of simLinks) {
      degreeMap.set(link.source as string, (degreeMap.get(link.source as string) ?? 0) + 1);
      degreeMap.set(link.target as string, (degreeMap.get(link.target as string) ?? 0) + 1);
    }

    // Custom clustering force: gently pull nodes toward their file-cluster centroid
    function clusterForce(alpha: number) {
      const strength = 0.15 * alpha;
      for (const node of simNodes) {
        const cluster = nodeCluster.get(node.id);
        if (!cluster) continue;
        const members = clusterMap.get(cluster);
        if (!members || members.length <= 1) continue;
        let cx = 0, cy = 0, count = 0;
        for (const m of members) {
          if (m.x != null && m.y != null) { cx += m.x; cy += m.y; count++; }
        }
        if (count === 0) continue;
        cx /= count; cy /= count;
        node.vx = (node.vx ?? 0) + (cx - (node.x ?? 0)) * strength;
        node.vy = (node.vy ?? 0) + (cy - (node.y ?? 0)) * strength;
      }
    }

    // Link strength: very weak, inversely proportional to endpoint degrees
    // High-degree nodes barely pull — prevents star-collapse
    const baseLinkStrength = edgeCount > 3000 ? 0.02 : edgeCount > 1000 ? 0.05 : 0.08;
    function linkStrengthFn(link: SimLink) {
      const sId = typeof link.source === 'string' ? link.source : (link.source as SimNode).id;
      const tId = typeof link.target === 'string' ? link.target : (link.target as SimNode).id;
      const sd = degreeMap.get(sId) ?? 1;
      const td = degreeMap.get(tId) ?? 1;
      return baseLinkStrength / Math.sqrt(Math.min(sd, td));
    }

    const simulation = d3.forceSimulation(simNodes)
      .force('link', d3.forceLink<SimNode, SimLink>(simLinks).id(d => d.id).distance(linkDist).strength(linkStrengthFn))
      .force('charge', d3.forceManyBody().strength(chargeStrength).theta(1.0).distanceMax(2000))
      .force('center', d3.forceCenter(width / 2, height / 2).strength(0.05))
      .force('cluster', clusterForce)
      .force('collide', d3.forceCollide<SimNode>().radius(d => (NODE_RADIUS[d.node_type] ?? 8) + 4).strength(0.7).iterations(2))
      .alphaDecay(alphaDecay)
      .on('tick', drawForce)
      .on('end', () => { simSettled = true; });

    // Mouse handlers for force mode
    function handleMouseDown(event: MouseEvent) {
      if (event.button !== 0) return;
      const [wx, wy] = toWorldCoords(event);
      const node = findNodeAt(wx, wy);
      if (node) {
        dragNode = node;
        isDragging = false;
        dragStartX = event.clientX;
        dragStartY = event.clientY;
        if (!simSettled) {
          simulation.alphaTarget(0.1).restart();
        }
        dragNode.fx = dragNode.x;
        dragNode.fy = dragNode.y;
        event.preventDefault();
      }
    }

    function handleMouseMove(event: MouseEvent) {
      if (dragNode) {
        const dx = event.clientX - dragStartX;
        const dy = event.clientY - dragStartY;
        if (dx * dx + dy * dy > DRAG_THRESHOLD * DRAG_THRESHOLD) isDragging = true;
        const [wx, wy] = toWorldCoords(event);
        if (simSettled) {
          dragNode.x = wx;
          dragNode.y = wy;
          dragNode.fx = wx;
          dragNode.fy = wy;
          drawForce();
        } else {
          dragNode.fx = wx;
          dragNode.fy = wy;
        }
        return;
      }
      const [wx, wy] = toWorldCoords(event);
      const node = findNodeAt(wx, wy);
      if (node !== hoveredNode) {
        hoveredNode = node;
        canvas.style.cursor = node ? 'pointer' : 'grab';
        drawForce();
      }
    }

    function handleMouseUp() {
      if (dragNode) {
        if (!simSettled) {
          simulation.alphaTarget(0);
        }
        if (!isDragging) {
          const original = nodes.find(n => n.id === dragNode!.id);
          if (original && onNodeClickRef.current) onNodeClickRef.current(original);
        }
        dragNode.fx = null;
        dragNode.fy = null;
        dragNode = null;
        isDragging = false;
      }
    }

    function handleMouseLeave() {
      if (hoveredNode) {
        hoveredNode = null;
        drawForce();
      }
    }

    canvas.addEventListener('mousedown', handleMouseDown);
    canvas.addEventListener('mousemove', handleMouseMove);
    canvas.addEventListener('mouseup', handleMouseUp);
    canvas.addEventListener('mouseleave', handleMouseLeave);

    onResize = () => {
      simulation.force('center', d3.forceCenter(width / 2, height / 2).strength(0.05));
      simulation.alpha(0.05).restart();
    };

    return () => {
      ro.disconnect();
      simulation.stop();
      d3.select(canvas).on('.zoom', null);
      canvas.removeEventListener('mousedown', handleMouseDown);
      canvas.removeEventListener('mousemove', handleMouseMove);
      canvas.removeEventListener('mouseup', handleMouseUp);
      canvas.removeEventListener('mouseleave', handleMouseLeave);
    };
  }, [nodes, edges, canvasReady, centerNodeId]);

  return (
    <div className="graph-canvas" ref={containerRef}>
      <canvas ref={canvasRef} />
      {legendItems && legendItems.length > 0 && (
        <div className="graph-legend">
          {legendItems.map(item => (
            <div key={item.label} className="legend-item">
              <span className="legend-dot" style={{ background: item.color }} />
              <span className="legend-icon">{item.icon}</span>
              <span className="legend-label">{item.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
