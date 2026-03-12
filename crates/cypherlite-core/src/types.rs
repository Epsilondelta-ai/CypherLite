// Core types for CypherLite storage engine

use serde::{Deserialize, Serialize};

/// Unique identifier for a node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Unique identifier for an edge in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub u64);

/// Unique identifier for a page in the database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PageId(pub u32);

/// Property values that can be stored on nodes and edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    /// Null / missing value (type tag 0)
    Null,
    /// Boolean value (type tag 1)
    Bool(bool),
    /// 64-bit signed integer (type tag 2)
    Int64(i64),
    /// 64-bit floating point (type tag 3)
    Float64(f64),
    /// UTF-8 string (type tag 4)
    String(String),
    /// Raw byte array (type tag 5)
    Bytes(Vec<u8>),
    /// Nested array of property values (type tag 6)
    Array(Vec<PropertyValue>),
    /// DateTime as milliseconds since Unix epoch (type tag 7)
    DateTime(i64),
}

/// Unique identifier for a subgraph entity.
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SubgraphId(pub u64);

/// A subgraph record stored in the subgraph store.
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphRecord {
    /// Unique identifier for this subgraph.
    pub subgraph_id: SubgraphId,
    /// Optional temporal anchor (milliseconds since Unix epoch).
    pub temporal_anchor: Option<i64>,
    /// Key-value property pairs stored on this subgraph.
    pub properties: Vec<(u32, PropertyValue)>,
}

/// Unique identifier for a hyperedge entity.
#[cfg(feature = "hypergraph")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HyperEdgeId(pub u64);

/// A hyperedge record connecting multiple sources to multiple targets.
#[cfg(feature = "hypergraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct HyperEdgeRecord {
    /// Unique identifier for this hyperedge.
    pub id: HyperEdgeId,
    /// Relationship type ID for this hyperedge.
    pub rel_type_id: u32,
    /// Source endpoints (nodes, subgraphs, or other hyperedges).
    pub sources: Vec<GraphEntity>,
    /// Target endpoints (nodes, subgraphs, or other hyperedges).
    pub targets: Vec<GraphEntity>,
    /// Key-value property pairs stored on this hyperedge.
    pub properties: Vec<(u32, PropertyValue)>,
}

/// A graph entity that can be either a node or a subgraph.
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub enum GraphEntity {
    /// A regular node in the graph.
    Node(NodeId),
    /// A subgraph containing other entities.
    Subgraph(SubgraphId),
    /// A hyperedge connecting multiple sources and targets.
    #[cfg(feature = "hypergraph")]
    HyperEdge(HyperEdgeId),
    /// A temporal reference to a node at a specific point in time.
    #[cfg(feature = "hypergraph")]
    TemporalRef(NodeId, i64),
}

#[cfg(feature = "subgraph")]
impl From<NodeId> for GraphEntity {
    fn from(id: NodeId) -> Self {
        GraphEntity::Node(id)
    }
}

/// Direction of a relationship traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Traversal follows outgoing edges from a node.
    Outgoing,
    /// Traversal follows incoming edges into a node.
    Incoming,
    /// Traversal follows edges in either direction.
    Both,
}

/// A node record stored in the Node B-tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    /// Unique identifier for this node.
    pub node_id: NodeId,
    /// Label IDs assigned to this node.
    pub labels: Vec<u32>,
    /// Key-value property pairs stored on this node.
    pub properties: Vec<(u32, PropertyValue)>,
    /// Head of the outgoing adjacency chain (None = no edges).
    pub next_edge_id: Option<EdgeId>,
    /// Overflow page for large property data.
    pub overflow_page: Option<PageId>,
}

/// A relationship record stored in the Edge B-tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipRecord {
    /// Unique identifier for this edge.
    pub edge_id: EdgeId,
    /// The node this edge originates from.
    pub start_node: NodeId,
    /// The node this edge points to.
    pub end_node: NodeId,
    /// Relationship type ID for this edge.
    pub rel_type_id: u32,
    /// Direction of this edge in the graph.
    pub direction: Direction,
    /// Next outgoing edge in the start node's adjacency chain.
    pub next_out_edge: Option<EdgeId>,
    /// Next incoming edge in the end node's adjacency chain.
    pub next_in_edge: Option<EdgeId>,
    /// Key-value property pairs stored on this edge.
    pub properties: Vec<(u32, PropertyValue)>,
    /// Whether the start endpoint is a subgraph (vs. a regular node).
    #[cfg(feature = "subgraph")]
    #[serde(default)]
    pub start_is_subgraph: bool,
    /// Whether the end endpoint is a subgraph (vs. a regular node).
    #[cfg(feature = "subgraph")]
    #[serde(default)]
    pub end_is_subgraph: bool,
}

