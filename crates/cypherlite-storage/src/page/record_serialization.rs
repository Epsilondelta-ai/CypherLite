// Record serialization for data page persistence (SPEC-PERSIST-001)
//
// Provides binary serialization for NodeRecord, RelationshipRecord,
// SubgraphRecord, HyperEdgeRecord, and VersionRecord,
// plus a DataPageHeader for slotted data pages.

use cypherlite_core::{Direction, EdgeId, NodeId, NodeRecord, RelationshipRecord};
#[cfg(feature = "subgraph")]
use cypherlite_core::{SubgraphId, SubgraphRecord};
#[cfg(feature = "hypergraph")]
use cypherlite_core::{GraphEntity, HyperEdgeId, HyperEdgeRecord};
use crate::version::VersionRecord;

use super::PAGE_SIZE;
use crate::btree::property_store::PropertyStore;

/// 12-byte header at the start of each data page (node/edge/catalog).
///
/// Layout:
///   page_type:    u8   (1 byte)
///   record_count: u16  (2 bytes, LE)
///   free_offset:  u16  (2 bytes, LE)
///   next_page:    u32  (4 bytes, LE)
///   _reserved:    [u8; 3]  (padding to 12 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataPageHeader {
    /// Page type discriminant (6=NodeData, 7=EdgeData, 8=CatalogData).
    pub page_type: u8,
    /// Number of records stored in this page.
    pub record_count: u16,
    /// Byte offset where the next free space begins.
    pub free_offset: u16,
    /// Next overflow/continuation page (0 = none).
    pub next_page: u32,
}

impl DataPageHeader {
    /// Size of the data page header in bytes.
    pub const SIZE: usize = 12;

    /// Create a new data page header with the given type.
    pub fn new(page_type: u8) -> Self {
        Self {
            page_type,
            record_count: 0,
            free_offset: Self::SIZE as u16,
            next_page: 0,
        }
    }

    /// Serialize data page header into the first 12 bytes of a buffer.
    pub fn write_to(&self, buf: &mut [u8]) {
        debug_assert!(buf.len() >= Self::SIZE);
        buf[0] = self.page_type;
        buf[1..3].copy_from_slice(&self.record_count.to_le_bytes());
        buf[3..5].copy_from_slice(&self.free_offset.to_le_bytes());
        buf[5..9].copy_from_slice(&self.next_page.to_le_bytes());
        buf[9..12].copy_from_slice(&[0u8; 3]); // reserved
    }

    /// Deserialize data page header from the first 12 bytes of a buffer.
    pub fn read_from(buf: &[u8]) -> Self {
        debug_assert!(buf.len() >= Self::SIZE);
        Self {
            page_type: buf[0],
            record_count: u16::from_le_bytes([buf[1], buf[2]]),
            free_offset: u16::from_le_bytes([buf[3], buf[4]]),
            next_page: u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]),
        }
    }

    /// Returns the number of usable bytes remaining in a 4096-byte page.
    pub fn remaining_space(&self) -> usize {
        PAGE_SIZE.saturating_sub(self.free_offset as usize)
    }
}

/// Serialize a `NodeRecord` to its on-disk binary format.
///
/// Layout:
///   node_id:      u64  (8 bytes, LE)
///   flags:        u8   (1 byte, 0x01 = deleted)
///   label_count:  u16  (2 bytes, LE)
///   labels:       [u32; label_count]  (4 bytes each, LE)
///   prop_count:   u16  (2 bytes, LE)
///   properties:   repeated [key_id(u32) + type_tag(u8) + value_bytes]
pub fn serialize_node_record(record: &NodeRecord, deleted: bool) -> Vec<u8> {
    let mut buf = Vec::new();

    // node_id: u64 LE
    buf.extend_from_slice(&record.node_id.0.to_le_bytes());

    // flags: u8
    let flags: u8 = if deleted { 0x01 } else { 0x00 };
    buf.push(flags);

    // label_count: u16 LE
    let label_count = record.labels.len() as u16;
    buf.extend_from_slice(&label_count.to_le_bytes());

    // labels: [u32; N]
    for label in &record.labels {
        buf.extend_from_slice(&label.to_le_bytes());
    }

    // prop_count: u16 LE
    let prop_count = record.properties.len() as u16;
    buf.extend_from_slice(&prop_count.to_le_bytes());

    // properties: repeated [key_id(u32) + type_tag(u8) + value]
    for (key_id, value) in &record.properties {
        let prop_bytes = PropertyStore::serialize_property(*key_id, value);
        // Write length prefix so we can skip over variable-length values
        let len = prop_bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&prop_bytes);
    }

    buf
}

/// Deserialize a `NodeRecord` from bytes.
///
/// Returns `(NodeRecord, is_deleted, bytes_consumed)` on success.
pub fn deserialize_node_record(data: &[u8]) -> Option<(NodeRecord, bool, usize)> {
    let mut offset = 0;

    // node_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let node_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // flags: u8
    if data.len() < offset + 1 {
        return None;
    }
    let flags = data[offset];
    let deleted = (flags & 0x01) != 0;
    offset += 1;

    // label_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let label_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // labels: [u32; label_count]
    let labels_size = label_count * 4;
    if data.len() < offset + labels_size {
        return None;
    }
    let mut labels = Vec::with_capacity(label_count);
    for _ in 0..label_count {
        let label = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        labels.push(label);
        offset += 4;
    }

    // prop_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let prop_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // properties: repeated [len(u16) + key_id(u32) + type_tag(u8) + value]
    let mut properties = Vec::with_capacity(prop_count);
    for _ in 0..prop_count {
        if data.len() < offset + 2 {
            return None;
        }
        let prop_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        offset += 2;

        if data.len() < offset + prop_len {
            return None;
        }
        let (key_id, value, _consumed) =
            PropertyStore::deserialize_property(&data[offset..offset + prop_len])?;
        properties.push((key_id, value));
        offset += prop_len;
    }

    let record = NodeRecord {
        node_id: NodeId(node_id),
        labels,
        properties,
        next_edge_id: None,
        overflow_page: None,
    };

    Some((record, deleted, offset))
}

