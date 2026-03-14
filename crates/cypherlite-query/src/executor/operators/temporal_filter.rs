// Temporal filter for edge validity checking during AT TIME / BETWEEN TIME queries.
//
// DD-T1: TemporalFilter enum and edge validity checking.

use cypherlite_core::PropertyValue;
use cypherlite_storage::StorageEngine;

use crate::executor::operators::create::{TEMPORAL_PROP_VALID_FROM, TEMPORAL_PROP_VALID_TO};
use cypherlite_core::{EdgeId, LabelRegistry};

/// Extract millisecond timestamp from a PropertyValue (supports both DateTime and Int64).
fn extract_millis(v: &PropertyValue) -> Option<i64> {
    match v {
        PropertyValue::DateTime(ms) => Some(*ms),
        PropertyValue::Int64(ms) => Some(*ms),
        _ => None,
    }
}

/// Temporal filter applied to edges during traversal.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalFilter {
    /// Edge must be valid at a specific timestamp.
    AsOf(i64),
    /// Edge validity must overlap with [start, end].
    Between(i64, i64),
}

/// Check if an edge is temporally valid given a filter.
///
/// Looks up `_valid_from` and `_valid_to` properties on the edge.
/// Edges without `_valid_from` are treated as always valid (backward compat).
///
/// For AsOf(T): valid if `_valid_from <= T AND (_valid_to IS NULL OR _valid_to > T)`
/// For Between(start, end): valid if `_valid_from <= end AND (_valid_to IS NULL OR _valid_to > start)`
pub fn is_edge_temporally_valid(
    edge_id: EdgeId,
    filter: &TemporalFilter,
    engine: &StorageEngine,
) -> bool {
    let edge = match engine.get_edge(edge_id) {
        Some(e) => e,
        None => return false,
    };

    let valid_from_key = engine.prop_key_id(TEMPORAL_PROP_VALID_FROM);
    let valid_to_key = engine.prop_key_id(TEMPORAL_PROP_VALID_TO);

    // Get _valid_from timestamp (accepts both DateTime and Int64 for flexibility)
    let valid_from = valid_from_key.and_then(|key| {
        edge.properties
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| extract_millis(v))
    });

    // Edges without _valid_from are always valid (backward compat)
    let valid_from = match valid_from {
        Some(vf) => vf,
        None => return true,
    };

    // Get _valid_to timestamp (optional, accepts both DateTime and Int64)
    let valid_to = valid_to_key.and_then(|key| {
        edge.properties
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| extract_millis(v))
    });

    match filter {
        TemporalFilter::AsOf(t) => {
            // valid_from <= T AND (_valid_to IS NULL OR _valid_to > T)
            valid_from <= *t && valid_to.is_none_or(|vt| vt > *t)
        }
        TemporalFilter::Between(start, end) => {
            // Overlap: valid_from <= end AND (_valid_to IS NULL OR _valid_to > start)
            valid_from <= *end && valid_to.is_none_or(|vt| vt > *start)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypherlite_core::{DatabaseConfig, LabelRegistry, SyncMode};
    use cypherlite_storage::StorageEngine;
    use tempfile::tempdir;

    fn test_engine(dir: &std::path::Path) -> StorageEngine {
        let config = DatabaseConfig {
            path: dir.join("test.cyl"),
            wal_sync_mode: SyncMode::Normal,
            ..Default::default()
        };
        StorageEngine::open(config).expect("open")
    }

    // DD-T1: Edge without _valid_from is always valid
    #[test]
    fn test_edge_without_valid_from_always_valid() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);
        let eid = engine.create_edge(n1, n2, knows, vec![]).expect("edge");

        let filter = TemporalFilter::AsOf(1_000_000);
        assert!(is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: Edge with _valid_from before T is valid at T
    #[test]
    fn test_edge_valid_from_before_as_of() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let props = vec![(vf_key, PropertyValue::DateTime(500))];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        // At T=1000, edge with _valid_from=500 should be valid
        let filter = TemporalFilter::AsOf(1000);
        assert!(is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: Edge with _valid_from after T is NOT valid at T
    #[test]
    fn test_edge_valid_from_after_as_of() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let props = vec![(vf_key, PropertyValue::DateTime(2000))];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        let filter = TemporalFilter::AsOf(1000);
        assert!(!is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: Edge with _valid_to before T is NOT valid at T
    #[test]
    fn test_edge_valid_to_before_as_of() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let vt_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_TO);
        let props = vec![
            (vf_key, PropertyValue::DateTime(100)),
            (vt_key, PropertyValue::DateTime(500)),
        ];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        // _valid_from=100, _valid_to=500, T=1000 -> NOT valid (expired)
        let filter = TemporalFilter::AsOf(1000);
        assert!(!is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: Edge with _valid_from <= T and _valid_to > T is valid
    #[test]
    fn test_edge_within_validity_window() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let vt_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_TO);
        let props = vec![
            (vf_key, PropertyValue::DateTime(100)),
            (vt_key, PropertyValue::DateTime(2000)),
        ];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        let filter = TemporalFilter::AsOf(1000);
        assert!(is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: BETWEEN overlap -- edge overlaps range
    #[test]
    fn test_edge_between_overlaps() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let vt_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_TO);
        let props = vec![
            (vf_key, PropertyValue::DateTime(500)),
            (vt_key, PropertyValue::DateTime(1500)),
        ];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        // Range [1000, 2000] overlaps with [500, 1500]
        let filter = TemporalFilter::Between(1000, 2000);
        assert!(is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: BETWEEN no overlap -- edge does NOT overlap range
    #[test]
    fn test_edge_between_no_overlap() {
        let dir = tempdir().expect("tmpdir");
        let mut engine = test_engine(dir.path());
        let knows = engine.get_or_create_rel_type("KNOWS");
        let n1 = engine.create_node(vec![], vec![]);
        let n2 = engine.create_node(vec![], vec![]);

        let vf_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_FROM);
        let vt_key = engine.get_or_create_prop_key(TEMPORAL_PROP_VALID_TO);
        let props = vec![
            (vf_key, PropertyValue::DateTime(100)),
            (vt_key, PropertyValue::DateTime(400)),
        ];
        let eid = engine.create_edge(n1, n2, knows, props).expect("edge");

        // Range [1000, 2000] does NOT overlap [100, 400]
        let filter = TemporalFilter::Between(1000, 2000);
        assert!(!is_edge_temporally_valid(eid, &filter, &engine));
    }

    // DD-T1: Nonexistent edge returns false
    #[test]
    fn test_nonexistent_edge_returns_false() {
        let dir = tempdir().expect("tmpdir");
        let engine = test_engine(dir.path());

        let filter = TemporalFilter::AsOf(1000);
        assert!(!is_edge_temporally_valid(EdgeId(999), &filter, &engine));
    }
}
