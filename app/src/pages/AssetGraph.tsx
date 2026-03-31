import { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import GraphCanvas from '../components/GraphCanvas';
import { NODE_COLORS, ASSET_KIND_COLORS } from '../components/GraphCanvas';
import FileTree, { TreeNode } from '../components/FileTree';
import { api, type FrontendNode, type FrontendEdge } from '../api/tauri';

const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp', 'tga', 'tiff', 'tif', 'ico', 'svg']);

function isImageFile(node: FrontendNode | null): boolean {
  if (!node?.file_path) return false;
  const ext = node.file_path.split('.').pop()?.toLowerCase() ?? '';
  return IMAGE_EXTENSIONS.has(ext);
}

export default function AssetGraph() {
  const { t } = useTranslation();
  const { assetGraph, loadAssetGraph, stats, project, openFileLocation, runAiAnalysis, runDeepAiAnalysis, aiLoading, aiResult, aiError, clearAiResult } = useAppStore();
  const [selectedNode, setSelectedNode] = useState<FrontendNode | null>(null);
  const [searchInput, setSearchInput] = useState('');
  const [search, setSearch] = useState('');
  const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(null);

  useEffect(() => {
    loadAssetGraph();
  }, [loadAssetGraph]);

  // Load image thumbnail when selecting an image-type node
  useEffect(() => {
    if (!selectedNode || !project || !isImageFile(selectedNode)) {
      setThumbnailUrl(null);
      return;
    }
    let cancelled = false;
    api.readImageBase64(selectedNode.file_path!, project.path)
      .then((url: string) => { if (!cancelled) setThumbnailUrl(url); })
      .catch(() => { if (!cancelled) setThumbnailUrl(null); });
    return () => { cancelled = true; };
  }, [selectedNode, project]);

  const nodes = assetGraph?.nodes ?? [];
  const edges = assetGraph?.edges ?? [];

  // Filter out Directory nodes from graph view (they're in the tree already)
  const graphNodes = useMemo(() => {
    let filtered = nodes.filter(n => n.node_type !== 'Directory');
    if (search) {
      filtered = filtered.filter(n => n.name.toLowerCase().includes(search.toLowerCase()));
    }
    return filtered;
  }, [nodes, search]);

  const graphEdges = useMemo(() => {
    const nodeIds = new Set(graphNodes.map(n => n.id));
    return edges.filter(e => nodeIds.has(e.source) && nodeIds.has(e.target));
  }, [edges, graphNodes]);

  // Build directory tree from flat file paths
  const tree = useMemo(() => buildAssetTree(nodes), [nodes]);

  const handleNodeSelect = (treeNode: TreeNode) => {
    if (treeNode.data) {
      setSelectedNode(treeNode.data);
      clearAiResult();
    }
  };

  const handleGraphNodeClick = (node: FrontendNode) => {
    setSelectedNode(node);
    clearAiResult();
  };

  // Ego graph: when a node is selected, show it + 1-hop neighbors
  const { egoNodes, egoEdges } = useMemo(() => {
    if (!selectedNode) return { egoNodes: [] as FrontendNode[], egoEdges: [] as FrontendEdge[] };
    const centerId = selectedNode.id;
    const neighborIds = new Set<string>([centerId]);
    for (const edge of edges) {
      if (edge.source === centerId) neighborIds.add(edge.target);
      if (edge.target === centerId) neighborIds.add(edge.source);
    }
    const egoNodes = nodes.filter(n => neighborIds.has(n.id));
    const egoEdges = edges.filter(e => neighborIds.has(e.source) && neighborIds.has(e.target));
    return { egoNodes, egoEdges };
  }, [selectedNode, nodes, edges]);

  const handleBackToOverview = () => {
    setSelectedNode(null);
    clearAiResult();
  };

  const analysisTargetId = selectedNode ? (selectedNode.file_path ?? selectedNode.id) : null;
  const analysisTargetNode = useMemo(() => {
    if (!selectedNode || !analysisTargetId) return null;
    return nodes.find((n) => n.id === analysisTargetId) ?? selectedNode;
  }, [analysisTargetId, nodes, selectedNode]);
  const analysisTargetSummary = analysisTargetNode?.metadata?.ai_summary;

  const assetLegend = useMemo(() => [
    { label: '贴图 Texture', color: ASSET_KIND_COLORS.Texture, icon: '🖼️' },
    { label: '场景 Scene', color: ASSET_KIND_COLORS.Scene, icon: '🎬' },
    { label: '预制体 Prefab', color: ASSET_KIND_COLORS.Prefab, icon: '🧩' },
    { label: '材质 Material', color: ASSET_KIND_COLORS.Material, icon: '🎨' },
    { label: '音频 Audio', color: ASSET_KIND_COLORS.Audio, icon: '🔊' },
    { label: '动画 Animation', color: ASSET_KIND_COLORS.Animation, icon: '🎞️' },
    { label: '着色器 Shader', color: ASSET_KIND_COLORS.Shader, icon: '✨' },
    { label: '目录 Directory', color: NODE_COLORS.Directory, icon: '📁' },
  ], []);

  return (
    <div className="workbench">
      {/* Left panel - tree */}
      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">📁 {t('graph.resourceDir')}</div>
          <div className="panel-stats">
            {stats?.asset_count ?? 0} {t('graph.resources')} · {stats?.official_edges ?? 0} {t('graph.references')}
          </div>
        </div>
        <div className="panel-search">
          <input
            placeholder={t('graph.searchAsset')}
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') setSearch(searchInput); }}
          />
        </div>
        <div className="panel-body">
          <FileTree
            tree={tree}
            selectedId={selectedNode?.id ?? null}
            onSelect={handleNodeSelect}
            search={search}
          />
        </div>
      </div>

      {/* Center - graph */}
      <div className="graph-center" style={{ position: 'relative' }}>
        <div className="graph-level-bar">
          {selectedNode ? (
            <>
              <button className="graph-level-btn active" onClick={handleBackToOverview}>
                ← 返回总览
              </button>
              <span className="graph-level-count">
                {getAssetIcon(selectedNode.asset_kind)} {selectedNode.name}
              </span>
            </>
          ) : (
            <span className="graph-level-count">{graphNodes.length} 资源 · {graphEdges.length} 引用</span>
          )}
        </div>
        <GraphCanvas
          nodes={selectedNode ? egoNodes : graphNodes}
          edges={selectedNode ? egoEdges : graphEdges}
          onNodeClick={handleGraphNodeClick}
          legendItems={assetLegend}
          centerNodeId={selectedNode?.id ?? null}
        />
        {thumbnailUrl && (
          <div style={{
            position: 'absolute',
            top: 48,
            right: 12,
            background: 'var(--bg-secondary, #1e1e2e)',
            border: '1px solid var(--border-color, #333)',
            borderRadius: 8,
            padding: 6,
            boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
            zIndex: 10,
            maxWidth: 220,
            maxHeight: 220,
          }}>
            <img
              src={thumbnailUrl}
              alt={selectedNode?.name ?? ''}
              style={{
                display: 'block',
                maxWidth: 208,
                maxHeight: 208,
                objectFit: 'contain',
                borderRadius: 4,
              }}
            />
          </div>
        )}
      </div>

      {/* Right panel - detail */}
      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">📋 {t('graph.resourceDetail')}</div>
        </div>
        <div className="panel-body" style={{ padding: '16px' }}>
          {selectedNode ? (
            <>
              <div className="detail-section">
                <div className="detail-label">{t('graph.type')}</div>
                <div className="detail-value">{selectedNode.node_type}</div>
              </div>
              <div className="detail-section">
                <div className="detail-label">{t('graph.path')}</div>
                <div className="detail-value">{selectedNode.id}</div>
              </div>
              {selectedNode.asset_kind && (
                <div className="detail-section">
                  <div className="detail-label">Asset Kind</div>
                  <div className="detail-value">{selectedNode.asset_kind}</div>
                </div>
              )}

              {/* Open File button */}
              {selectedNode.file_path && project && (
                <button
                  className="open-file-btn"
                  onClick={() => openFileLocation(selectedNode.file_path!)}
                >
                  📂 {t('graph.openFile')}
                </button>
              )}

              <div className="detail-section" style={{ marginTop: 12 }}>
                <div className="detail-label">{t('graph.referencedBy')} ({edges.filter(e => e.target === selectedNode.id).length})</div>
                <ul className="detail-ref-list">
                  {edges
                    .filter((e) => e.target === selectedNode.id)
                    .map((e, i) => {
                      const srcNode = nodes.find((n) => n.id === e.source);
                      return (
                        <li key={i} className="detail-ref-item" onClick={() => {
                          if (srcNode) { setSelectedNode(srcNode); clearAiResult(); }
                        }}>
                          <span className="ref-icon">{srcNode ? getAssetIcon(srcNode.asset_kind) : '📄'}</span>
                          {e.source.split(/[\\/]/).pop()} <span className="ref-edge-type">{e.edge_type}</span>
                        </li>
                      );
                    })}
                </ul>
              </div>
              <div className="detail-section">
                <div className="detail-label">{t('graph.dependsOn')} ({edges.filter(e => e.source === selectedNode.id).length})</div>
                <ul className="detail-ref-list">
                  {edges
                    .filter((e) => e.source === selectedNode.id)
                    .map((e, i) => {
                      const tgtNode = nodes.find((n) => n.id === e.target);
                      return (
                        <li key={i} className="detail-ref-item" onClick={() => {
                          if (tgtNode) { setSelectedNode(tgtNode); clearAiResult(); }
                        }}>
                          <span className="ref-icon">{tgtNode ? getAssetIcon(tgtNode.asset_kind) : '📄'}</span>
                          {e.target.split(/[\\/]/).pop()} <span className="ref-edge-type">{e.edge_type}</span>
                        </li>
                      );
                    })}
                </ul>
              </div>

              {/* AI Analysis section */}
              <div className="detail-section ai-section">
                <div className="ai-header">
                  <span className="detail-label" style={{ margin: 0 }}>🤖 {t('graph.aiAnalysis')}</span>
                  <div style={{ display: 'flex', gap: 4 }}>
                    <button
                      className="btn-mini"
                      onClick={() => analysisTargetId && runAiAnalysis(analysisTargetId)}
                      disabled={aiLoading}
                    >
                      {aiLoading ? t('graph.aiAnalyzing') : t('graph.aiAnalyze')}
                    </button>
                    <button
                      className="btn-mini btn-deep"
                      onClick={() => analysisTargetId && runDeepAiAnalysis(analysisTargetId)}
                      disabled={aiLoading}
                    >
                      {aiLoading ? t('graph.aiAnalyzing') : t('graph.aiDeepAnalyze')}
                    </button>
                  </div>
                </div>
                {analysisTargetNode && (
                  <div style={{ color: 'var(--text-dimmer)', fontSize: 11, marginTop: 6 }}>
                    当前为文件级分析：{analysisTargetNode.id}
                  </div>
                )}
                {/* Pre-computed AI summary from batch analysis */}
                {analysisTargetSummary && analysisTargetSummary !== '未分析' && !aiResult && (
                  <div className="ai-result">
                    {analysisTargetSummary}
                  </div>
                )}
                {aiResult && <div className="ai-result">{aiResult}</div>}
                {aiError && <div className="ai-error">{aiError}</div>}
                {!aiResult && !aiError && !analysisTargetSummary && (
                  <div style={{ color: 'var(--text-dimmer)', fontSize: 11, marginTop: 6 }}>
                    {t('graph.aiNoResult')}
                  </div>
                )}
              </div>
            </>
          ) : (
            <div style={{ color: 'var(--text-dimmer)', fontSize: 13, textAlign: 'center', marginTop: 40 }}>
              {t('graph.clickToView')}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function buildAssetTree(nodes: FrontendNode[]): TreeNode[] {
  const root: TreeNode[] = [];
  const dirMap = new Map<string, TreeNode>();

  const assetNodes = nodes
    .filter(n => n.node_type === 'Asset' || n.node_type === 'Directory')
    .sort((a, b) => a.id.localeCompare(b.id));

  for (const node of assetNodes) {
    const parts = node.id.split('/');
    let currentChildren = root;
    let currentPath = '';

    for (let i = 0; i < parts.length - 1; i++) {
      currentPath = currentPath ? `${currentPath}/${parts[i]}` : parts[i];

      if (!dirMap.has(currentPath)) {
        const dirNode: TreeNode = {
          id: currentPath,
          name: parts[i],
          icon: '📁',
          children: [],
          depth: i,
        };
        dirMap.set(currentPath, dirNode);
        currentChildren.push(dirNode);
      }
      currentChildren = dirMap.get(currentPath)!.children;
    }

    currentChildren.push({
      id: node.id,
      name: node.name,
      icon: getAssetIcon(node.asset_kind),
      children: [],
      depth: parts.length - 1,
      data: node,
    });
  }

  return root;
}

function getAssetIcon(kind: string | null): string {
  switch (kind) {
    case 'Texture': return '🖼️';
    case 'Audio': return '🔊';
    case 'Scene': return '🎬';
    case 'Prefab': return '🧩';
    case 'Material': return '🎨';
    case 'Shader': return '✨';
    case 'Script': return '📜';
    case 'Animation': return '🎞️';
    default: return '📄';
  }
}