/// Serialize a `RelationshipRecord` to its on-disk binary format.
///
/// Layout:
///   edge_id:      u64  (8 bytes, LE)
///   source_id:    u64  (8 bytes, LE)
///   target_id:    u64  (8 bytes, LE)
///   rel_type_id:  u32  (4 bytes, LE)
///   flags:        u8   (1 byte, 0x01 = deleted)
///   prop_count:   u16  (2 bytes, LE)
///   properties:   repeated [len(u16) + key_id(u32) + type_tag(u8) + value]
pub fn serialize_edge_record(record: &RelationshipRecord, deleted: bool) -> Vec<u8> {
    let mut buf = Vec::new();

    // edge_id: u64 LE
    buf.extend_from_slice(&record.edge_id.0.to_le_bytes());

    // source_id: u64 LE
    buf.extend_from_slice(&record.start_node.0.to_le_bytes());

    // target_id: u64 LE
    buf.extend_from_slice(&record.end_node.0.to_le_bytes());

    // rel_type_id: u32 LE
    buf.extend_from_slice(&record.rel_type_id.to_le_bytes());

    // flags: u8
    let flags: u8 = if deleted { 0x01 } else { 0x00 };
    buf.push(flags);

    // prop_count: u16 LE
    let prop_count = record.properties.len() as u16;
    buf.extend_from_slice(&prop_count.to_le_bytes());

    // properties
    for (key_id, value) in &record.properties {
        let prop_bytes = PropertyStore::serialize_property(*key_id, value);
        let len = prop_bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&prop_bytes);
    }

    buf
}

/// Deserialize a `RelationshipRecord` from bytes.
///
/// Returns `(RelationshipRecord, is_deleted, bytes_consumed)` on success.
pub fn deserialize_edge_record(data: &[u8]) -> Option<(RelationshipRecord, bool, usize)> {
    let mut offset = 0;

    // edge_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let edge_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // source_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let source_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // target_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let target_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // rel_type_id: u32
    if data.len() < offset + 4 {
        return None;
    }
    let rel_type_id = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
    offset += 4;

    // flags: u8
    if data.len() < offset + 1 {
        return None;
    }
    let flags = data[offset];
    let deleted = (flags & 0x01) != 0;
    offset += 1;

    // prop_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let prop_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // properties
    let mut properties = Vec::with_capacity(prop_count);
    for _ in 0..prop_count {
        if data.len() < offset + 2 {
            return None;
        }
        let prop_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        offset += 2;

        if data.len() < offset + prop_len {
            return None;
        }
        let (key_id, value, _consumed) =
            PropertyStore::deserialize_property(&data[offset..offset + prop_len])?;
        properties.push((key_id, value));
        offset += prop_len;
    }

    let record = RelationshipRecord {
        edge_id: EdgeId(edge_id),
        start_node: NodeId(source_id),
        end_node: NodeId(target_id),
        rel_type_id,
        direction: Direction::Outgoing,
        next_out_edge: None,
        next_in_edge: None,
        properties,
        #[cfg(feature = "subgraph")]
        start_is_subgraph: false,
        #[cfg(feature = "subgraph")]
        end_is_subgraph: false,
    };

    Some((record, deleted, offset))
}

/// Try to pack a serialized record into a data page.
///
/// Returns `true` if the record was successfully added.
/// The caller provides the full 4096-byte page buffer and the serialized record bytes.
pub fn pack_record_into_page(page: &mut [u8; PAGE_SIZE], record_bytes: &[u8]) -> bool {
    let mut header = DataPageHeader::read_from(page);
    let record_size = record_bytes.len();

    // Check if there is enough space (record + 2-byte length prefix)
    let total_needed = 2 + record_size;
    if header.remaining_space() < total_needed {
        return false;
    }

    let write_offset = header.free_offset as usize;

    // Write length prefix (u16 LE)
    page[write_offset..write_offset + 2].copy_from_slice(&(record_size as u16).to_le_bytes());
    // Write record bytes
    page[write_offset + 2..write_offset + 2 + record_size].copy_from_slice(record_bytes);

    // Update header
    header.record_count += 1;
    header.free_offset += total_needed as u16;
    header.write_to(page);

    true
}

/// Read all records from a data page as raw byte slices.
///
/// Returns a vector of `(offset, length)` pairs into the page buffer.
pub fn read_records_from_page(page: &[u8; PAGE_SIZE]) -> Vec<(usize, usize)> {
    let header = DataPageHeader::read_from(page);
    let mut results = Vec::with_capacity(header.record_count as usize);
    let mut offset = DataPageHeader::SIZE;

    for _ in 0..header.record_count {
        if offset + 2 > PAGE_SIZE {
            break;
        }
        let record_len =
            u16::from_le_bytes([page[offset], page[offset + 1]]) as usize;
        offset += 2;

        if offset + record_len > PAGE_SIZE {
            break;
        }
        results.push((offset, record_len));
        offset += record_len;
    }

    results
}

// ================================================================
// PERSIST-001 Phase 5: SubgraphRecord serialization
// ================================================================