#[cfg(feature = "subgraph")]
impl RelationshipRecord {
    /// Returns the start endpoint as a `GraphEntity`.
    pub fn start_entity(&self) -> GraphEntity {
        if self.start_is_subgraph {
            GraphEntity::Subgraph(SubgraphId(self.start_node.0))
        } else {
            GraphEntity::Node(self.start_node)
        }
    }

    /// Returns the end endpoint as a `GraphEntity`.
    pub fn end_entity(&self) -> GraphEntity {
        if self.end_is_subgraph {
            GraphEntity::Subgraph(SubgraphId(self.end_node.0))
        } else {
            GraphEntity::Node(self.end_node)
        }
    }

    /// Returns true if either endpoint is a subgraph.
    pub fn is_subgraph_edge(&self) -> bool {
        self.start_is_subgraph || self.end_is_subgraph
    }

    /// Construct a RelationshipRecord from GraphEntity endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn from_entities(
        edge_id: EdgeId,
        start: GraphEntity,
        end: GraphEntity,
        rel_type_id: u32,
        direction: Direction,
        next_out_edge: Option<EdgeId>,
        next_in_edge: Option<EdgeId>,
        properties: Vec<(u32, PropertyValue)>,
    ) -> Self {
        let (start_node, start_is_subgraph) = match start {
            GraphEntity::Node(id) => (id, false),
            GraphEntity::Subgraph(id) => (NodeId(id.0), true),
            #[cfg(feature = "hypergraph")]
            GraphEntity::HyperEdge(id) => (NodeId(id.0), false),
            #[cfg(feature = "hypergraph")]
            GraphEntity::TemporalRef(id, _) => (id, false),
        };
        let (end_node, end_is_subgraph) = match end {
            GraphEntity::Node(id) => (id, false),
            GraphEntity::Subgraph(id) => (NodeId(id.0), true),
            #[cfg(feature = "hypergraph")]
            GraphEntity::HyperEdge(id) => (NodeId(id.0), false),
            #[cfg(feature = "hypergraph")]
            GraphEntity::TemporalRef(id, _) => (id, false),
        };
        Self {
            edge_id,
            start_node,
            end_node,
            rel_type_id,
            direction,
            next_out_edge,
            next_in_edge,
            properties,
            start_is_subgraph,
            end_is_subgraph,
        }
    }
}

impl PropertyValue {
    /// Returns the type tag byte for this property value.
    pub fn type_tag(&self) -> u8 {
        match self {
            PropertyValue::Null => 0,
            PropertyValue::Bool(_) => 1,
            PropertyValue::Int64(_) => 2,
            PropertyValue::Float64(_) => 3,
            PropertyValue::String(_) => 4,
            PropertyValue::Bytes(_) => 5,
            PropertyValue::Array(_) => 6,
            PropertyValue::DateTime(_) => 7,
        }
    }
}

impl std::fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValue::Null => write!(f, "null"),
            PropertyValue::Bool(b) => write!(f, "{}", b),
            PropertyValue::Int64(i) => write!(f, "{}", i),
            PropertyValue::Float64(v) => write!(f, "{}", v),
            PropertyValue::String(s) => write!(f, "{}", s),
            PropertyValue::Bytes(b) => write!(f, "{:?}", b),
            PropertyValue::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            PropertyValue::DateTime(millis) => {
                write!(f, "{}", format_millis_as_iso8601(*millis))
            }
        }
    }
}

/// Format milliseconds since Unix epoch as ISO 8601 string.
fn format_millis_as_iso8601(millis: i64) -> String {
    let (total_secs, ms_part) = if millis >= 0 {
        (millis / 1000, (millis % 1000) as u32)
    } else {
        // For negative millis, floor division
        let s = (millis - 999) / 1000; // floor division for negative
        let m = (millis - s * 1000) as u32;
        (s, m)
    };

    // Convert seconds since epoch to date/time components
    let (year, month, day, hour, min, sec) = epoch_secs_to_datetime(total_secs);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hour, min, sec, ms_part
    )
}

