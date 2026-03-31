import { useState, useMemo, useCallback, useEffect } from 'react';

export interface TreeNode {
  id: string;
  name: string;
  icon: string;
  children: TreeNode[];
  depth: number;
  data?: any;
}

interface Props {
  tree: TreeNode[];
  selectedId: string | null;
  onSelect: (node: TreeNode) => void;
  search: string;
}

export default function FileTree({ tree, selectedId, onSelect, search }: Props) {
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  const toggleExpand = useCallback((id: string) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const filteredTree = useMemo(() => {
    if (!search) return tree;
    return filterTree(tree, search.toLowerCase());
  }, [tree, search]);

  // Auto-expand all parent nodes when search is active
  useEffect(() => {
    if (!search) return;
    const ids = new Set<string>();
    collectParentIds(filteredTree, ids);
    setExpanded(prev => {
      const next = new Set(prev);
      for (const id of ids) next.add(id);
      return next;
    });
  }, [search, filteredTree]);

  return (
    <div className="file-tree">
      {filteredTree.map(node => (
        <TreeItem
          key={node.id}
          node={node}
          depth={0}
          expanded={expanded}
          toggleExpand={toggleExpand}
          selectedId={selectedId}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}

/** Collect IDs of all nodes that have children (directories to expand) */
function collectParentIds(nodes: TreeNode[], out: Set<string>) {
  for (const node of nodes) {
    if (node.children.length > 0) {
      out.add(node.id);
      collectParentIds(node.children, out);
    }
  }
}

function TreeItem({ node, depth, expanded, toggleExpand, selectedId, onSelect }: {
  node: TreeNode;
  depth: number;
  expanded: Set<string>;
  toggleExpand: (id: string) => void;
  selectedId: string | null;
  onSelect: (node: TreeNode) => void;
}) {
  const hasChildren = node.children.length > 0;
  const isExpanded = expanded.has(node.id);

  return (
    <>
      <div
        className={`tree-row${selectedId === node.id ? ' active' : ''}`}
        style={{ paddingLeft: depth * 16 + 8 }}
        onClick={() => {
          onSelect(node);
          if (hasChildren) toggleExpand(node.id);
        }}
      >
        {hasChildren ? (
          <span className={`tree-arrow${isExpanded ? ' expanded' : ''}`}>▸</span>
        ) : (
          <span className="tree-arrow-space" />
        )}
        <span className="tree-node-icon">{node.icon}</span>
        <span className="tree-name">{node.name}</span>
      </div>
      {isExpanded && node.children.map(child => (
        <TreeItem
          key={child.id}
          node={child}
          depth={depth + 1}
          expanded={expanded}
          toggleExpand={toggleExpand}
          selectedId={selectedId}
          onSelect={onSelect}
        />
      ))}
    </>
  );
}

function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  return nodes
    .map(node => {
      const childMatches = filterTree(node.children, query);
      const selfMatches = node.name.toLowerCase().includes(query);
      if (selfMatches || childMatches.length > 0) {
        return { ...node, children: childMatches };
      }
      return null;
    })
    .filter(Boolean) as TreeNode[];
}