/// Serialize a `SubgraphRecord` with its membership list to on-disk binary format.
///
/// Layout:
///   subgraph_id:      u64  (8 bytes, LE)
///   flags:            u8   (1 byte, 0x01 = deleted)
///   has_anchor:       u8   (0 = no, 1 = yes)
///   temporal_anchor:  i64  (8 bytes, LE, present only if has_anchor == 1)
///   prop_count:       u16  (2 bytes, LE)
///   properties:       repeated [len(u16) + key_id(u32) + type_tag(u8) + value]
///   member_count:     u16  (2 bytes, LE)
///   members:          [u64; member_count]  (8 bytes each, LE) -- node IDs
#[cfg(feature = "subgraph")]
pub fn serialize_subgraph_record(
    record: &SubgraphRecord,
    members: &[NodeId],
    deleted: bool,
) -> Vec<u8> {
    let mut buf = Vec::new();

    // subgraph_id: u64 LE
    buf.extend_from_slice(&record.subgraph_id.0.to_le_bytes());

    // flags: u8
    let flags: u8 = if deleted { 0x01 } else { 0x00 };
    buf.push(flags);

    // has_anchor: u8, temporal_anchor: i64 (conditional)
    match record.temporal_anchor {
        Some(anchor) => {
            buf.push(1u8);
            buf.extend_from_slice(&anchor.to_le_bytes());
        }
        None => {
            buf.push(0u8);
        }
    }

    // prop_count: u16 LE
    let prop_count = record.properties.len() as u16;
    buf.extend_from_slice(&prop_count.to_le_bytes());

    // properties
    for (key_id, value) in &record.properties {
        let prop_bytes = PropertyStore::serialize_property(*key_id, value);
        let len = prop_bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&prop_bytes);
    }

    // member_count: u16 LE
    let member_count = members.len() as u16;
    buf.extend_from_slice(&member_count.to_le_bytes());

    // members: [u64; N]
    for node_id in members {
        buf.extend_from_slice(&node_id.0.to_le_bytes());
    }

    buf
}

/// Deserialize a `SubgraphRecord` with membership list from bytes.
///
/// Returns `(SubgraphRecord, member_node_ids, is_deleted, bytes_consumed)`.
#[cfg(feature = "subgraph")]
pub fn deserialize_subgraph_record(
    data: &[u8],
) -> Option<(SubgraphRecord, Vec<NodeId>, bool, usize)> {
    let mut offset = 0;

    // subgraph_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let subgraph_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // flags: u8
    if data.len() < offset + 1 {
        return None;
    }
    let flags = data[offset];
    let deleted = (flags & 0x01) != 0;
    offset += 1;

    // has_anchor: u8
    if data.len() < offset + 1 {
        return None;
    }
    let has_anchor = data[offset];
    offset += 1;

    let temporal_anchor = if has_anchor == 1 {
        if data.len() < offset + 8 {
            return None;
        }
        let anchor = i64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        Some(anchor)
    } else {
        None
    };

    // prop_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let prop_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // properties
    let mut properties = Vec::with_capacity(prop_count);
    for _ in 0..prop_count {
        if data.len() < offset + 2 {
            return None;
        }
        let prop_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        offset += 2;

        if data.len() < offset + prop_len {
            return None;
        }
        let (key_id, value, _consumed) =
            PropertyStore::deserialize_property(&data[offset..offset + prop_len])?;
        properties.push((key_id, value));
        offset += prop_len;
    }

    // member_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let member_count =
        u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // members: [u64; N]
    let mut members = Vec::with_capacity(member_count);
    for _ in 0..member_count {
        if data.len() < offset + 8 {
            return None;
        }
        let nid = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        members.push(NodeId(nid));
        offset += 8;
    }

    let record = SubgraphRecord {
        subgraph_id: SubgraphId(subgraph_id),
        temporal_anchor,
        properties,
    };

    Some((record, members, deleted, offset))
}

// ================================================================
// PERSIST-001 Phase 5: HyperEdgeRecord serialization
// ================================================================

/// Serialize a `GraphEntity` to bytes.
///
/// Layout: tag(u8) + payload
///   0 = Node(u64)
///   1 = Subgraph(u64)
///   2 = HyperEdge(u64)
///   3 = TemporalRef(u64, i64)
#[cfg(feature = "hypergraph")]
fn serialize_graph_entity(entity: &GraphEntity) -> Vec<u8> {
    let mut buf = Vec::new();
    match entity {
        GraphEntity::Node(nid) => {
            buf.push(0u8);
            buf.extend_from_slice(&nid.0.to_le_bytes());
        }
        GraphEntity::Subgraph(sid) => {
            buf.push(1u8);
            buf.extend_from_slice(&sid.0.to_le_bytes());
        }
        #[cfg(feature = "hypergraph")]
        GraphEntity::HyperEdge(hid) => {
            buf.push(2u8);
            buf.extend_from_slice(&hid.0.to_le_bytes());
        }
        #[cfg(feature = "hypergraph")]
        GraphEntity::TemporalRef(nid, ts) => {
            buf.push(3u8);
            buf.extend_from_slice(&nid.0.to_le_bytes());
            buf.extend_from_slice(&ts.to_le_bytes());
        }
    }
    buf
}

/// Deserialize a `GraphEntity` from bytes.
/// Returns `(entity, bytes_consumed)`.
#[cfg(feature = "hypergraph")]
fn deserialize_graph_entity(data: &[u8]) -> Option<(GraphEntity, usize)> {
    if data.is_empty() {
        return None;
    }
    let tag = data[0];
    match tag {
        0 => {
            // Node(u64)
            if data.len() < 9 {
                return None;
            }
            let nid = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some((GraphEntity::Node(NodeId(nid)), 9))
        }
        1 => {
            // Subgraph(u64)
            if data.len() < 9 {
                return None;
            }
            let sid = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some((
                GraphEntity::Subgraph(cypherlite_core::SubgraphId(sid)),
                9,
            ))
        }
        2 => {
            // HyperEdge(u64)
            if data.len() < 9 {
                return None;
            }
            let hid = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some((GraphEntity::HyperEdge(HyperEdgeId(hid)), 9))
        }
        3 => {
            // TemporalRef(u64, i64)
            if data.len() < 17 {
                return None;
            }
            let nid = u64::from_le_bytes(data[1..9].try_into().ok()?);
            let ts = i64::from_le_bytes(data[9..17].try_into().ok()?);
            Some((GraphEntity::TemporalRef(NodeId(nid), ts), 17))
        }
        _ => None,
    }
}

