import { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../store';
import GraphCanvas from '../components/GraphCanvas';
import { NODE_COLORS } from '../components/GraphCanvas';
import FileTree, { TreeNode } from '../components/FileTree';
import type { FrontendNode, FrontendEdge } from '../api/tauri';

export default function CodeGraph() {
  const { t } = useTranslation();
  const { codeGraph, loadCodeGraph, stats, project, openFileLocation, runAiAnalysis, runDeepAiAnalysis, aiLoading, aiResult, aiError, clearAiResult } = useAppStore();
  const [selectedNode, setSelectedNode] = useState<FrontendNode | null>(null);
  const [searchInput, setSearchInput] = useState('');
  const [search, setSearch] = useState('');
  const [graphLevel, setGraphLevel] = useState<'file' | 'class' | 'all'>('file');

  useEffect(() => {
    loadCodeGraph();
  }, [loadCodeGraph]);

  const nodes = codeGraph?.nodes ?? [];
  const edges = codeGraph?.edges ?? [];

  // Graph-level filtering: show only selected node types in the force graph
  const graphNodes = useMemo(() => {
    let filtered = nodes;
    if (graphLevel === 'file') {
      filtered = nodes.filter(n => n.node_type === 'CodeFile');
    } else if (graphLevel === 'class') {
      filtered = nodes.filter(n => ['CodeFile', 'Class', 'Interface', 'Module'].includes(n.node_type));
    }
    if (search) {
      filtered = filtered.filter(n => n.name.toLowerCase().includes(search.toLowerCase()));
    }
    return filtered;
  }, [nodes, graphLevel, search]);

  // Project edges to match the filtered node level
  const graphEdges = useMemo(() => {
    const nodeIds = new Set(graphNodes.map(n => n.id));

    if (graphLevel === 'all') {
      return edges.filter(e => nodeIds.has(e.source) && nodeIds.has(e.target));
    }

    // Map each node to its closest displayed ancestor
    function findDisplayParent(nodeId: string): string | null {
      if (nodeIds.has(nodeId)) return nodeId;
      const parts = nodeId.split('::');
      for (let i = parts.length - 1; i >= 1; i--) {
        const prefix = parts.slice(0, i).join('::');
        if (nodeIds.has(prefix)) return prefix;
      }
      return null;
    }

    const seen = new Set<string>();
    const projected: FrontendEdge[] = [];
    for (const edge of edges) {
      if (edge.edge_type === 'Contains' || edge.edge_type === 'Declares') continue;
      const s = findDisplayParent(edge.source);
      const t = findDisplayParent(edge.target);
      if (s && t && s !== t) {
        const key = `${s}\u2192${t}`;
        if (!seen.has(key)) {
          seen.add(key);
          projected.push({ source: s, target: t, edge_type: 'References', reference_class: '', label: null });
        }
      }
    }
    return projected;
  }, [edges, graphNodes, graphLevel]);

  // Build code structure tree: directories > files > classes > methods/members
  const tree = useMemo(() => buildCodeTree(nodes, edges), [nodes, edges]);

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
  const isFileScopedAnalysis = !!selectedNode && analysisTargetId !== selectedNode.id;

  const codeLegend = useMemo(() => [
    { label: '代码文件 CodeFile', color: NODE_COLORS.CodeFile, icon: '📜' },
    { label: '类 Class', color: NODE_COLORS.Class, icon: '🟣' },
    { label: '函数 Method', color: NODE_COLORS.Method, icon: '🟠' },
    { label: '成员变量 MemberVar', color: NODE_COLORS.MemberVariable, icon: '🔹' },
    { label: '模块 Module', color: NODE_COLORS.Module, icon: '🔵' },
    { label: '接口 Interface', color: NODE_COLORS.Interface, icon: '🔴' },
  ], []);

  return (
    <div className="workbench">
      {/* Left panel - tree */}
      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">🔗 {t('graph.projectStructure')}</div>
          <div className="panel-stats">
            {stats?.script_count ?? 0} {t('graph.nodes')} · {stats?.class_count ?? 0} {t('graph.classes')} · {stats?.method_count ?? 0} {t('graph.functions')}
          </div>
        </div>
        <div className="panel-search">
          <input
            placeholder={t('graph.searchCode')}
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
      <div className="graph-center">
        <div className="graph-level-bar">
          {selectedNode ? (
            <>
              <button className="graph-level-btn active" onClick={handleBackToOverview}>
                ← 返回总览
              </button>
              <span className="graph-level-count">
                {getCodeIcon(selectedNode.node_type)} {selectedNode.name}
              </span>
            </>
          ) : (
            <>
              {(['file', 'class', 'all'] as const).map(level => (
                <button
                  key={level}
                  className={`graph-level-btn${graphLevel === level ? ' active' : ''}`}
                  onClick={() => setGraphLevel(level)}
                >
                  {level === 'file' ? '📄 文件级' : level === 'class' ? '🟣 类级' : '🔗 全部'}
                </button>
              ))}
              <span className="graph-level-count">{graphNodes.length} 节点 · {graphEdges.length} 连线</span>
            </>
          )}
        </div>
        <GraphCanvas
          nodes={selectedNode ? egoNodes : graphNodes}
          edges={selectedNode ? egoEdges : graphEdges}
          onNodeClick={handleGraphNodeClick}
          legendItems={codeLegend}
          centerNodeId={selectedNode?.id ?? null}
        />
      </div>

      {/* Right panel - detail */}
      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">📋 {t('graph.nodeDetail')}</div>
        </div>
        <div className="panel-body" style={{ padding: '16px' }}>
          {selectedNode ? (
            <>
              <div className="detail-section">
                <div className="detail-label">{t('graph.type')}</div>
                <div className="detail-value">{selectedNode.node_type}</div>
              </div>
              <div className="detail-section">
                <div className="detail-label">{t('graph.file')}</div>
                <div className="detail-value">{selectedNode.file_path ?? selectedNode.id}</div>
              </div>
              {selectedNode.line_number && (
                <div className="detail-section">
                  <div className="detail-label">{t('graph.line')}</div>
                  <div className="detail-value">L{selectedNode.line_number}</div>
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
                <div className="detail-label">{t('graph.calledBy')} ({edges.filter(e => e.target === selectedNode.id).length})</div>
                <ul className="detail-ref-list">
                  {edges
                    .filter((e) => e.target === selectedNode.id)
                    .map((e, i) => {
                      const srcNode = nodes.find((n) => n.id === e.source);
                      return (
                        <li key={i} className="detail-ref-item" onClick={() => {
                          if (srcNode) { setSelectedNode(srcNode); clearAiResult(); }
                        }}>
                          <span className="ref-icon">{srcNode ? getCodeIcon(srcNode.node_type) : '⚪'}</span>
                          {e.source.split(/[\\/]/).pop()?.split('::').pop()} <span className="ref-edge-type">{e.edge_type}</span>
                        </li>
                      );
                    })}
                </ul>
              </div>
              <div className="detail-section">
                <div className="detail-label">{t('graph.calls')} ({edges.filter(e => e.source === selectedNode.id).length})</div>
                <ul className="detail-ref-list">
                  {edges
                    .filter((e) => e.source === selectedNode.id)
                    .map((e, i) => {
                      const tgtNode = nodes.find((n) => n.id === e.target);
                      return (
                        <li key={i} className="detail-ref-item" onClick={() => {
                          if (tgtNode) { setSelectedNode(tgtNode); clearAiResult(); }
                        }}>
                          <span className="ref-icon">{tgtNode ? getCodeIcon(tgtNode.node_type) : '⚪'}</span>
                          {e.target.split(/[\\/]/).pop()?.split('::').pop()} <span className="ref-edge-type">{e.edge_type}</span>
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
                    {isFileScopedAnalysis ? '当前选择会按所属文件分析' : '当前为文件级分析'}：{analysisTargetNode.id}
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

function buildCodeTree(nodes: FrontendNode[], edges: FrontendEdge[]): TreeNode[] {
  // Build parent → children map from Contains/Declares edges
  const childMap = new Map<string, string[]>();
  for (const edge of edges) {
    if (edge.edge_type === 'Contains' || edge.edge_type === 'Declares') {
      const children = childMap.get(edge.source) ?? [];
      children.push(edge.target);
      childMap.set(edge.source, children);
    }
  }

  const nodeMap = new Map(nodes.map(n => [n.id, n]));

  function buildSubTree(nodeId: string): TreeNode | null {
    const node = nodeMap.get(nodeId);
    if (!node) return null;

    const childIds = childMap.get(nodeId) ?? [];
    const children = childIds
      .map(id => buildSubTree(id))
      .filter(Boolean) as TreeNode[];

    return {
      id: node.id,
      name: node.name,
      icon: getCodeIcon(node.node_type),
      children,
      depth: 0,
      data: node,
    };
  }

  // Group code files by directory
  const root: TreeNode[] = [];
  const dirMap = new Map<string, TreeNode>();

  const codeFiles = nodes
    .filter(n => n.node_type === 'CodeFile')
    .sort((a, b) => a.id.localeCompare(b.id));

  for (const file of codeFiles) {
    const parts = file.id.split('/');
    let currentChildren = root;
    let currentPath = '';

    for (let i = 0; i < parts.length - 1; i++) {
      currentPath = currentPath ? `${currentPath}/${parts[i]}` : parts[i];

      if (!dirMap.has(currentPath)) {
        const dirNode: TreeNode = {
          id: `dir:${currentPath}`,
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

    const fileTree = buildSubTree(file.id);
    if (fileTree) {
      currentChildren.push(fileTree);
    }
  }

  return root;
}

function getCodeIcon(type: string): string {
  switch (type) {
    case 'Class': return '🟣';
    case 'Method': return '🟠';
    case 'MemberVariable': return '🔹';
    case 'Module': return '🔵';
    case 'Interface': return '🔴';
    case 'CodeFile': return '📜';
    default: return '⚪';
  }
}
