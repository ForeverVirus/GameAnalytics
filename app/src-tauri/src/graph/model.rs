use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique node identifier
pub type NodeId = String;

/// Engine type detected from project
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EngineType {
    Unity,
    Godot,
    Unknown,
}

/// Node types in the unified graph model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Directory,
    Asset,
    SceneObject,
    CodeFile,
    Class,
    Method,
    MemberVariable,
    Module,
    Interface,
}

/// Asset sub-types for finer classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetKind {
    Texture,
    Material,
    Prefab,
    Scene,
    Audio,
    Animation,
    Shader,
    Script,
    Data,
    Other,
}

/// Edge types in the graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeType {
    Contains,
    Declares,
    References,
    DependsOn,
    MountedOn,
    Calls,
    Inherits,
    Owns,
    DynamicLoad,
}

/// Whether a reference is verified or suspected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReferenceClass {
    Official,
    Suspected,
    UserConfirmed,
}

/// Evidence for how an edge was discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub parser_type: String,
    pub source_file: String,
    pub source_line: Option<u32>,
    pub rule: Option<String>,
}

/// A node in the project graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub name: String,
    pub node_type: NodeType,
    pub asset_kind: Option<AssetKind>,
    pub file_path: Option<String>,
    pub line_number: Option<u32>,
    pub metadata: HashMap<String, String>,
}

/// An edge in the project graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: EdgeType,
    pub reference_class: ReferenceClass,
    pub label: Option<String>,
    pub evidence: Option<Evidence>,
}

/// Summary statistics for a project analysis
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisStats {
    pub total_files: u32,
    pub asset_count: u32,
    pub script_count: u32,
    pub class_count: u32,
    pub method_count: u32,
    pub official_edges: u32,
    pub suspected_count: u32,
    pub hardcode_count: u32,
}

/// A hardcoded string finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardcodeFinding {
    pub id: String,
    pub file_path: String,
    pub line_number: u32,
    pub value: String,
    pub code_excerpt: String,
    pub category: HardcodeCategory,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HardcodeCategory {
    Path,
    Url,
    MagicNumber,
    Color,
    StringLiteral,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

/// A suspected reference entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspectedReference {
    pub id: String,
    pub resource_path: String,
    pub resource_type: Option<AssetKind>,
    pub code_location: String,
    pub code_line: Option<u32>,
    pub code_excerpt: Option<String>,
    pub load_method: String,
    pub confidence: f32,
    pub status: SuspectedStatus,
    pub ai_explanation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuspectedStatus {
    Pending,
    Confirmed,
    Ignored,
}

/// Progress event payload emitted to frontend during analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisProgress {
    pub phase: String,
    pub step: String,
    pub current: u32,
    pub total: u32,
    pub message: String,
}

/// Application settings (persisted to disk)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub ai_cli: String,
    pub language: String,
    pub scan_scope: String,
    pub hardcode_enabled: bool,
    pub suspected_enabled: bool,
    #[serde(default)]
    pub ai_model: Option<String>,
    #[serde(default)]
    pub ai_thinking: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            ai_cli: "claude".to_string(),
            language: "zh".to_string(),
            scan_scope: "full".to_string(),
            hardcode_enabled: true,
            suspected_enabled: true,
            ai_model: None,
            ai_thinking: None,
        }
    }
}