/// Convert seconds since Unix epoch to (year, month, day, hour, minute, second).
fn epoch_secs_to_datetime(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    // Based on Howard Hinnant's algorithm for civil_from_days
    let mut remaining = secs;
    let hour;
    let minute;
    let second;

    if remaining >= 0 {
        second = (remaining % 60) as u32;
        remaining /= 60;
        minute = (remaining % 60) as u32;
        remaining /= 60;
        hour = (remaining % 24) as u32;
        let days = remaining / 24;
        let (y, m, d) = civil_from_days(days);
        (y, m, d, hour, minute, second)
    } else {
        // For negative epoch, compute from days
        // floor division for days
        let days = if remaining < 0 {
            (remaining - 86399) / 86400
        } else {
            remaining / 86400
        };
        let day_secs = remaining - days * 86400;
        hour = (day_secs / 3600) as u32;
        minute = ((day_secs % 3600) / 60) as u32;
        second = (day_secs % 60) as u32;
        let (y, m, d) = civil_from_days(days);
        (y, m, d, hour, minute, second)
    }
}

/// Convert days since 1970-01-01 to (year, month, day).
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-STORE-001: NodeId uniqueness via u64
    #[test]
    fn test_node_id_creation_and_equality() {
        let id1 = NodeId(1);
        let id2 = NodeId(1);
        let id3 = NodeId(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    // REQ-STORE-005: EdgeId uniqueness via u64
    #[test]
    fn test_edge_id_creation_and_equality() {
        let id1 = EdgeId(1);
        let id2 = EdgeId(1);
        let id3 = EdgeId(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_page_id_creation_and_ordering() {
        let p0 = PageId(0);
        let p1 = PageId(1);
        let p2 = PageId(2);
        assert!(p0 < p1);
        assert!(p1 < p2);
    }

    // REQ-STORE-011: All property types supported
    #[test]
    fn test_property_value_type_tags() {
        assert_eq!(PropertyValue::Null.type_tag(), 0);
        assert_eq!(PropertyValue::Bool(true).type_tag(), 1);
        assert_eq!(PropertyValue::Int64(42).type_tag(), 2);
        assert_eq!(PropertyValue::Float64(1.5_f64).type_tag(), 3);
        assert_eq!(PropertyValue::String("hello".into()).type_tag(), 4);
        assert_eq!(PropertyValue::Bytes(vec![1, 2, 3]).type_tag(), 5);
        assert_eq!(PropertyValue::Array(vec![]).type_tag(), 6);
    }

    // REQ-STORE-011: Nested arrays
    #[test]
    fn test_property_value_nested_array() {
        let nested = PropertyValue::Array(vec![
            PropertyValue::Int64(1),
            PropertyValue::Array(vec![PropertyValue::Bool(true)]),
        ]);
        assert_eq!(nested.type_tag(), 6);
    }

    #[test]
    fn test_node_record_creation() {
        let node = NodeRecord {
            node_id: NodeId(1),
            labels: vec![100, 200],
            properties: vec![
                (1, PropertyValue::String("Alice".into())),
                (2, PropertyValue::Int64(30)),
            ],
            next_edge_id: None,
            overflow_page: None,
        };
        assert_eq!(node.node_id, NodeId(1));
        assert_eq!(node.labels.len(), 2);
        assert_eq!(node.properties.len(), 2);
        assert!(node.next_edge_id.is_none());
    }

    #[test]
    fn test_node_record_with_adjacency_chain() {
        let node = NodeRecord {
            node_id: NodeId(1),
            labels: vec![],
            properties: vec![],
            next_edge_id: Some(EdgeId(10)),
            overflow_page: None,
        };
        assert_eq!(node.next_edge_id, Some(EdgeId(10)));
    }

    #[test]
    fn test_relationship_record_creation() {
        let edge = RelationshipRecord {
            edge_id: EdgeId(1),
            start_node: NodeId(10),
            end_node: NodeId(20),
            rel_type_id: 1,
            direction: Direction::Outgoing,
            next_out_edge: None,
            next_in_edge: None,
            properties: vec![],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        assert_eq!(edge.edge_id, EdgeId(1));
        assert_eq!(edge.start_node, NodeId(10));
        assert_eq!(edge.end_node, NodeId(20));
        assert_eq!(edge.direction, Direction::Outgoing);
    }

    // REQ-STORE-007: Adjacency chain pointers
    #[test]
    fn test_relationship_record_adjacency_chain() {
        let edge = RelationshipRecord {
            edge_id: EdgeId(1),
            start_node: NodeId(10),
            end_node: NodeId(20),
            rel_type_id: 1,
            direction: Direction::Outgoing,
            next_out_edge: Some(EdgeId(2)),
            next_in_edge: Some(EdgeId(3)),
            properties: vec![],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        assert_eq!(edge.next_out_edge, Some(EdgeId(2)));
        assert_eq!(edge.next_in_edge, Some(EdgeId(3)));
    }

    #[test]
    fn test_direction_variants() {
        assert_ne!(Direction::Outgoing, Direction::Incoming);
        assert_ne!(Direction::Incoming, Direction::Both);
        assert_ne!(Direction::Outgoing, Direction::Both);
    }

    // Serialization round-trip tests
    #[test]
    fn test_node_id_serialization_roundtrip() {
        let id = NodeId(42);
        let encoded = bincode::serialize(&id).expect("serialize");
        let decoded: NodeId = bincode::deserialize(&encoded).expect("deserialize");
        assert_eq!(id, decoded);
    }

    #[test]
    fn test_property_value_serialization_roundtrip() {
        let values = vec![
            PropertyValue::Null,
            PropertyValue::Bool(true),
            PropertyValue::Int64(-999),
            PropertyValue::Float64(2.5_f64),
            PropertyValue::String("test".into()),
            PropertyValue::Bytes(vec![0xDE, 0xAD]),
            PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null]),
        ];
        for val in &values {
            let encoded = bincode::serialize(val).expect("serialize");
            let decoded: PropertyValue = bincode::deserialize(&encoded).expect("deserialize");
            assert_eq!(val, &decoded);
        }
    }

    #[test]
    fn test_node_record_serialization_roundtrip() {
        let node = NodeRecord {
            node_id: NodeId(1),
            labels: vec![1, 2, 3],
            properties: vec![(1, PropertyValue::String("Alice".into()))],
            next_edge_id: Some(EdgeId(10)),
            overflow_page: Some(PageId(5)),
        };
        let encoded = bincode::serialize(&node).expect("serialize");
        let decoded: NodeRecord = bincode::deserialize(&encoded).expect("deserialize");
        assert_eq!(node, decoded);
    }

    #[test]
    fn test_relationship_record_serialization_roundtrip() {
        let edge = RelationshipRecord {
            edge_id: EdgeId(1),
            start_node: NodeId(10),
            end_node: NodeId(20),
            rel_type_id: 5,
            direction: Direction::Both,
            next_out_edge: Some(EdgeId(2)),
            next_in_edge: None,
            properties: vec![(1, PropertyValue::Bool(false))],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        let encoded = bincode::serialize(&edge).expect("serialize");
        let decoded: RelationshipRecord = bincode::deserialize(&encoded).expect("deserialize");
        assert_eq!(edge, decoded);
    }

    #[test]
    fn test_node_id_hash_works_in_collections() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(NodeId(1));
        set.insert(NodeId(2));
        set.insert(NodeId(1)); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_edge_id_ordering() {
        let mut ids = vec![EdgeId(5), EdgeId(1), EdgeId(3)];
        ids.sort();
        assert_eq!(ids, vec![EdgeId(1), EdgeId(3), EdgeId(5)]);
    }

    // ======================================================================
    // U-001: PropertyValue::DateTime(i64) variant
    // ======================================================================

    #[test]
    fn test_datetime_type_tag_is_7() {
        let dt = PropertyValue::DateTime(1_700_000_000_000);
        assert_eq!(dt.type_tag(), 7);
    }

    #[test]
    fn test_datetime_equality() {
        let a = PropertyValue::DateTime(1_700_000_000_000);
        let b = PropertyValue::DateTime(1_700_000_000_000);
        let c = PropertyValue::DateTime(0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_datetime_clone() {
        let a = PropertyValue::DateTime(1_700_000_000_000);
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_datetime_serialization_roundtrip() {
        let val = PropertyValue::DateTime(1_700_000_000_000);
        let encoded = bincode::serialize(&val).expect("serialize");
        let decoded: PropertyValue = bincode::deserialize(&encoded).expect("deserialize");
        assert_eq!(val, decoded);
    }

    #[test]
    fn test_datetime_does_not_break_existing_serialization() {
        // Ensure existing variants still serialize/deserialize correctly
        let values = vec![
            PropertyValue::Null,
            PropertyValue::Bool(true),
            PropertyValue::Int64(-999),
            PropertyValue::Float64(2.5_f64),
            PropertyValue::String("test".into()),
            PropertyValue::Bytes(vec![0xDE, 0xAD]),
            PropertyValue::Array(vec![PropertyValue::Int64(1), PropertyValue::Null]),
        ];
        for val in &values {
            let encoded = bincode::serialize(val).expect("serialize");
            let decoded: PropertyValue = bincode::deserialize(&encoded).expect("deserialize");
            assert_eq!(val, &decoded);
        }
    }

    // U-005: Display formatting
    #[test]
    fn test_datetime_display_iso8601() {
        // 2024-01-15T00:00:00.000Z = 1705276800000 ms since epoch
        let val = PropertyValue::DateTime(1_705_276_800_000);
        let display = format!("{}", val);
        assert_eq!(display, "2024-01-15T00:00:00.000Z");
    }

    #[test]
    fn test_datetime_display_epoch() {
        let val = PropertyValue::DateTime(0);
        let display = format!("{}", val);
        assert_eq!(display, "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn test_datetime_display_with_time() {
        // 2024-06-15T12:30:45.123Z
        // Calculate: 1718454645123 ms
        let val = PropertyValue::DateTime(1_718_454_645_123);
        let display = format!("{}", val);
        assert_eq!(display, "2024-06-15T12:30:45.123Z");
    }

    #[test]
    fn test_datetime_debug_includes_raw_millis() {
        let val = PropertyValue::DateTime(1_705_276_800_000);
        let debug = format!("{:?}", val);
        assert!(debug.contains("1705276800000"));
    }

    #[test]
    fn test_datetime_display_negative_epoch() {
        // Before Unix epoch: 1969-12-31T23:59:59.000Z = -1000 ms
        let val = PropertyValue::DateTime(-1000);
        let display = format!("{}", val);
        assert_eq!(display, "1969-12-31T23:59:59.000Z");
    }

    // ======================================================================
    // GG-001: SubgraphId newtype
    // ======================================================================

    #[cfg(feature = "subgraph")]
    mod subgraph_tests {
        use super::*;

        // GG-001: SubgraphId creation and equality
        #[test]
        fn test_subgraph_id_creation_and_equality() {
            let id1 = SubgraphId(1);
            let id2 = SubgraphId(1);
            let id3 = SubgraphId(2);
            assert_eq!(id1, id2);
            assert_ne!(id1, id3);
        }

        // GG-001: SubgraphId is Copy
        #[test]
        fn test_subgraph_id_is_copy() {
            let id = SubgraphId(42);
            let copied = id;
            assert_eq!(id, copied); // both usable after copy
        }

        // GG-001: SubgraphId ordering
        #[test]
        fn test_subgraph_id_ordering() {
            let mut ids = vec![SubgraphId(5), SubgraphId(1), SubgraphId(3)];
            ids.sort();
            assert_eq!(ids, vec![SubgraphId(1), SubgraphId(3), SubgraphId(5)]);
        }

        // GG-001: SubgraphId Hash (usable in HashSet)
        #[test]
        fn test_subgraph_id_hash() {
            use std::collections::HashSet;
            let mut set = HashSet::new();
            set.insert(SubgraphId(1));
            set.insert(SubgraphId(2));
            set.insert(SubgraphId(1)); // duplicate
            assert_eq!(set.len(), 2);
        }

        // GG-001: SubgraphId serialization roundtrip
        #[test]
        fn test_subgraph_id_serialization_roundtrip() {
            let id = SubgraphId(42);
            let encoded = bincode::serialize(&id).expect("serialize");
            let decoded: SubgraphId = bincode::deserialize(&encoded).expect("deserialize");
            assert_eq!(id, decoded);
        }

        // GG-001: SubgraphId Debug
        #[test]
        fn test_subgraph_id_debug() {
            let id = SubgraphId(99);
            let debug = format!("{:?}", id);
            assert!(debug.contains("99"));
        }

        // GG-002: SubgraphRecord construction
        #[test]
        fn test_subgraph_record_creation() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: None,
                properties: vec![],
            };
            assert_eq!(record.subgraph_id, SubgraphId(1));
            assert!(record.temporal_anchor.is_none());
            assert!(record.properties.is_empty());
        }

        // GG-002: SubgraphRecord with temporal anchor
        #[test]
        fn test_subgraph_record_with_temporal_anchor() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: Some(1_700_000_000_000),
                properties: vec![
                    (1, PropertyValue::String("test-graph".into())),
                ],
            };
            assert_eq!(record.temporal_anchor, Some(1_700_000_000_000));
            assert_eq!(record.properties.len(), 1);
        }

        // GG-002: SubgraphRecord equality
        #[test]
        fn test_subgraph_record_equality() {
            let a = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: None,
                properties: vec![(1, PropertyValue::Int64(42))],
            };
            let b = a.clone();
            assert_eq!(a, b);
        }

        // GG-002: SubgraphRecord Debug
        #[test]
        fn test_subgraph_record_debug() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: None,
                properties: vec![],
            };
            let debug = format!("{:?}", record);
            assert!(debug.contains("SubgraphRecord"));
        }

        // II-001: GraphEntity enum
        #[test]
        fn test_graph_entity_node_variant() {
            let entity = GraphEntity::Node(NodeId(42));
            assert_eq!(entity, GraphEntity::Node(NodeId(42)));
        }

        // II-001: GraphEntity subgraph variant
        #[test]
        fn test_graph_entity_subgraph_variant() {
            let entity = GraphEntity::Subgraph(SubgraphId(7));
            assert_eq!(entity, GraphEntity::Subgraph(SubgraphId(7)));
        }

        // II-001: GraphEntity From<NodeId>
        #[test]
        fn test_graph_entity_from_node_id() {
            let node_id = NodeId(10);
            let entity: GraphEntity = node_id.into();
            assert_eq!(entity, GraphEntity::Node(NodeId(10)));
        }

        // II-001: GraphEntity inequality between variants
        #[test]
        fn test_graph_entity_variant_inequality() {
            let node = GraphEntity::Node(NodeId(1));
            let subgraph = GraphEntity::Subgraph(SubgraphId(1));
            assert_ne!(node, subgraph);
        }

        // II-001: GraphEntity Clone
        #[test]
        fn test_graph_entity_clone() {
            let entity = GraphEntity::Node(NodeId(5));
            let cloned = entity.clone();
            assert_eq!(entity, cloned);
        }

        // II-001: GraphEntity Debug
        #[test]
        fn test_graph_entity_debug() {
            let entity = GraphEntity::Node(NodeId(1));
            let debug = format!("{:?}", entity);
            assert!(debug.contains("Node"));
        }

        // ======================================================================
        // II-002: RelationshipRecord GraphEntity extension
        // ======================================================================

        // II-002: RelationshipRecord has start_is_subgraph and end_is_subgraph fields
        #[test]
        fn test_relationship_record_subgraph_flags_default_false() {
            let edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(10),
                end_node: NodeId(20),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: false,
                end_is_subgraph: false,
            };
            assert!(!edge.start_is_subgraph);
            assert!(!edge.end_is_subgraph);
        }

        // II-002: start_entity() returns GraphEntity::Node when start_is_subgraph is false
        #[test]
        fn test_relationship_record_start_entity_node() {
            let edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(10),
                end_node: NodeId(20),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: false,
                end_is_subgraph: false,
            };
            assert_eq!(edge.start_entity(), GraphEntity::Node(NodeId(10)));
        }

        // II-002: start_entity() returns GraphEntity::Subgraph when start_is_subgraph is true
        #[test]
        fn test_relationship_record_start_entity_subgraph() {
            let edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(5), // raw u64 value, interpreted as SubgraphId
                end_node: NodeId(20),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: true,
                end_is_subgraph: false,
            };
            assert_eq!(edge.start_entity(), GraphEntity::Subgraph(SubgraphId(5)));
        }

        // II-002: end_entity() returns GraphEntity::Node when end_is_subgraph is false
        #[test]
        fn test_relationship_record_end_entity_node() {
            let edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(10),
                end_node: NodeId(20),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: false,
                end_is_subgraph: false,
            };
            assert_eq!(edge.end_entity(), GraphEntity::Node(NodeId(20)));
        }

        // II-002: end_entity() returns GraphEntity::Subgraph when end_is_subgraph is true
        #[test]
        fn test_relationship_record_end_entity_subgraph() {
            let edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(10),
                end_node: NodeId(7), // raw u64 value, interpreted as SubgraphId
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: false,
                end_is_subgraph: true,
            };
            assert_eq!(edge.end_entity(), GraphEntity::Subgraph(SubgraphId(7)));
        }

        // II-002: from_entities constructs a record with correct flags
        #[test]
        fn test_relationship_record_from_entities_node_to_node() {
            let edge = RelationshipRecord::from_entities(
                EdgeId(1),
                GraphEntity::Node(NodeId(10)),
                GraphEntity::Node(NodeId(20)),
                1,
                Direction::Outgoing,
                None,
                None,
                vec![],
            );
            assert_eq!(edge.start_node, NodeId(10));
            assert_eq!(edge.end_node, NodeId(20));
            assert!(!edge.start_is_subgraph);
            assert!(!edge.end_is_subgraph);
        }

        // II-003: from_entities with subgraph-to-subgraph
        #[test]
        fn test_relationship_record_from_entities_subgraph_to_subgraph() {
            let edge = RelationshipRecord::from_entities(
                EdgeId(2),
                GraphEntity::Subgraph(SubgraphId(5)),
                GraphEntity::Subgraph(SubgraphId(8)),
                2,
                Direction::Outgoing,
                None,
                None,
                vec![],
            );
            assert_eq!(edge.start_node, NodeId(5)); // same raw id
            assert_eq!(edge.end_node, NodeId(8));
            assert!(edge.start_is_subgraph);
            assert!(edge.end_is_subgraph);
        }

        // II-004: from_entities with mixed node-to-subgraph
        #[test]
        fn test_relationship_record_from_entities_node_to_subgraph() {
            let edge = RelationshipRecord::from_entities(
                EdgeId(3),
                GraphEntity::Node(NodeId(10)),
                GraphEntity::Subgraph(SubgraphId(7)),
                1,
                Direction::Outgoing,
                None,
                None,
                vec![],
            );
            assert_eq!(edge.start_node, NodeId(10));
            assert_eq!(edge.end_node, NodeId(7));
            assert!(!edge.start_is_subgraph);
            assert!(edge.end_is_subgraph);
        }

        // II-004: from_entities with mixed subgraph-to-node
        #[test]
        fn test_relationship_record_from_entities_subgraph_to_node() {
            let edge = RelationshipRecord::from_entities(
                EdgeId(4),
                GraphEntity::Subgraph(SubgraphId(3)),
                GraphEntity::Node(NodeId(20)),
                1,
                Direction::Outgoing,
                None,
                None,
                vec![],
            );
            assert_eq!(edge.start_node, NodeId(3));
            assert_eq!(edge.end_node, NodeId(20));
            assert!(edge.start_is_subgraph);
            assert!(!edge.end_is_subgraph);
        }

        // ======================================================================
        // HH-001: HyperEdgeId and HyperEdgeRecord tests
        // ======================================================================

        #[cfg(feature = "hypergraph")]
        mod hypergraph_tests {
            use super::*;

            // HH-001: HyperEdgeId creation and equality
            #[test]
            fn test_hyperedge_id_creation_and_equality() {
                let id1 = HyperEdgeId(1);
                let id2 = HyperEdgeId(1);
                let id3 = HyperEdgeId(2);
                assert_eq!(id1, id2);
                assert_ne!(id1, id3);
            }

            // HH-001: HyperEdgeId is Copy
            #[test]
            fn test_hyperedge_id_is_copy() {
                let id = HyperEdgeId(42);
                let copied = id;
                assert_eq!(id, copied);
            }

            // HH-001: HyperEdgeId ordering
            #[test]
            fn test_hyperedge_id_ordering() {
                let mut ids = vec![HyperEdgeId(5), HyperEdgeId(1), HyperEdgeId(3)];
                ids.sort();
                assert_eq!(ids, vec![HyperEdgeId(1), HyperEdgeId(3), HyperEdgeId(5)]);
            }

            // HH-001: HyperEdgeId Hash (usable in HashSet)
            #[test]
            fn test_hyperedge_id_hash() {
                use std::collections::HashSet;
                let mut set = HashSet::new();
                set.insert(HyperEdgeId(1));
                set.insert(HyperEdgeId(2));
                set.insert(HyperEdgeId(1)); // duplicate
                assert_eq!(set.len(), 2);
            }

            // HH-001: HyperEdgeId serialization roundtrip
            #[test]
            fn test_hyperedge_id_serialization_roundtrip() {
                let id = HyperEdgeId(42);
                let encoded = bincode::serialize(&id).expect("serialize");
                let decoded: HyperEdgeId = bincode::deserialize(&encoded).expect("deserialize");
                assert_eq!(id, decoded);
            }

            // HH-001: HyperEdgeId Debug
            #[test]
            fn test_hyperedge_id_debug() {
                let id = HyperEdgeId(99);
                let debug = format!("{:?}", id);
                assert!(debug.contains("99"));
            }

            // HH-002: HyperEdgeRecord creation
            #[test]
            fn test_hyperedge_record_creation() {
                let record = HyperEdgeRecord {
                    id: HyperEdgeId(1),
                    rel_type_id: 5,
                    sources: vec![GraphEntity::Node(NodeId(10))],
                    targets: vec![GraphEntity::Node(NodeId(20))],
                    properties: vec![],
                };
                assert_eq!(record.id, HyperEdgeId(1));
                assert_eq!(record.rel_type_id, 5);
                assert_eq!(record.sources.len(), 1);
                assert_eq!(record.targets.len(), 1);
                assert!(record.properties.is_empty());
            }

            // HH-002: HyperEdgeRecord with multiple sources and targets
            #[test]
            fn test_hyperedge_record_multi_sources_targets() {
                let record = HyperEdgeRecord {
                    id: HyperEdgeId(2),
                    rel_type_id: 3,
                    sources: vec![
                        GraphEntity::Node(NodeId(1)),
                        GraphEntity::Node(NodeId(2)),
                        GraphEntity::Node(NodeId(3)),
                    ],
                    targets: vec![
                        GraphEntity::Node(NodeId(10)),
                        GraphEntity::Subgraph(SubgraphId(1)),
                    ],
                    properties: vec![(1, PropertyValue::String("weight".into()))],
                };
                assert_eq!(record.sources.len(), 3);
                assert_eq!(record.targets.len(), 2);
                assert_eq!(record.properties.len(), 1);
            }

            // HH-002: HyperEdgeRecord clone and equality
            #[test]
            fn test_hyperedge_record_clone_equality() {
                let record = HyperEdgeRecord {
                    id: HyperEdgeId(1),
                    rel_type_id: 5,
                    sources: vec![GraphEntity::Node(NodeId(10))],
                    targets: vec![GraphEntity::Node(NodeId(20))],
                    properties: vec![(1, PropertyValue::Int64(42))],
                };
                let cloned = record.clone();
                assert_eq!(record, cloned);
            }

            // HH-002: HyperEdgeRecord Debug
            #[test]
            fn test_hyperedge_record_debug() {
                let record = HyperEdgeRecord {
                    id: HyperEdgeId(1),
                    rel_type_id: 0,
                    sources: vec![],
                    targets: vec![],
                    properties: vec![],
                };
                let debug = format!("{:?}", record);
                assert!(debug.contains("HyperEdgeRecord"));
            }

            // HH-003: GraphEntity HyperEdge variant
            #[test]
            fn test_graph_entity_hyperedge_variant() {
                let entity = GraphEntity::HyperEdge(HyperEdgeId(42));
                assert_eq!(entity, GraphEntity::HyperEdge(HyperEdgeId(42)));
                assert_ne!(entity, GraphEntity::Node(NodeId(42)));
            }

            // HH-003: GraphEntity TemporalRef variant
            #[test]
            fn test_graph_entity_temporal_ref_variant() {
                let entity = GraphEntity::TemporalRef(NodeId(10), 1_700_000_000_000);
                assert_eq!(
                    entity,
                    GraphEntity::TemporalRef(NodeId(10), 1_700_000_000_000)
                );
                assert_ne!(entity, GraphEntity::Node(NodeId(10)));
            }

            // HH-003: GraphEntity new variants are Clone + Debug
            #[test]
            fn test_graph_entity_hyperedge_clone_debug() {
                let entity = GraphEntity::HyperEdge(HyperEdgeId(5));
                let cloned = entity.clone();
                assert_eq!(entity, cloned);
                let debug = format!("{:?}", entity);
                assert!(debug.contains("HyperEdge"));
            }

            // HH-003: from_entities with HyperEdge returns error
            #[test]
            fn test_from_entities_with_hyperedge_uses_raw_id() {
                let edge = RelationshipRecord::from_entities(
                    EdgeId(1),
                    GraphEntity::HyperEdge(HyperEdgeId(99)),
                    GraphEntity::Node(NodeId(20)),
                    1,
                    Direction::Outgoing,
                    None,
                    None,
                    vec![],
                );
                // HyperEdge(99) maps to NodeId(99) with start_is_subgraph = false
                assert_eq!(edge.start_node, NodeId(99));
                assert!(!edge.start_is_subgraph);
            }

            // HH-003: from_entities with TemporalRef uses raw node id
            #[test]
            fn test_from_entities_with_temporal_ref_uses_raw_id() {
                let edge = RelationshipRecord::from_entities(
                    EdgeId(2),
                    GraphEntity::Node(NodeId(10)),
                    GraphEntity::TemporalRef(NodeId(30), 1_700_000_000_000),
                    1,
                    Direction::Outgoing,
                    None,
                    None,
                    vec![],
                );
                // TemporalRef(NodeId(30), _) maps to NodeId(30) with end_is_subgraph = false
                assert_eq!(edge.end_node, NodeId(30));
                assert!(!edge.end_is_subgraph);
            }
        }

        // II-002: is_subgraph_edge returns true when either endpoint is subgraph
        #[test]
        fn test_relationship_record_is_subgraph_edge() {
            let node_edge = RelationshipRecord {
                edge_id: EdgeId(1),
                start_node: NodeId(1),
                end_node: NodeId(2),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: false,
                end_is_subgraph: false,
            };
            assert!(!node_edge.is_subgraph_edge());

            let mixed_edge = RelationshipRecord {
                edge_id: EdgeId(2),
                start_node: NodeId(1),
                end_node: NodeId(2),
                rel_type_id: 1,
                direction: Direction::Outgoing,
                next_out_edge: None,
                next_in_edge: None,
                properties: vec![],
                start_is_subgraph: true,
                end_is_subgraph: false,
            };
            assert!(mixed_edge.is_subgraph_edge());
        }
    }
}
