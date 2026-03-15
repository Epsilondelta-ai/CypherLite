// AST node type definitions for openCypher subset

/// A complete Cypher query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Ordered list of clauses that make up the query.
    pub clauses: Vec<Clause>,
}

/// Top-level clause types.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    /// `MATCH` clause.
    Match(MatchClause),
    /// `RETURN` clause.
    Return(ReturnClause),
    /// `CREATE` clause.
    Create(CreateClause),
    /// `SET` clause.
    Set(SetClause),
    /// `REMOVE` clause.
    Remove(RemoveClause),
    /// `DELETE` / `DETACH DELETE` clause.
    Delete(DeleteClause),
    /// `WITH` clause.
    With(WithClause),
    /// `MERGE` clause.
    Merge(MergeClause),
    /// `UNWIND` clause.
    Unwind(UnwindClause),
    /// `CREATE INDEX` clause.
    CreateIndex(CreateIndexClause),
    /// `DROP INDEX` clause.
    DropIndex(DropIndexClause),
    /// `CREATE SNAPSHOT` clause (subgraph feature).
    #[cfg(feature = "subgraph")]
    CreateSnapshot(CreateSnapshotClause),
    /// `CREATE HYPEREDGE` clause (hypergraph feature).
    #[cfg(feature = "hypergraph")]
    CreateHyperedge(CreateHyperedgeClause),
    /// `MATCH HYPEREDGE` clause (hypergraph feature).
    #[cfg(feature = "hypergraph")]
    MatchHyperedge(MatchHyperedgeClause),
}

/// Temporal predicate for time-travel queries.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalPredicate {
    /// `AT TIME expr` -- find the version current at a specific point in time.
    AsOf(Expression),
    /// `BETWEEN TIME expr AND expr` -- find all versions within a time range.
    Between(Expression, Expression),
}

/// A `MATCH` clause with optional temporal predicate and filter.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    /// True when `OPTIONAL MATCH`.
    pub optional: bool,
    /// The graph pattern to match.
    pub pattern: Pattern,
    /// Optional temporal predicate (`AT TIME` / `BETWEEN TIME`).
    pub temporal_predicate: Option<TemporalPredicate>,
    /// Optional `WHERE` filter expression.
    pub where_clause: Option<Expression>,
}

/// A `RETURN` clause with optional ordering, skip, and limit.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnClause {
    /// Whether `DISTINCT` was specified.
    pub distinct: bool,
    /// Expressions to return.
    pub items: Vec<ReturnItem>,
    /// Optional `ORDER BY` items.
    pub order_by: Option<Vec<OrderItem>>,
    /// Optional `SKIP` expression.
    pub skip: Option<Expression>,
    /// Optional `LIMIT` expression.
    pub limit: Option<Expression>,
}

/// A single item in a `RETURN` or `WITH` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnItem {
    /// The expression to evaluate.
    pub expr: Expression,
    /// Optional `AS` alias.
    pub alias: Option<String>,
}

/// A single sort key in an `ORDER BY` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderItem {
    /// The expression to sort by.
    pub expr: Expression,
    /// True for `ASC` (default), false for `DESC`.
    pub ascending: bool,
}

/// A `CREATE` clause with a pattern to create.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateClause {
    /// The graph pattern to create.
    pub pattern: Pattern,
}

/// A `SET` clause with property assignments.
#[derive(Debug, Clone, PartialEq)]
pub struct SetClause {
    /// Property assignment items.
    pub items: Vec<SetItem>,
}

/// A single property assignment in a `SET` clause.
#[derive(Debug, Clone, PartialEq)]
pub enum SetItem {
    /// Set a property to a value: `target = value`.
    Property {
        /// The property access expression (e.g. `n.name`).
        target: Expression,
        /// The value expression.
        value: Expression,
    },
}

/// A `REMOVE` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoveClause {
    /// Items to remove.
    pub items: Vec<RemoveItem>,
}

/// A single item in a `REMOVE` clause.
#[derive(Debug, Clone, PartialEq)]
pub enum RemoveItem {
    /// Remove a property (e.g. `n.name`).
    Property(Expression),
    /// Remove a label from a node (e.g. `n:Label`).
    Label {
        /// Variable name.
        variable: String,
        /// Label to remove.
        label: String,
    },
}