/// Serialize a `HyperEdgeRecord` to on-disk binary format.
///
/// Layout:
///   id:           u64  (8 bytes, LE)
///   flags:        u8   (1 byte, 0x01 = deleted)
///   rel_type_id:  u32  (4 bytes, LE)
///   src_count:    u16  (2 bytes, LE)
///   sources:      [GraphEntity; src_count]
///   tgt_count:    u16  (2 bytes, LE)
///   targets:      [GraphEntity; tgt_count]
///   prop_count:   u16  (2 bytes, LE)
///   properties:   repeated [len(u16) + key_id(u32) + type_tag(u8) + value]
#[cfg(feature = "hypergraph")]
pub fn serialize_hyperedge_record(record: &HyperEdgeRecord, deleted: bool) -> Vec<u8> {
    let mut buf = Vec::new();

    // id: u64 LE
    buf.extend_from_slice(&record.id.0.to_le_bytes());

    // flags: u8
    let flags: u8 = if deleted { 0x01 } else { 0x00 };
    buf.push(flags);

    // rel_type_id: u32 LE
    buf.extend_from_slice(&record.rel_type_id.to_le_bytes());

    // src_count: u16 LE + sources
    let src_count = record.sources.len() as u16;
    buf.extend_from_slice(&src_count.to_le_bytes());
    for entity in &record.sources {
        buf.extend_from_slice(&serialize_graph_entity(entity));
    }

    // tgt_count: u16 LE + targets
    let tgt_count = record.targets.len() as u16;
    buf.extend_from_slice(&tgt_count.to_le_bytes());
    for entity in &record.targets {
        buf.extend_from_slice(&serialize_graph_entity(entity));
    }

    // prop_count: u16 LE
    let prop_count = record.properties.len() as u16;
    buf.extend_from_slice(&prop_count.to_le_bytes());

    // properties
    for (key_id, value) in &record.properties {
        let prop_bytes = PropertyStore::serialize_property(*key_id, value);
        let len = prop_bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&prop_bytes);
    }

    buf
}

/// Deserialize a `HyperEdgeRecord` from bytes.
///
/// Returns `(HyperEdgeRecord, is_deleted, bytes_consumed)`.
#[cfg(feature = "hypergraph")]
pub fn deserialize_hyperedge_record(data: &[u8]) -> Option<(HyperEdgeRecord, bool, usize)> {
    let mut offset = 0;

    // id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // flags: u8
    if data.len() < offset + 1 {
        return None;
    }
    let flags = data[offset];
    let deleted = (flags & 0x01) != 0;
    offset += 1;

    // rel_type_id: u32
    if data.len() < offset + 4 {
        return None;
    }
    let rel_type_id = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
    offset += 4;

    // src_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let src_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // sources
    let mut sources = Vec::with_capacity(src_count);
    for _ in 0..src_count {
        let (entity, consumed) = deserialize_graph_entity(&data[offset..])?;
        sources.push(entity);
        offset += consumed;
    }

    // tgt_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let tgt_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // targets
    let mut targets = Vec::with_capacity(tgt_count);
    for _ in 0..tgt_count {
        let (entity, consumed) = deserialize_graph_entity(&data[offset..])?;
        targets.push(entity);
        offset += consumed;
    }

    // prop_count: u16
    if data.len() < offset + 2 {
        return None;
    }
    let prop_count = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    // properties
    let mut properties = Vec::with_capacity(prop_count);
    for _ in 0..prop_count {
        if data.len() < offset + 2 {
            return None;
        }
        let prop_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
        offset += 2;

        if data.len() < offset + prop_len {
            return None;
        }
        let (key_id, value, _consumed) =
            PropertyStore::deserialize_property(&data[offset..offset + prop_len])?;
        properties.push((key_id, value));
        offset += prop_len;
    }

    let record = HyperEdgeRecord {
        id: HyperEdgeId(id),
        rel_type_id,
        sources,
        targets,
        properties,
    };

    Some((record, deleted, offset))
}

// ================================================================
// PERSIST-001 Phase 5: VersionRecord serialization
// ================================================================

/// Serialize a `VersionRecord` with its composite key to on-disk binary format.
///
/// Layout:
///   entity_id:    u64  (8 bytes, LE)
///   version_seq:  u64  (8 bytes, LE)
///   tag:          u8   (0 = Node, 1 = Relationship)
///   record_len:   u16  (2 bytes, LE)
///   record_data:  [u8; record_len]  (serialized node or edge record)
pub fn serialize_version_record(
    entity_id: u64,
    version_seq: u64,
    record: &VersionRecord,
) -> Vec<u8> {
    let mut buf = Vec::new();

    // entity_id: u64 LE
    buf.extend_from_slice(&entity_id.to_le_bytes());

    // version_seq: u64 LE
    buf.extend_from_slice(&version_seq.to_le_bytes());

    match record {
        VersionRecord::Node(node) => {
            buf.push(0u8); // tag = Node
            let record_bytes = serialize_node_record(node, false);
            let len = record_bytes.len() as u16;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(&record_bytes);
        }
        VersionRecord::Relationship(edge) => {
            buf.push(1u8); // tag = Relationship
            let record_bytes = serialize_edge_record(edge, false);
            let len = record_bytes.len() as u16;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(&record_bytes);
        }
    }

    buf
}

