// Property-based tests for hypergraph operations (NN/PP).
//
// Invariants tested:
// 1. HyperEdgeStore create-get roundtrip: fields match after creation
// 2. ReverseIndex consistency: forward and reverse indexes agree
// 3. GraphEntity variant identity: all variants preserve data

#![cfg(feature = "hypergraph")]

use cypherlite_core::*;
use cypherlite_storage::hyperedge::reverse_index::HyperEdgeReverseIndex;
use cypherlite_storage::hyperedge::HyperEdgeStore;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Arbitrary generators
// ---------------------------------------------------------------------------

fn arb_graph_entity() -> impl Strategy<Value = GraphEntity> {
    prop_oneof![
        (1u64..1000).prop_map(|id| GraphEntity::Node(NodeId(id))),
        (1u64..100).prop_map(|id| GraphEntity::Subgraph(SubgraphId(id))),
        (1u64..100).prop_map(|id| GraphEntity::HyperEdge(HyperEdgeId(id))),
        (1u64..1000, any::<i64>()).prop_map(|(id, ts)| GraphEntity::TemporalRef(NodeId(id), ts)),
    ]
}

fn arb_property_value() -> impl Strategy<Value = PropertyValue> {
    prop_oneof![
        Just(PropertyValue::Null),
        any::<bool>().prop_map(PropertyValue::Bool),
        any::<i64>().prop_map(PropertyValue::Int64),
        "[a-zA-Z0-9]{0,16}".prop_map(PropertyValue::String),
    ]
}

// ---------------------------------------------------------------------------
// PP-002: HyperEdgeStore create-get roundtrip
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn prop_hyperedge_create_get_roundtrip(
        rel_type_id in 0u32..100,
        sources in prop::collection::vec(arb_graph_entity(), 0..5),
        targets in prop::collection::vec(arb_graph_entity(), 0..5),
        props in prop::collection::vec((0u32..50, arb_property_value()), 0..4),
    ) {
        let mut store = HyperEdgeStore::new(1);
        let he_id = store.create(rel_type_id, sources.clone(), targets.clone(), props.clone());

        let record = store.get(he_id).expect("get after create");
        prop_assert_eq!(record.id, he_id);
        prop_assert_eq!(record.rel_type_id, rel_type_id);
        prop_assert_eq!(&record.sources, &sources);
        prop_assert_eq!(&record.targets, &targets);
        prop_assert_eq!(&record.properties, &props);
    }
}

// ---------------------------------------------------------------------------
// PP-002: ReverseIndex consistency
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn prop_reverse_index_consistency(
        ops in prop::collection::vec(
            (1u64..50, prop::collection::vec(1u64..200, 0..6)),
            1..10,
        ),
    ) {
        let mut store = HyperEdgeStore::new(1);
        let mut rev_idx = HyperEdgeReverseIndex::new();

        // Create hyperedges and register participants in the reverse index.
        let mut created_ids: Vec<HyperEdgeId> = Vec::new();
        for (rel_type, participant_ids) in &ops {
            let sources: Vec<GraphEntity> = participant_ids
                .iter()
                .map(|id| GraphEntity::Node(NodeId(*id)))
                .collect();
            let he_id = store.create(*rel_type as u32, sources.clone(), vec![], vec![]);
            created_ids.push(he_id);

            for pid in participant_ids {
                rev_idx.add(he_id.0, *pid);
            }
        }

        // Verify consistency: for every live hyperedge, each participant's
        // reverse index must list that hyperedge.
        for he_id in &created_ids {
            let record = store.get(*he_id).expect("hyperedge must exist");
            let fwd_participants = rev_idx.participants_for(he_id.0);

            // Every source entity's raw ID should appear in the forward index.
            for entity in &record.sources {
                let raw_id = match entity {
                    GraphEntity::Node(nid) => nid.0,
                    GraphEntity::Subgraph(sid) => sid.0,
                    GraphEntity::HyperEdge(hid) => hid.0,
                    GraphEntity::TemporalRef(nid, _) => nid.0,
                };
                prop_assert!(
                    fwd_participants.contains(&raw_id),
                    "participant {} missing from forward index for hyperedge {}",
                    raw_id, he_id.0,
                );
            }

            // Every forward-index participant should also reference this
            // hyperedge in its reverse index.
            for pid in &fwd_participants {
                let rev_hyperedges = rev_idx.hyperedges_for(*pid);
                prop_assert!(
                    rev_hyperedges.contains(&he_id.0),
                    "hyperedge {} missing from reverse index for participant {}",
                    he_id.0, pid,
                );
            }
        }

        // Verify: deleting a hyperedge cleans up both indexes.
        if let Some(he_id) = created_ids.first().copied() {
            let old_participants = rev_idx.remove_all(he_id.0);
            store.delete(he_id);

            prop_assert!(store.get(he_id).is_none());
            for pid in &old_participants {
                prop_assert!(
                    !rev_idx.hyperedges_for(*pid).contains(&he_id.0),
                    "hyperedge {} still in reverse index after removal for participant {}",
                    he_id.0, pid,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PP-002: GraphEntity variant identity
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn prop_graph_entity_variant_identity(entity in arb_graph_entity()) {
        // Verify that cloning preserves identity.
        let cloned = entity.clone();
        prop_assert_eq!(&entity, &cloned);

        // Verify variant-specific data is preserved.
        match &entity {
            GraphEntity::Node(nid) => {
                if let GraphEntity::Node(nid2) = &cloned {
                    prop_assert_eq!(nid.0, nid2.0);
                } else {
                    prop_assert!(false, "variant mismatch after clone");
                }
            }
            GraphEntity::Subgraph(sid) => {
                if let GraphEntity::Subgraph(sid2) = &cloned {
                    prop_assert_eq!(sid.0, sid2.0);
                } else {
                    prop_assert!(false, "variant mismatch after clone");
                }
            }
            GraphEntity::HyperEdge(hid) => {
                if let GraphEntity::HyperEdge(hid2) = &cloned {
                    prop_assert_eq!(hid.0, hid2.0);
                } else {
                    prop_assert!(false, "variant mismatch after clone");
                }
            }
            GraphEntity::TemporalRef(nid, ts) => {
                if let GraphEntity::TemporalRef(nid2, ts2) = &cloned {
                    prop_assert_eq!(nid.0, nid2.0);
                    prop_assert_eq!(ts, ts2);
                } else {
                    prop_assert!(false, "variant mismatch after clone");
                }
            }
        }
    }
}