/// A `DELETE` or `DETACH DELETE` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteClause {
    /// True if `DETACH DELETE` (also deletes relationships).
    pub detach: bool,
    /// Expressions identifying entities to delete.
    pub exprs: Vec<Expression>,
}

/// A `WITH` clause for intermediate result piping.
#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    /// Whether `DISTINCT` was specified.
    pub distinct: bool,
    /// Projected items.
    pub items: Vec<ReturnItem>,
    /// Optional `WHERE` filter.
    pub where_clause: Option<Expression>,
}

/// A `MERGE` clause with optional `ON MATCH` / `ON CREATE` actions.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeClause {
    /// The pattern to match or create.
    pub pattern: Pattern,
    /// `SET` items to apply when the pattern matches.
    pub on_match: Vec<SetItem>,
    /// `SET` items to apply when the pattern is created.
    pub on_create: Vec<SetItem>,
}

/// An `UNWIND` clause that expands a list into rows.
#[derive(Debug, Clone, PartialEq)]
pub struct UnwindClause {
    /// The list expression to unwind.
    pub expr: Expression,
    /// The variable name bound to each element.
    pub variable: String,
}

/// The target kind of a property index.
#[derive(Debug, Clone, PartialEq)]
pub enum IndexTarget {
    /// Index on a node label.
    NodeLabel(String),
    /// Index on a relationship type.
    RelationshipType(String),
}

/// `CREATE INDEX [name] ON :Label(property)` or `CREATE INDEX [name] ON :REL_TYPE(property)`
#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexClause {
    /// Optional index name.
    pub name: Option<String>,
    /// Label or relationship type the index applies to.
    pub target: IndexTarget,
    /// Property key the index covers.
    pub property: String,
}

/// DROP INDEX name
#[derive(Debug, Clone, PartialEq)]
pub struct DropIndexClause {
    /// Name of the index to drop.
    pub name: String,
}

// -- Patterns --

/// A graph pattern consisting of one or more comma-separated chains.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    /// Comma-separated pattern chains.
    pub chains: Vec<PatternChain>,
}

/// A single chain of alternating nodes and relationships.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternChain {
    /// Alternating node and relationship elements.
    pub elements: Vec<PatternElement>,
}

/// An element within a pattern chain.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    /// A node pattern (e.g. `(n:Person)`).
    Node(NodePattern),
    /// A relationship pattern (e.g. `-[:KNOWS]->`).
    Relationship(RelationshipPattern),
}

/// A node pattern: `(variable:Label {properties})`.
#[derive(Debug, Clone, PartialEq)]
pub struct NodePattern {
    /// Optional variable binding.
    pub variable: Option<String>,
    /// Zero or more labels.
    pub labels: Vec<String>,
    /// Optional inline property map.
    pub properties: Option<MapLiteral>,
}

/// A relationship pattern: `-[variable:TYPE {props}]->`.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipPattern {
    /// Optional variable binding.
    pub variable: Option<String>,
    /// Relationship type filters.
    pub rel_types: Vec<String>,
    /// Arrow direction.
    pub direction: RelDirection,
    /// Optional inline property map.
    pub properties: Option<MapLiteral>,
    /// Minimum hops for variable-length paths. None means regular 1-hop.
    pub min_hops: Option<u32>,
    /// Maximum hops for variable-length paths. None means unbounded (capped by planner).
    pub max_hops: Option<u32>,
}

/// Relationship arrow direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelDirection {
    /// `-[...]->`
    Outgoing,
    /// `<-[...]-`
    Incoming,
    /// `-[...]-`
    Undirected,
}

/// A map literal is a list of key-value pairs: `{key1: expr1, key2: expr2}`.
pub type MapLiteral = Vec<(String, Expression)>;

// -- Expressions --

