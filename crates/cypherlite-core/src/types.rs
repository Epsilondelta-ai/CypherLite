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
}

/// Direction of a relationship traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

/// A node record stored in the Node B-tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_id: NodeId,
    pub labels: Vec<u32>,
    pub properties: Vec<(u32, PropertyValue)>,
    /// Head of the outgoing adjacency chain (None = no edges).
    pub next_edge_id: Option<EdgeId>,
    /// Overflow page for large property data.
    pub overflow_page: Option<PageId>,
}

/// A relationship record stored in the Edge B-tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipRecord {
    pub edge_id: EdgeId,
    pub start_node: NodeId,
    pub end_node: NodeId,
    pub rel_type_id: u32,
    pub direction: Direction,
    /// Next outgoing edge in the start node's adjacency chain.
    pub next_out_edge: Option<EdgeId>,
    /// Next incoming edge in the end node's adjacency chain.
    pub next_in_edge: Option<EdgeId>,
    pub properties: Vec<(u32, PropertyValue)>,
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
        }
    }
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
}
