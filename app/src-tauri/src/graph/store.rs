use crate::graph::model::*;
use std::collections::HashMap;

/// In-memory graph store holding nodes, edges, and analysis results
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GraphStore {
    pub nodes: HashMap<NodeId, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub suspected_refs: Vec<SuspectedReference>,
    pub hardcode_findings: Vec<HardcodeFinding>,
    pub stats: AnalysisStats,
}

impl GraphStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    pub fn get_node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.get(id)
    }

    /// Get all edges where the given node is the target (upstream references)
    pub fn get_upstream(&self, node_id: &str) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.target == node_id).collect()
    }

    /// Get all edges where the given node is the source (downstream dependencies)
    pub fn get_downstream(&self, node_id: &str) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.source == node_id).collect()
    }

    /// Get official edges only
    pub fn official_edges(&self) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.reference_class == ReferenceClass::Official)
            .collect()
    }

    /// Add a suspected reference
    pub fn add_suspected_ref(&mut self, sr: SuspectedReference) {
        self.suspected_refs.push(sr);
    }

    /// Promote a suspected reference to official graph
    pub fn promote_suspected(&mut self, suspected_id: &str) -> bool {
        if let Some(sr) = self
            .suspected_refs
            .iter_mut()
            .find(|s| s.id == suspected_id)
        {
            sr.status = SuspectedStatus::Confirmed;
            let already_exists = self.edges.iter().any(|edge| {
                edge.source == sr.code_location
                    && edge.target == sr.resource_path
                    && edge.edge_type == EdgeType::DynamicLoad
                    && edge.label.as_deref() == Some(sr.load_method.as_str())
            });

            if !already_exists {
                let edge = GraphEdge {
                    source: sr.code_location.clone(),
                    target: sr.resource_path.clone(),
                    edge_type: EdgeType::DynamicLoad,
                    reference_class: ReferenceClass::UserConfirmed,
                    label: Some(sr.load_method.clone()),
                    evidence: Some(Evidence {
                        parser_type: "user_promotion".to_string(),
                        source_file: sr.code_location.clone(),
                        source_line: sr.code_line,
                        rule: Some("suspected_reference_promoted".to_string()),
                    }),
                };
                self.edges.push(edge);
            }
            true
        } else {
            false
        }
    }

    /// Ignore a suspected reference
    pub fn ignore_suspected(&mut self, suspected_id: &str) -> bool {
        if let Some(sr) = self
            .suspected_refs
            .iter_mut()
            .find(|s| s.id == suspected_id)
        {
            sr.status = SuspectedStatus::Ignored;
            true
        } else {
            false
        }
    }

    /// Add a hardcode finding
    pub fn add_hardcode_finding(&mut self, finding: HardcodeFinding) {
        self.hardcode_findings.push(finding);
    }

    /// Recalculate stats from current data
    pub fn recalculate_stats(&mut self) {
        self.stats.official_edges = self
            .edges
            .iter()
            .filter(|e| {
                matches!(
                    e.reference_class,
                    ReferenceClass::Official | ReferenceClass::UserConfirmed
                )
            })
            .count() as u32;
        self.stats.suspected_count = self
            .suspected_refs
            .iter()
            .filter(|s| s.status == SuspectedStatus::Pending)
            .count() as u32;
        self.stats.hardcode_count = self.hardcode_findings.len() as u32;

        let mut assets = 0u32;
        let mut scripts = 0u32;
        let mut classes = 0u32;
        let mut methods = 0u32;
        for node in self.nodes.values() {
            match node.node_type {
                NodeType::Asset => assets += 1,
                NodeType::CodeFile => scripts += 1,
                NodeType::Class | NodeType::Interface => classes += 1,
                NodeType::Method => methods += 1,
                _ => {}
            }
        }
        self.stats.asset_count = assets;
        self.stats.script_count = scripts;
        self.stats.class_count = classes;
        self.stats.method_count = methods;
        self.stats.total_files = assets + scripts;
    }

    /// Serialize the entire graph for frontend consumption
    pub fn to_frontend_graph(&self) -> FrontendGraph {
        let nodes: Vec<FrontendNode> = self
            .nodes
            .values()
            .map(|n| FrontendNode {
                id: n.id.clone(),
                name: n.name.clone(),
                node_type: n.node_type.clone(),
                asset_kind: n.asset_kind.clone(),
                file_path: n.file_path.clone(),
                line_number: n.line_number,
                metadata: n.metadata.clone(),
            })
            .collect();

        let edges: Vec<FrontendEdge> = self
            .edges
            .iter()
            .map(|e| FrontendEdge {
                source: e.source.clone(),
                target: e.target.clone(),
                edge_type: e.edge_type.clone(),
                reference_class: e.reference_class.clone(),
                label: e.label.clone(),
            })
            .collect();

        FrontendGraph { nodes, edges }
    }
}

/// Simplified graph data for frontend rendering
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrontendGraph {
    pub nodes: Vec<FrontendNode>,
    pub edges: Vec<FrontendEdge>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrontendNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: NodeType,
    pub asset_kind: Option<AssetKind>,
    pub file_path: Option<String>,
    pub line_number: Option<u32>,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrontendEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: EdgeType,
    pub reference_class: ReferenceClass,
    pub label: Option<String>,
}
