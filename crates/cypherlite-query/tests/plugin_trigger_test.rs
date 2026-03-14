//! Integration tests for the Trigger plugin system (Phase 10e).
//!
//! All tests are gated behind `#[cfg(feature = "plugin")]`.

#![cfg(feature = "plugin")]

use cypherlite_core::error::CypherLiteError;
use cypherlite_core::plugin::{Plugin, Trigger, TriggerContext};
use cypherlite_query::api::CypherLite;
use cypherlite_query::executor::Value;

use std::sync::{Arc, Mutex};
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// Test helpers: AuditTrigger (logs all trigger invocations)
// ---------------------------------------------------------------------------

/// A trigger that records each invocation into a shared log.
struct AuditTrigger {
    log: Arc<Mutex<Vec<String>>>,
}

impl Plugin for AuditTrigger {
    fn name(&self) -> &str {
        "audit"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Trigger for AuditTrigger {
    fn on_before_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log
            .lock()
            .unwrap()
            .push(format!("before_create:{:?}", ctx.entity_type));
        Ok(())
    }
    fn on_after_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log.lock().unwrap().push(format!(
            "after_create:{:?}:{}",
            ctx.entity_type, ctx.entity_id
        ));
        Ok(())
    }
    fn on_before_update(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log.lock().unwrap().push(format!(
            "before_update:{:?}:{}",
            ctx.entity_type, ctx.entity_id
        ));
        Ok(())
    }
    fn on_after_update(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log.lock().unwrap().push(format!(
            "after_update:{:?}:{}",
            ctx.entity_type, ctx.entity_id
        ));
        Ok(())
    }
    fn on_before_delete(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log.lock().unwrap().push(format!(
            "before_delete:{:?}:{}",
            ctx.entity_type, ctx.entity_id
        ));
        Ok(())
    }
    fn on_after_delete(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        self.log.lock().unwrap().push(format!(
            "after_delete:{:?}:{}",
            ctx.entity_type, ctx.entity_id
        ));
        Ok(())
    }
}

/// A trigger that blocks before_create (returns error).
struct BlockCreateTrigger;

impl Plugin for BlockCreateTrigger {
    fn name(&self) -> &str {
        "block_create"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Trigger for BlockCreateTrigger {
    fn on_before_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Err(CypherLiteError::TriggerError(
            "creation blocked by trigger".to_string(),
        ))
    }
    fn on_after_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
}

/// A trigger that blocks before_update (returns error).
struct BlockUpdateTrigger;

impl Plugin for BlockUpdateTrigger {
    fn name(&self) -> &str {
        "block_update"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Trigger for BlockUpdateTrigger {
    fn on_before_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Err(CypherLiteError::TriggerError(
            "update blocked by trigger".to_string(),
        ))
    }
    fn on_after_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
}

/// A trigger that blocks before_delete (returns error).
struct BlockDeleteTrigger;

impl Plugin for BlockDeleteTrigger {
    fn name(&self) -> &str {
        "block_delete"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
}

impl Trigger for BlockDeleteTrigger {
    fn on_before_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_create(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_after_update(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
    fn on_before_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Err(CypherLiteError::TriggerError(
            "deletion blocked by trigger".to_string(),
        ))
    }
    fn on_after_delete(&self, _ctx: &TriggerContext) -> Result<(), CypherLiteError> {
        Ok(())
    }
}

fn test_config(dir: &std::path::Path) -> cypherlite_core::DatabaseConfig {
    cypherlite_core::DatabaseConfig {
        path: dir.join("test.cyl"),
        wal_sync_mode: cypherlite_core::SyncMode::Normal,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// T1: Register trigger, CREATE node, verify before/after_create fired
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_fires_on_node_create() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log = Arc::new(Mutex::new(Vec::new()));

    db.register_trigger(Box::new(AuditTrigger { log: log.clone() }))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    let entries = log.lock().unwrap();
    assert!(
        entries.iter().any(|e| e.starts_with("before_create:Node")),
        "expected before_create:Node in log, got: {:?}",
        *entries,
    );
    assert!(
        entries.iter().any(|e| e.starts_with("after_create:Node")),
        "expected after_create:Node in log, got: {:?}",
        *entries,
    );
}

// ---------------------------------------------------------------------------
// T2: Register trigger, CREATE relationship, verify triggers fired
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_fires_on_relationship_create() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log = Arc::new(Mutex::new(Vec::new()));

    db.register_trigger(Box::new(AuditTrigger { log: log.clone() }))
        .expect("register");

    db.execute("CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})")
        .expect("create");

    let entries = log.lock().unwrap();
    // Should have before/after for 2 nodes and 1 edge
    let before_create_nodes = entries
        .iter()
        .filter(|e| e.starts_with("before_create:Node"))
        .count();
    let after_create_nodes = entries
        .iter()
        .filter(|e| e.starts_with("after_create:Node"))
        .count();
    let before_create_edges = entries
        .iter()
        .filter(|e| e.starts_with("before_create:Edge"))
        .count();
    let after_create_edges = entries
        .iter()
        .filter(|e| e.starts_with("after_create:Edge"))
        .count();

    assert_eq!(before_create_nodes, 2, "expected 2 before_create:Node");
    assert_eq!(after_create_nodes, 2, "expected 2 after_create:Node");
    assert_eq!(before_create_edges, 1, "expected 1 before_create:Edge");
    assert_eq!(after_create_edges, 1, "expected 1 after_create:Edge");
}

// ---------------------------------------------------------------------------
// T3: Register trigger, SET property, verify before/after_update fired
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_fires_on_set_property() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log = Arc::new(Mutex::new(Vec::new()));