/// An expression node in the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal value.
    Literal(Literal),
    /// A variable reference.
    Variable(String),
    /// Property access: `expr.prop`
    Property(Box<Expression>, String),
    /// Parameter reference: `$name`
    Parameter(String),
    /// Binary operation: `lhs op rhs`.
    BinaryOp(BinaryOp, Box<Expression>, Box<Expression>),
    /// Unary operation: `op expr`.
    UnaryOp(UnaryOp, Box<Expression>),
    /// Function call: `name(args)` or `name(DISTINCT args)`.
    FunctionCall {
        /// Function name.
        name: String,
        /// Whether `DISTINCT` was specified.
        distinct: bool,
        /// Function arguments.
        args: Vec<Expression>,
    },
    /// `expr IS [NOT] NULL` -- bool is true when IS NOT NULL
    IsNull(Box<Expression>, bool),
    /// `count(*)`
    CountStar,
    /// List literal: `[expr, expr, ...]`
    ListLiteral(Vec<Expression>),
    /// Temporal reference: `expr AT TIME expr` in hyperedge participant lists.
    #[cfg(feature = "hypergraph")]
    TemporalRef {
        /// The node expression.
        node: Box<Expression>,
        /// The timestamp expression.
        timestamp: Box<Expression>,
    },
}

/// A literal value in the AST.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal (e.g. `42`).
    Integer(i64),
    /// Floating-point literal (e.g. `3.14`).
    Float(f64),
    /// String literal (e.g. `'hello'`).
    String(String),
    /// Boolean literal (`true` / `false`).
    Bool(bool),
    /// `NULL` literal.
    Null,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `+` addition.
    Add,
    /// `-` subtraction.
    Sub,
    /// `*` multiplication.
    Mul,
    /// `/` division.
    Div,
    /// `%` modulus.
    Mod,
    /// `=` equality.
    Eq,
    /// `<>` / `!=` inequality.
    Neq,
    /// `<` less than.
    Lt,
    /// `<=` less than or equal.
    Lte,
    /// `>` greater than.
    Gt,
    /// `>=` greater than or equal.
    Gte,
    /// `AND` logical conjunction.
    And,
    /// `OR` logical disjunction.
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `NOT` logical negation.
    Not,
    /// `-` arithmetic negation.
    Neg,
}

// -- Subgraph Snapshot --

/// CREATE SNAPSHOT clause for materializing query results into a subgraph.
///
/// Syntax: CREATE SNAPSHOT (var:Label {props}) [AT TIME expr] FROM MATCH pattern [WHERE filter] RETURN items
#[cfg(feature = "subgraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSnapshotClause {
    /// Optional variable name for the snapshot subgraph.
    pub variable: Option<String>,
    /// Labels for the snapshot subgraph.
    pub labels: Vec<String>,
    /// Properties to set on the snapshot subgraph.
    pub properties: Option<MapLiteral>,
    /// Optional temporal anchor (AT TIME expr).
    pub temporal_anchor: Option<Expression>,
    /// The FROM MATCH clause defining the source pattern.
    pub from_match: MatchClause,
    /// The FROM RETURN items defining what to capture.
    pub from_return: Vec<ReturnItem>,
}

/// CREATE HYPEREDGE clause for creating a hyperedge connecting multiple sources and targets.
///
/// Syntax: CREATE HYPEREDGE (var:Label) FROM (expr, expr, ...) TO (expr, expr, ...)
#[cfg(feature = "hypergraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct CreateHyperedgeClause {
    /// Optional variable name for the hyperedge.
    pub variable: Option<String>,
    /// Labels (relationship types) for the hyperedge.
    pub labels: Vec<String>,
    /// Source participant expressions (FROM list).
    pub sources: Vec<Expression>,
    /// Target participant expressions (TO list).
    pub targets: Vec<Expression>,
}

/// MATCH HYPEREDGE clause for querying hyperedges.
///
/// Syntax: MATCH HYPEREDGE (var:Label)
#[cfg(feature = "hypergraph")]
#[derive(Debug, Clone, PartialEq)]
pub struct MatchHyperedgeClause {
    /// Optional variable name for the hyperedge.
    pub variable: Option<String>,
    /// Labels to filter by.
    pub labels: Vec<String>,
}
