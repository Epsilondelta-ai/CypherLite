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
        if let Some(existing) = self.variables.get(&name) {
            if existing.kind != kind {
                return Err(format!(
                    "variable '{}' already defined as {:?}",
                    name, existing.kind
                ));
            }
            // Re-defining with same kind is ok (e.g., same var in multiple patterns).
            return Ok(());
        }
        self.variables.insert(name, VariableInfo { kind });
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
}