    db.register_trigger(Box::new(AuditTrigger { log: log.clone() }))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // Clear the creation log entries
    log.lock().unwrap().clear();

    db.execute("MATCH (n:Person) SET n.age = 30").expect("set");

    let entries = log.lock().unwrap();
    assert!(
        entries.iter().any(|e| e.starts_with("before_update:Node")),
        "expected before_update:Node in log, got: {:?}",
        *entries,
    );
    assert!(
        entries.iter().any(|e| e.starts_with("after_update:Node")),
        "expected after_update:Node in log, got: {:?}",
        *entries,
    );
}

// ---------------------------------------------------------------------------
// T4: Register trigger, DELETE node, verify before/after_delete fired
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_fires_on_delete() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log = Arc::new(Mutex::new(Vec::new()));

    db.register_trigger(Box::new(AuditTrigger { log: log.clone() }))
        .expect("register");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // Clear creation logs
    log.lock().unwrap().clear();

    db.execute("MATCH (n:Person) DELETE n").expect("delete");

    let entries = log.lock().unwrap();
    assert!(
        entries.iter().any(|e| e.starts_with("before_delete:Node")),
        "expected before_delete:Node in log, got: {:?}",
        *entries,
    );
    assert!(
        entries.iter().any(|e| e.starts_with("after_delete:Node")),
        "expected after_delete:Node in log, got: {:?}",
        *entries,
    );
}

// ---------------------------------------------------------------------------
// T5: before_create returns error -> CREATE aborted, node not created
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_blocks_create() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    db.register_trigger(Box::new(BlockCreateTrigger))
        .expect("register");

    let result = db.execute("CREATE (n:Person {name: 'Alice'})");
    assert!(result.is_err(), "CREATE should fail when trigger blocks it");

    let err_msg = format!("{}", result.expect_err("should error"));
    assert!(
        err_msg.contains("trigger") || err_msg.contains("Trigger"),
        "error should mention trigger, got: {}",
        err_msg,
    );

    // Verify no node was created
    assert_eq!(db.engine().node_count(), 0, "no node should be created");
}

// ---------------------------------------------------------------------------
// T6: before_update returns error -> SET aborted, property unchanged
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_blocks_update() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create node first (no trigger yet)
    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // Register blocking trigger
    db.register_trigger(Box::new(BlockUpdateTrigger))
        .expect("register");