/// Deserialize a `VersionRecord` with its composite key from bytes.
///
/// Returns `(entity_id, version_seq, VersionRecord, bytes_consumed)`.
pub fn deserialize_version_record(
    data: &[u8],
) -> Option<(u64, u64, VersionRecord, usize)> {
    let mut offset = 0;

    // entity_id: u64
    if data.len() < offset + 8 {
        return None;
    }
    let entity_id = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // version_seq: u64
    if data.len() < offset + 8 {
        return None;
    }
    let version_seq = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
    offset += 8;

    // tag: u8
    if data.len() < offset + 1 {
        return None;
    }
    let tag = data[offset];
    offset += 1;

    // record_len: u16
    if data.len() < offset + 2 {
        return None;
    }
    let record_len = u16::from_le_bytes(data[offset..offset + 2].try_into().ok()?) as usize;
    offset += 2;

    if data.len() < offset + record_len {
        return None;
    }

    let record = match tag {
        0 => {
            // Node
            let (node, _deleted, _consumed) =
                deserialize_node_record(&data[offset..offset + record_len])?;
            VersionRecord::Node(node)
        }
        1 => {
            // Relationship
            let (edge, _deleted, _consumed) =
                deserialize_edge_record(&data[offset..offset + record_len])?;
            VersionRecord::Relationship(edge)
        }
        _ => return None,
    };
    offset += record_len;

    Some((entity_id, version_seq, record, offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::PageType;
    use cypherlite_core::PropertyValue;

    // ================================================================
    // TASK-008: PageType extension tests
    // ================================================================

    #[test]
    fn test_page_type_node_data_is_6() {
        assert_eq!(PageType::NodeData as u8, 6);
    }

    #[test]
    fn test_page_type_edge_data_is_7() {
        assert_eq!(PageType::EdgeData as u8, 7);
    }

    #[test]
    fn test_page_type_catalog_data_is_8() {
        assert_eq!(PageType::CatalogData as u8, 8);
    }

    // ================================================================
    // TASK-010: DatabaseHeader extension tests
    // ================================================================

    #[test]
    fn test_database_header_new_has_persist_fields() {
        use crate::page::DatabaseHeader;
        let hdr = DatabaseHeader::new();
        assert_eq!(hdr.catalog_page_id, 0);
        assert_eq!(hdr.node_data_root_page, 0);
        assert_eq!(hdr.edge_data_root_page, 0);
    }

    #[test]
    fn test_database_header_persist_fields_roundtrip() {
        use crate::page::DatabaseHeader;
        let hdr = DatabaseHeader {
            catalog_page_id: 42,
            node_data_root_page: 100,
            edge_data_root_page: 200,
            ..DatabaseHeader::new()
        };
        let page = hdr.to_page();
        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.catalog_page_id, 42);
        assert_eq!(decoded.node_data_root_page, 100);
        assert_eq!(decoded.edge_data_root_page, 200);
    }

    #[test]
    fn test_database_header_persist_fields_v_migration() {
        use crate::page::{DatabaseHeader, MAGIC, FIRST_DATA_PAGE, FORMAT_VERSION};
        // Simulate opening a database from the previous format version
        // (no persist fields) -- they should default to 0
        let mut page = [0u8; PAGE_SIZE];
        page[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // Use current FORMAT_VERSION minus 1 so migration kicks in
        let old_version = FORMAT_VERSION - 1;
        page[4..8].copy_from_slice(&old_version.to_le_bytes());
        page[8..12].copy_from_slice(&FIRST_DATA_PAGE.to_le_bytes());
        page[20..28].copy_from_slice(&1u64.to_le_bytes());
        page[28..36].copy_from_slice(&1u64.to_le_bytes());
        // feature_flags
        page[44..48].copy_from_slice(&DatabaseHeader::compiled_feature_flags().to_le_bytes());

        let decoded = DatabaseHeader::from_page(&page);
        assert_eq!(decoded.catalog_page_id, 0);
        assert_eq!(decoded.node_data_root_page, 0);
        assert_eq!(decoded.edge_data_root_page, 0);
    }

    // ================================================================
    // TASK-016: DataPageHeader tests
    // ================================================================

    #[test]
    fn test_data_page_header_size_is_12() {
        assert_eq!(DataPageHeader::SIZE, 12);
    }

    #[test]
    fn test_data_page_header_new() {
        let hdr = DataPageHeader::new(6); // NodeData
        assert_eq!(hdr.page_type, 6);
        assert_eq!(hdr.record_count, 0);
        assert_eq!(hdr.free_offset, 12);
        assert_eq!(hdr.next_page, 0);
    }

    #[test]
    fn test_data_page_header_roundtrip() {
        let hdr = DataPageHeader {
            page_type: 7,
            record_count: 42,
            free_offset: 1024,
            next_page: 99,
        };
        let mut buf = [0u8; 12];
        hdr.write_to(&mut buf);
        let decoded = DataPageHeader::read_from(&buf);
        assert_eq!(hdr, decoded);
    }

    #[test]
    fn test_data_page_header_remaining_space() {
        let hdr = DataPageHeader::new(6);
        assert_eq!(hdr.remaining_space(), PAGE_SIZE - DataPageHeader::SIZE);
    }

    // ================================================================
    // TASK-012: NodeRecord serialization tests
    // ================================================================

    #[test]
    fn test_node_record_roundtrip_empty() {
        let record = NodeRecord {
            node_id: NodeId(1),
            labels: vec![],
            properties: vec![],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        let (decoded, deleted, consumed) = deserialize_node_record(&bytes).expect("deserialize");
        assert_eq!(decoded.node_id, record.node_id);
        assert_eq!(decoded.labels, record.labels);
        assert_eq!(decoded.properties, record.properties);
        assert!(!deleted);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn test_node_record_roundtrip_with_labels() {
        let record = NodeRecord {
            node_id: NodeId(42),
            labels: vec![1, 2, 3],
            properties: vec![],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        let (decoded, _, _) = deserialize_node_record(&bytes).expect("deserialize");
        assert_eq!(decoded.labels, vec![1, 2, 3]);
    }

    #[test]
    fn test_node_record_roundtrip_with_properties() {
        let record = NodeRecord {
            node_id: NodeId(7),
            labels: vec![],
            properties: vec![
                (1, PropertyValue::String("Alice".into())),
                (2, PropertyValue::Int64(30)),
                (3, PropertyValue::Bool(true)),
            ],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        let (decoded, _, _) = deserialize_node_record(&bytes).expect("deserialize");
        assert_eq!(decoded.properties, record.properties);
    }

    #[test]
    fn test_node_record_roundtrip_with_labels_and_properties() {
        let record = NodeRecord {
            node_id: NodeId(99),
            labels: vec![10, 20],
            properties: vec![
                (5, PropertyValue::Float64(1.5)),
                (6, PropertyValue::Null),
            ],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        let (decoded, deleted, consumed) = deserialize_node_record(&bytes).expect("deserialize");
        assert_eq!(decoded.node_id, NodeId(99));
        assert_eq!(decoded.labels, vec![10, 20]);
        assert_eq!(decoded.properties, record.properties);
        assert!(!deleted);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn test_node_record_deleted_flag() {
        let record = NodeRecord {
            node_id: NodeId(1),
            labels: vec![],
            properties: vec![],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, true);
        let (_, deleted, _) = deserialize_node_record(&bytes).expect("deserialize");
        assert!(deleted);
    }

    #[test]
    fn test_node_record_deserialize_truncated() {
        assert!(deserialize_node_record(&[0u8; 3]).is_none());
    }

    // ================================================================
    // TASK-014: RelationshipRecord serialization tests
    // ================================================================

    #[test]
    fn test_edge_record_roundtrip_no_props() {
        let record = RelationshipRecord {
            edge_id: EdgeId(1),
            start_node: NodeId(10),
            end_node: NodeId(20),
            rel_type_id: 5,
            direction: Direction::Outgoing,
            next_out_edge: None,
            next_in_edge: None,
            properties: vec![],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        let bytes = serialize_edge_record(&record, false);
        let (decoded, deleted, consumed) = deserialize_edge_record(&bytes).expect("deserialize");
        assert_eq!(decoded.edge_id, EdgeId(1));
        assert_eq!(decoded.start_node, NodeId(10));
        assert_eq!(decoded.end_node, NodeId(20));
        assert_eq!(decoded.rel_type_id, 5);
        assert!(decoded.properties.is_empty());
        assert!(!deleted);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn test_edge_record_roundtrip_with_props() {
        let record = RelationshipRecord {
            edge_id: EdgeId(42),
            start_node: NodeId(1),
            end_node: NodeId(2),
            rel_type_id: 100,
            direction: Direction::Outgoing,
            next_out_edge: None,
            next_in_edge: None,
            properties: vec![
                (1, PropertyValue::String("since".into())),
                (2, PropertyValue::Int64(2024)),
            ],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        let bytes = serialize_edge_record(&record, false);
        let (decoded, _, _) = deserialize_edge_record(&bytes).expect("deserialize");
        assert_eq!(decoded.properties, record.properties);
    }

    #[test]
    fn test_edge_record_deleted_flag() {
        let record = RelationshipRecord {
            edge_id: EdgeId(1),
            start_node: NodeId(1),
            end_node: NodeId(2),
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
        let bytes = serialize_edge_record(&record, true);
        let (_, deleted, _) = deserialize_edge_record(&bytes).expect("deserialize");
        assert!(deleted);
    }

    #[test]
    fn test_edge_record_deserialize_truncated() {
        assert!(deserialize_edge_record(&[0u8; 10]).is_none());
    }

    // ================================================================
    // TASK-016/017: Page packing tests
    // ================================================================

    #[test]
    fn test_pack_single_record_into_page() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::NodeData as u8);
        hdr.write_to(&mut page);

        let record = NodeRecord {
            node_id: NodeId(1),
            labels: vec![1],
            properties: vec![(1, PropertyValue::Int64(42))],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        assert!(pack_record_into_page(&mut page, &bytes));

        let read_hdr = DataPageHeader::read_from(&page);
        assert_eq!(read_hdr.record_count, 1);
    }

    #[test]
    fn test_pack_multiple_records_into_page() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::NodeData as u8);
        hdr.write_to(&mut page);

        for i in 0..10u64 {
            let record = NodeRecord {
                node_id: NodeId(i),
                labels: vec![1],
                properties: vec![],
                next_edge_id: None,
                overflow_page: None,
            };
            let bytes = serialize_node_record(&record, false);
            assert!(pack_record_into_page(&mut page, &bytes));
        }

        let read_hdr = DataPageHeader::read_from(&page);
        assert_eq!(read_hdr.record_count, 10);
    }

    #[test]
    fn test_pack_record_page_full() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::NodeData as u8);
        hdr.write_to(&mut page);

        // Fill a page with records until it's full
        let mut count = 0u64;
        loop {
            let record = NodeRecord {
                node_id: NodeId(count),
                labels: vec![1, 2, 3],
                properties: vec![(1, PropertyValue::String("hello world".into()))],
                next_edge_id: None,
                overflow_page: None,
            };
            let bytes = serialize_node_record(&record, false);
            if !pack_record_into_page(&mut page, &bytes) {
                break;
            }
            count += 1;
        }
        assert!(count > 0, "should have packed at least one record");
        let read_hdr = DataPageHeader::read_from(&page);
        assert_eq!(read_hdr.record_count, count as u16);
    }

    #[test]
    fn test_read_records_from_page() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::NodeData as u8);
        hdr.write_to(&mut page);

        let mut records = vec![];
        for i in 0..5u64 {
            let record = NodeRecord {
                node_id: NodeId(i),
                labels: vec![1],
                properties: vec![(1, PropertyValue::Int64(i as i64))],
                next_edge_id: None,
                overflow_page: None,
            };
            records.push(record);
            let bytes = serialize_node_record(records.last().unwrap(), false);
            assert!(pack_record_into_page(&mut page, &bytes));
        }

        let entries = read_records_from_page(&page);
        assert_eq!(entries.len(), 5);

        // Verify each record can be deserialized back
        for (i, (off, len)) in entries.iter().enumerate() {
            let (decoded, _, _) =
                deserialize_node_record(&page[*off..*off + *len]).expect("deserialize");
            assert_eq!(decoded.node_id, NodeId(i as u64));
        }
    }

    #[test]
    fn test_read_records_empty_page() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::NodeData as u8);
        hdr.write_to(&mut page);
        let entries = read_records_from_page(&page);
        assert!(entries.is_empty());
    }

    // ================================================================
    // Property type coverage tests
    // ================================================================

    #[test]
    fn test_node_record_with_all_property_types() {
        let record = NodeRecord {
            node_id: NodeId(1),
            labels: vec![],
            properties: vec![
                (1, PropertyValue::Null),
                (2, PropertyValue::Bool(false)),
                (3, PropertyValue::Int64(-999)),
                (4, PropertyValue::Float64(2.719)),
                (5, PropertyValue::String("hello".into())),
                (6, PropertyValue::Bytes(vec![0xDE, 0xAD])),
                (7, PropertyValue::DateTime(1700000000000)),
            ],
            next_edge_id: None,
            overflow_page: None,
        };
        let bytes = serialize_node_record(&record, false);
        let (decoded, _, _) = deserialize_node_record(&bytes).expect("deserialize");
        assert_eq!(decoded.properties.len(), 7);
        assert_eq!(decoded.properties, record.properties);
    }

    // ================================================================
    // Edge records in page packing
    // ================================================================

    #[test]
    fn test_pack_edge_records_into_page() {
        let mut page = [0u8; PAGE_SIZE];
        let hdr = DataPageHeader::new(PageType::EdgeData as u8);
        hdr.write_to(&mut page);

        for i in 0..5u64 {
            let record = RelationshipRecord {
                edge_id: EdgeId(i),
                start_node: NodeId(i * 10),
                end_node: NodeId(i * 10 + 1),
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
            let bytes = serialize_edge_record(&record, false);
            assert!(pack_record_into_page(&mut page, &bytes));
        }

        let entries = read_records_from_page(&page);
        assert_eq!(entries.len(), 5);

        for (i, (off, len)) in entries.iter().enumerate() {
            let (decoded, _, _) =
                deserialize_edge_record(&page[*off..*off + *len]).expect("deserialize");
            assert_eq!(decoded.edge_id, EdgeId(i as u64));
        }
    }

    // ================================================================
    // PERSIST-001 Phase 5: SubgraphRecord serialization tests
    // ================================================================

    #[cfg(feature = "subgraph")]
    mod subgraph_tests {
        use super::*;
        use cypherlite_core::{SubgraphId, SubgraphRecord};

        #[test]
        fn test_subgraph_record_roundtrip_empty() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: None,
                properties: vec![],
            };
            let bytes = serialize_subgraph_record(&record, &[], false);
            let (decoded, members, deleted, consumed) =
                deserialize_subgraph_record(&bytes).expect("deserialize");
            assert_eq!(decoded.subgraph_id, SubgraphId(1));
            assert_eq!(decoded.temporal_anchor, None);
            assert!(decoded.properties.is_empty());
            assert!(members.is_empty());
            assert!(!deleted);
            assert_eq!(consumed, bytes.len());
        }

        #[test]
        fn test_subgraph_record_roundtrip_with_anchor_and_props() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(42),
                temporal_anchor: Some(1_700_000_000_000),
                properties: vec![
                    (1, PropertyValue::String("my-graph".into())),
                    (2, PropertyValue::Int64(99)),
                ],
            };
            let bytes = serialize_subgraph_record(&record, &[], false);
            let (decoded, _, _, _) =
                deserialize_subgraph_record(&bytes).expect("deserialize");
            assert_eq!(decoded.subgraph_id, SubgraphId(42));
            assert_eq!(decoded.temporal_anchor, Some(1_700_000_000_000));
            assert_eq!(decoded.properties, record.properties);
        }

        #[test]
        fn test_subgraph_record_roundtrip_with_members() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(5),
                temporal_anchor: None,
                properties: vec![],
            };
            let members = vec![NodeId(10), NodeId(20), NodeId(30)];
            let bytes = serialize_subgraph_record(&record, &members, false);
            let (decoded, decoded_members, _, _) =
                deserialize_subgraph_record(&bytes).expect("deserialize");
            assert_eq!(decoded.subgraph_id, SubgraphId(5));
            assert_eq!(decoded_members, members);
        }

        #[test]
        fn test_subgraph_record_deleted_flag() {
            let record = SubgraphRecord {
                subgraph_id: SubgraphId(1),
                temporal_anchor: None,
                properties: vec![],
            };
            let bytes = serialize_subgraph_record(&record, &[], true);
            let (_, _, deleted, _) =
                deserialize_subgraph_record(&bytes).expect("deserialize");
            assert!(deleted);
        }

        #[test]
        fn test_subgraph_record_deserialize_truncated() {
            assert!(deserialize_subgraph_record(&[0u8; 3]).is_none());
        }
    }

    // ================================================================
    // PERSIST-001 Phase 5: HyperEdgeRecord serialization tests
    // ================================================================

    #[cfg(feature = "hypergraph")]
    mod hyperedge_tests {
        use super::*;
        use cypherlite_core::{GraphEntity, HyperEdgeId, HyperEdgeRecord};

        #[test]
        fn test_hyperedge_record_roundtrip_empty() {
            let record = HyperEdgeRecord {
                id: HyperEdgeId(1),
                rel_type_id: 10,
                sources: vec![],
                targets: vec![],
                properties: vec![],
            };
            let bytes = serialize_hyperedge_record(&record, false);
            let (decoded, deleted, consumed) =
                deserialize_hyperedge_record(&bytes).expect("deserialize");
            assert_eq!(decoded.id, HyperEdgeId(1));
            assert_eq!(decoded.rel_type_id, 10);
            assert!(decoded.sources.is_empty());
            assert!(decoded.targets.is_empty());
            assert!(decoded.properties.is_empty());
            assert!(!deleted);
            assert_eq!(consumed, bytes.len());
        }

        #[test]
        fn test_hyperedge_record_roundtrip_with_entities() {
            let record = HyperEdgeRecord {
                id: HyperEdgeId(42),
                rel_type_id: 5,
                sources: vec![
                    GraphEntity::Node(NodeId(1)),
                    GraphEntity::Subgraph(cypherlite_core::SubgraphId(2)),
                ],
                targets: vec![
                    GraphEntity::Node(NodeId(3)),
                    GraphEntity::HyperEdge(HyperEdgeId(10)),
                    GraphEntity::TemporalRef(NodeId(4), 1_700_000_000_000),
                ],
                properties: vec![
                    (1, PropertyValue::String("rel".into())),
                    (2, PropertyValue::Int64(77)),
                ],
            };
            let bytes = serialize_hyperedge_record(&record, false);
            let (decoded, _, _) =
                deserialize_hyperedge_record(&bytes).expect("deserialize");
            assert_eq!(decoded.id, HyperEdgeId(42));
            assert_eq!(decoded.rel_type_id, 5);
            assert_eq!(decoded.sources, record.sources);
            assert_eq!(decoded.targets, record.targets);
            assert_eq!(decoded.properties, record.properties);
        }

        #[test]
        fn test_hyperedge_record_deleted_flag() {
            let record = HyperEdgeRecord {
                id: HyperEdgeId(1),
                rel_type_id: 1,
                sources: vec![],
                targets: vec![],
                properties: vec![],
            };
            let bytes = serialize_hyperedge_record(&record, true);
            let (_, deleted, _) =
                deserialize_hyperedge_record(&bytes).expect("deserialize");
            assert!(deleted);
        }

        #[test]
        fn test_hyperedge_record_deserialize_truncated() {
            assert!(deserialize_hyperedge_record(&[0u8; 5]).is_none());
        }
    }

    // ================================================================
    // PERSIST-001 Phase 5: VersionRecord serialization tests
    // ================================================================

    #[test]
    fn test_version_record_node_roundtrip() {
        use crate::version::VersionRecord;

        let node = NodeRecord {
            node_id: NodeId(1),
            labels: vec![1, 2],
            properties: vec![(1, PropertyValue::String("Alice".into()))],
            next_edge_id: None,
            overflow_page: None,
        };
        let vr = VersionRecord::Node(node.clone());
        let bytes = serialize_version_record(1, 3, &vr);
        let (entity_id, version_seq, decoded, consumed) =
            deserialize_version_record(&bytes).expect("deserialize");
        assert_eq!(entity_id, 1);
        assert_eq!(version_seq, 3);
        assert_eq!(consumed, bytes.len());
        match decoded {
            VersionRecord::Node(n) => {
                assert_eq!(n.node_id, NodeId(1));
                assert_eq!(n.labels, vec![1, 2]);
                assert_eq!(n.properties, node.properties);
            }
            _ => panic!("expected Node variant"),
        }
    }

    #[test]
    fn test_version_record_relationship_roundtrip() {
        use crate::version::VersionRecord;

        let edge = RelationshipRecord {
            edge_id: EdgeId(5),
            start_node: NodeId(1),
            end_node: NodeId(2),
            rel_type_id: 10,
            direction: Direction::Outgoing,
            next_out_edge: None,
            next_in_edge: None,
            properties: vec![(1, PropertyValue::Int64(42))],
            #[cfg(feature = "subgraph")]
            start_is_subgraph: false,
            #[cfg(feature = "subgraph")]
            end_is_subgraph: false,
        };
        let vr = VersionRecord::Relationship(edge.clone());
        let bytes = serialize_version_record(5, 1, &vr);
        let (entity_id, version_seq, decoded, _) =
            deserialize_version_record(&bytes).expect("deserialize");
        assert_eq!(entity_id, 5);
        assert_eq!(version_seq, 1);
        match decoded {
            VersionRecord::Relationship(e) => {
                assert_eq!(e.edge_id, EdgeId(5));
                assert_eq!(e.start_node, NodeId(1));
                assert_eq!(e.end_node, NodeId(2));
                assert_eq!(e.rel_type_id, 10);
                assert_eq!(e.properties, edge.properties);
            }
            _ => panic!("expected Relationship variant"),
        }
    }

    #[test]
    fn test_version_record_deserialize_truncated() {
        assert!(deserialize_version_record(&[0u8; 10]).is_none());
    }
}
