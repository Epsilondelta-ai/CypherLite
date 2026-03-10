// SymbolTable: query-local variable binding table

use std::collections::HashMap;

/// Tracks variable bindings within a Cypher query scope.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// Map from variable name to its binding info.
    variables: HashMap<String, VariableInfo>,
}

/// Information about a bound variable.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableInfo {
    /// What kind of thing this variable refers to.
    pub kind: VariableKind,
    /// Whether this variable may be NULL (e.g., from OPTIONAL MATCH).
    pub nullable: bool,
}

/// The kind of entity a variable refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    /// A graph node.
    Node,
    /// A graph relationship.
    Relationship,
    /// An expression alias (from WITH/RETURN).
    Expression,
}

impl SymbolTable {
    /// Create a new empty symbol table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a variable. Returns error if already defined with a different kind.
    pub fn define(&mut self, name: String, kind: VariableKind) -> Result<(), String> {
        self.define_with_nullable(name, kind, false)
    }

    /// Define a variable with nullable annotation. Returns error if already defined
    /// with a different kind. If the variable already exists, its nullable flag is
    /// NOT changed -- only newly introduced variables get the nullable annotation.
    /// This preserves correct semantics for OPTIONAL MATCH: anchor variables from
    /// earlier MATCH clauses are not marked nullable even when re-referenced in
    /// OPTIONAL MATCH patterns.
    pub fn define_with_nullable(
        &mut self,
        name: String,
        kind: VariableKind,
        nullable: bool,
    ) -> Result<(), String> {
        if let Some(existing) = self.variables.get(&name) {
            if existing.kind != kind {
                return Err(format!(
                    "variable '{}' already defined as {:?}",
                    name, existing.kind
                ));
            }
            // Variable already exists: keep its current nullable status.
            return Ok(());
        }
        self.variables.insert(name, VariableInfo { kind, nullable });
        Ok(())
    }

    /// Check if a variable is defined.
    pub fn is_defined(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Get variable info.
    pub fn get(&self, name: &str) -> Option<&VariableInfo> {
        self.variables.get(name)
    }

    /// Reset scope: clear all variables and define only the specified ones.
    /// Used by WITH clause to implement scope reset -- only projected variables survive.
    pub fn reset_scope(&mut self, survivors: &[(String, VariableKind)]) {
        self.variables.clear();
        for (name, kind) in survivors {
            self.variables.insert(
                name.clone(),
                VariableInfo {
                    kind: *kind,
                    nullable: false,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_symbol_table_is_empty() {
        let st = SymbolTable::new();
        assert!(!st.is_defined("n"));
        assert!(st.get("n").is_none());
    }

    #[test]
    fn test_define_and_lookup_node() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();
        assert!(st.is_defined("n"));
        assert_eq!(st.get("n").unwrap().kind, VariableKind::Node);
    }

    #[test]
    fn test_define_and_lookup_relationship() {
        let mut st = SymbolTable::new();
        st.define("r".to_string(), VariableKind::Relationship)
            .unwrap();
        assert!(st.is_defined("r"));
        assert_eq!(st.get("r").unwrap().kind, VariableKind::Relationship);
    }

    #[test]
    fn test_define_and_lookup_expression() {
        let mut st = SymbolTable::new();
        st.define("alias".to_string(), VariableKind::Expression)
            .unwrap();
        assert!(st.is_defined("alias"));
        assert_eq!(st.get("alias").unwrap().kind, VariableKind::Expression);
    }

    #[test]
    fn test_redefine_same_kind_is_ok() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();
        // Re-defining with same kind succeeds.
        let result = st.define("n".to_string(), VariableKind::Node);
        assert!(result.is_ok());
    }

    #[test]
    fn test_redefine_different_kind_errors() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();
        let result = st.define("n".to_string(), VariableKind::Relationship);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already defined as"));
    }

    #[test]
    fn test_multiple_variables() {
        let mut st = SymbolTable::new();
        st.define("a".to_string(), VariableKind::Node).unwrap();
        st.define("r".to_string(), VariableKind::Relationship)
            .unwrap();
        st.define("b".to_string(), VariableKind::Node).unwrap();

        assert!(st.is_defined("a"));
        assert!(st.is_defined("r"));
        assert!(st.is_defined("b"));
        assert!(!st.is_defined("c"));
    }

    #[test]
    fn test_undefined_variable_returns_none() {
        let st = SymbolTable::new();
        assert!(st.get("nonexistent").is_none());
    }

    // TASK-060: SymbolTable scope reset for WITH clause
    #[test]
    fn test_reset_scope_keeps_only_specified_variables() {
        let mut st = SymbolTable::new();
        st.define("a".to_string(), VariableKind::Node).unwrap();
        st.define("b".to_string(), VariableKind::Node).unwrap();
        st.define("r".to_string(), VariableKind::Relationship)
            .unwrap();

        // After WITH a, only 'a' survives
        st.reset_scope(&[("a".to_string(), VariableKind::Expression)]);

        assert!(st.is_defined("a"));
        assert!(!st.is_defined("b"));
        assert!(!st.is_defined("r"));
        // The surviving variable should have kind Expression (it was projected via WITH)
        assert_eq!(st.get("a").unwrap().kind, VariableKind::Expression);
    }

    #[test]
    fn test_reset_scope_empty_clears_all() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();
        st.reset_scope(&[]);
        assert!(!st.is_defined("n"));
    }

    // TASK-074: nullable variable tracking
    #[test]
    fn test_define_with_nullable() {
        let mut st = SymbolTable::new();
        st.define_with_nullable("b".to_string(), VariableKind::Node, true)
            .unwrap();
        assert!(st.is_defined("b"));
        assert!(st.get("b").unwrap().nullable);
    }

    #[test]
    fn test_define_non_nullable_by_default() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();
        assert!(!st.get("n").unwrap().nullable);
    }

    #[test]
    fn test_redefine_preserves_original_nullable_status() {
        let mut st = SymbolTable::new();
        // First define as non-nullable (regular MATCH)
        st.define("a".to_string(), VariableKind::Node).unwrap();
        assert!(!st.get("a").unwrap().nullable);
        // Re-reference in OPTIONAL MATCH context should NOT upgrade to nullable
        st.define_with_nullable("a".to_string(), VariableKind::Node, true)
            .unwrap();
        assert!(!st.get("a").unwrap().nullable);
    }

    #[test]
    fn test_nullable_preserved_on_non_nullable_redefine() {
        let mut st = SymbolTable::new();
        // First define as nullable (OPTIONAL MATCH introduces this)
        st.define_with_nullable("b".to_string(), VariableKind::Node, true)
            .unwrap();
        assert!(st.get("b").unwrap().nullable);
        // Redefine as non-nullable -- should remain nullable (original status preserved)
        st.define_with_nullable("b".to_string(), VariableKind::Node, false)
            .unwrap();
        assert!(st.get("b").unwrap().nullable);
    }

    #[test]
    fn test_reset_scope_with_alias() {
        let mut st = SymbolTable::new();
        st.define("n".to_string(), VariableKind::Node).unwrap();

        // WITH n.name AS name -- introduces 'name' as Expression, removes 'n'
        st.reset_scope(&[("name".to_string(), VariableKind::Expression)]);

        assert!(!st.is_defined("n"));
        assert!(st.is_defined("name"));
        assert_eq!(st.get("name").unwrap().kind, VariableKind::Expression);
    }
}