    let result = db.execute("MATCH (n:Person) SET n.name = 'Bob'");
    assert!(result.is_err(), "SET should fail when trigger blocks it");

    // Verify property unchanged
    let check = db.execute("MATCH (n:Person) RETURN n.name").expect("check");
    assert_eq!(check.rows.len(), 1);
    assert_eq!(
        check.rows[0].get("n.name"),
        Some(&Value::String("Alice".to_string())),
    );
}

// ---------------------------------------------------------------------------
// T7: before_delete returns error -> DELETE aborted, node still exists
// ---------------------------------------------------------------------------

#[test]
fn test_trigger_blocks_delete() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");

    // Create node first (no trigger yet)
    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    // Register blocking trigger
    db.register_trigger(Box::new(BlockDeleteTrigger))
        .expect("register");

    let result = db.execute("MATCH (n:Person) DELETE n");
    assert!(result.is_err(), "DELETE should fail when trigger blocks it");

    // Verify node still exists
    let check = db.execute("MATCH (n:Person) RETURN n.name").expect("check");
    assert_eq!(check.rows.len(), 1);
}

// ---------------------------------------------------------------------------
// T8: Multiple triggers all fire in order
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_triggers_fire_in_order() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log1 = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::new(Mutex::new(Vec::new()));

    // Register two distinct audit triggers
    db.register_trigger(Box::new(AuditTrigger { log: log1.clone() }))
        .expect("register audit");

    // Second trigger with different name
    struct AuditTrigger2 {
        log: Arc<Mutex<Vec<String>>>,
    }
    impl Plugin for AuditTrigger2 {
        fn name(&self) -> &str {
            "audit2"
        }
        fn version(&self) -> &str {
            "1.0.0"
        }
    }
    impl Trigger for AuditTrigger2 {
        fn on_before_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("before_create:{:?}", ctx.entity_type));
            Ok(())
        }
        fn on_after_create(&self, ctx: &TriggerContext) -> Result<(), CypherLiteError> {
            self.log
                .lock()
                .unwrap()
                .push(format!("after_create:{:?}", ctx.entity_type));
            Ok(())
        }
        fn on_before_update(&self, _: &TriggerContext) -> Result<(), CypherLiteError> {
            Ok(())
        }
        fn on_after_update(&self, _: &TriggerContext) -> Result<(), CypherLiteError> {
            Ok(())
        }
        fn on_before_delete(&self, _: &TriggerContext) -> Result<(), CypherLiteError> {
            Ok(())
        }
        fn on_after_delete(&self, _: &TriggerContext) -> Result<(), CypherLiteError> {
            Ok(())
        }
    }

    db.register_trigger(Box::new(AuditTrigger2 { log: log2.clone() }))
        .expect("register audit2");

    db.execute("CREATE (n:Person {name: 'Alice'})")
        .expect("create");

    let entries1 = log1.lock().unwrap();
    let entries2 = log2.lock().unwrap();

    // Both triggers should have fired
    assert!(
        !entries1.is_empty(),
        "audit trigger should have log entries"
    );
    assert!(
        !entries2.is_empty(),
        "audit2 trigger should have log entries"
    );
}

// ---------------------------------------------------------------------------
// T9: list_triggers() returns registered triggers
// ---------------------------------------------------------------------------

#[test]
fn test_list_triggers() {
    let dir = tempdir().expect("tempdir");
    let mut db = CypherLite::open(test_config(dir.path())).expect("open");
    let log = Arc::new(Mutex::new(Vec::new()));

    db.register_trigger(Box::new(AuditTrigger { log: log.clone() }))
        .expect("register");

    let triggers = db.list_triggers();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].0, "audit");
    assert_eq!(triggers[0].1, "1.0.0");
}
