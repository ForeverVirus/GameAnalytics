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

// ======================== V2: Redundancy Detection ========================

/// An orphaned node with no references
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanReport {
    pub node_id: NodeId,
    pub node_name: String,
    pub node_type: NodeType,
    pub asset_kind: Option<AssetKind>,
    pub file_path: Option<String>,
    pub file_size_bytes: u64,
    pub suggestion: String,
}

/// A group of duplicate or similar files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub asset_kind: Option<AssetKind>,
    pub files: Vec<DuplicateItem>,
    pub total_size: u64,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateItem {
    pub node_id: NodeId,
    pub file_path: String,
    pub file_size: u64,
    pub hash: Option<String>,
}

/// A hotspot node with too many dependents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotReport {
    pub node_id: NodeId,
    pub node_name: String,
    pub node_type: NodeType,
    pub file_path: Option<String>,
    pub in_degree: u32,
    pub dependents: Vec<NodeId>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    High,
    Medium,
    Low,
}

// ======================== V2: Asset Metrics ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetrics {
    pub node_id: NodeId,
    pub file_path: String,
    pub file_size_bytes: u64,
    // Texture metrics
    pub texture_width: Option<u32>,
    pub texture_height: Option<u32>,
    pub texture_format: Option<String>,
    pub estimated_memory_bytes: Option<u64>,
    pub has_mipmaps: Option<bool>,
    // Model metrics
    pub vertex_count: Option<u32>,
    pub triangle_count: Option<u32>,
    pub submesh_count: Option<u32>,
    // Audio metrics
    pub sample_rate: Option<u32>,
    pub duration_seconds: Option<f32>,
    pub channels: Option<u32>,
    // Rating
    pub performance_rating: Option<String>,
    pub ai_optimization_suggestion: Option<String>,
}

// ======================== V2: AI Code Review ========================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewType {
    Line,
    Architecture,
    Performance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewSeverity {
    Critical,
    Warning,
    Info,
    Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub id: String,
    pub review_type: ReviewType,
    pub file_path: String,
    pub line_number: Option<u32>,
    pub line_end: Option<u32>,
    pub severity: ReviewSeverity,
    pub category: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub node_id: NodeId,
    pub review_type: ReviewType,
    pub findings: Vec<ReviewFinding>,
    pub summary: String,
    pub timestamp: String,
    pub raw_response: String,
}
