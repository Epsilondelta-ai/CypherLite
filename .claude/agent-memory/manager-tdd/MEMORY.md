# TDD Agent Memory - CypherLite

## Project Architecture
- Workspace: `crates/cypherlite-core`, `crates/cypherlite-storage`, `crates/cypherlite-query`
- Query crate: lexer (logos) -> parser (recursive descent) -> semantic -> planner -> executor
- Executor uses Volcano/Iterator model with operators in `executor/operators/`
- Integration tests in `crates/cypherlite-query/tests/`
- API entry point: `CypherLite::open(config)` then `.execute(cypher_query)`

## Key Patterns
- `RelationshipPattern` has `min_hops: Option<u32>`, `max_hops: Option<u32>` (added in Group R)
- Lexer uses logos crate; `DoubleDot` token must be BEFORE `Dot` for priority
- Adding new AST fields requires updating ALL constructors across tests in: pattern.rs, semantic/mod.rs, create.rs, merge.rs
- `LogicalPlan::VarLengthExpand` dispatches to `operators::var_length_expand`
- Default max_hops constant: `planner::DEFAULT_MAX_HOPS = 10`
- Semantic analyzer validates max_hops <= 10 and max >= min
- `LogicalPlan::NodeScan` has `limit: Option<usize>` field (added in Group S for LIMIT pushdown)
- `LogicalPlan::IndexScan` variant added in Group S for index-based lookups
- `optimize()` applies rules bottom-up: index_scan -> limit_pushdown -> constant_fold -> projection_prune
- `Expression::Property(Box<Expression>, String)` is the AST form (not `PropertyAccess`)
- `LabelRegistry` trait has `prop_key_id(&self, name) -> Option<u32>` - must import trait to use on engine
- `QueryResult` has `.rows` not `.records` in integration tests
- `StorageEngine::scan_nodes_by_property()` auto-uses index if available

## Integration Test Tips
- Node property filtering (e.g., `{name: 'Alice'}`) in MATCH patterns does NOT reliably filter before expansion
- Use unique labels (e.g., `:Root`, `:Start`, `:Leaf`) instead of property filters for test isolation
- Each `CREATE` statement creates new disconnected nodes; use single chain CREATE for connected graphs

## DateTime (Group U) Patterns
- `PropertyValue::DateTime(i64)` - millis since Unix epoch, type tag 7
- `Value::DateTime(i64)` in executor mirrors PropertyValue::DateTime
- `now()` reads `__query_start_ms__` from params (injected by api/mod.rs at query start)
- `datetime(string)` uses manual ISO 8601 parser (no chrono dependency)
- `PropertyValueKey` ordering: DateTime comes after Array (tag 7 > tag 6)
- `PropertyStore` serializes DateTime as 8-byte le i64 (same as Int64 but tag 7)
- `format_millis_as_iso8601()` and `epoch_secs_to_datetime()` in types.rs use Hinnant's civil_from_days
- `days_from_civil()` and `civil_from_days()` are inverse functions in types.rs and eval.rs respectively

## Timestamp Tracking (Group V) Patterns
- System properties: `_created_at`, `_updated_at` stored as regular PropertyValue::DateTime
- Helpers in `executor/operators/create.rs`: `is_system_property()`, `validate_no_system_properties()`, `inject_create_timestamps()`
- Timestamps injected at executor level (not storage engine) using `params["__query_start_ms__"]`
- `DatabaseConfig::temporal_tracking_enabled` (default: true) controls injection
- System property writes blocked in set_props.rs and merge.rs `apply_set_property()`
- Adding timestamps to CREATE changes property count - existing unit tests may need updating

## Version Storage (Group W) Patterns
- `VersionStore` in `crates/cypherlite-storage/src/version/mod.rs`
- BTreeMap<(entity_id, version_seq), VersionRecord> keyed by (u64, u64)
- `StorageEngine::update_node()` auto-snapshots before update when `version_storage_enabled`
- `DatabaseHeader::version_store_root_page` at bytes 36-43
- FORMAT_VERSION bumped 1->2; page_manager accepts v1 and auto-migrates
- `DatabaseConfig::version_storage_enabled` (default: true)

## Temporal Edge Filtering (Groups DD/EE) Patterns
- `TemporalFilter` enum in `executor/operators/temporal_filter.rs`: `AsOf(i64)`, `Between(i64, i64)`
- `is_edge_temporally_valid()` checks `_valid_from`/`_valid_to` on edges via `engine.get_edge()`
- Accepts BOTH `PropertyValue::DateTime` AND `PropertyValue::Int64` for timestamps (SET with int literal stores Int64)
- `TemporalFilterPlan` in planner: holds unevaluated expressions, resolved at execution time
- `annotate_temporal_filter()` walks plan tree setting temporal_filter on Expand/VarLengthExpand
- `resolve_temporal_filter()` in executor evaluates expressions via `eval()` before passing to operators
- LogicalPlan::Expand and VarLengthExpand have `temporal_filter: Option<TemporalFilterPlan>` field
- BETWEEN TIME tests: node must have a version (created_at/updated_at) within the BETWEEN range for TemporalRangeScan to include it
- Edge properties set via `SET r._valid_from = <int>` store as Int64, not DateTime

## Subgraph (Groups GG/II) Patterns
- Feature flag chain: `temporal-core -> temporal-edge -> subgraph` in both core and storage Cargo.toml
- `SubgraphId(pub u64)` newtype, `SubgraphRecord`, `GraphEntity` enum all `#[cfg(feature = "subgraph")]`
- `SubgraphNotFound(u64)` and `FeatureRequiresSubgraph` error variants cfg-gated
- DatabaseHeader v4: `FLAG_SUBGRAPH = 1 << 2`, `subgraph_root_page` (bytes 48-55), `next_subgraph_id` (bytes 56-63)
- `FORMAT_VERSION` is cfg-gated: 4 with subgraph, 3 without
- `SubgraphStore` in `crates/cypherlite-storage/src/subgraph/mod.rs` - BTreeMap<u64, SubgraphRecord>
- `MembershipIndex` in `subgraph/membership.rs` - forward/reverse BTreeMaps, idempotent add
- StorageEngine: `create_subgraph`, `get_subgraph`, `delete_subgraph`, `add_member`, `remove_member`, `list_members`, `get_subgraph_memberships`
- `delete_subgraph` cascades membership removal via `MembershipIndex::remove_all`
- When adding cfg-gated fields to DatabaseHeader, existing tests using struct literals need `..DatabaseHeader::new()` spread
- `page_manager.rs` version check: `header.version == 0 || header.version > FORMAT_VERSION` (accepts all older versions)
- Tests checking hardcoded FORMAT_VERSION values need `#[cfg(not(feature = "subgraph"))]` guard

## Test Counts
- Baseline before Group R: 729 tests
- After Group R: 791 tests (+62 new)
- After Group S: 856 tests (+65 new)
- After Group U: 897 tests (+32 new, +9 in core, +3 in storage, +20 in query)
- After Group V: 915 tests (+18 new, +5 in core, +13 integration)
- After Group W: 936 tests (+21 new, +11 version module, +4 page header, +7 integration)
- After Groups DD/EE/FF: 1035 tests (+20 new: 8 temporal_filter unit, 9 temporal_edge integration, 3 proptest)
- After Groups GG/II (default): 1035 tests (unchanged, new tests only with subgraph feature)
- After Groups GG/II (subgraph): 1093 tests (+58 new: 18 core types, 10 subgraph store, 11 membership, 9 header v4, 14 engine integration, -1 cfg-gated, -3 format_version adjustments)
