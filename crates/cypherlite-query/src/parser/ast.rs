// AST node type definitions for openCypher subset

/// A complete Cypher query.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub clauses: Vec<Clause>,
}

/// Top-level clause types.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Match(MatchClause),
    Return(ReturnClause),
    Create(CreateClause),
    Set(SetClause),
    Remove(RemoveClause),
    Delete(DeleteClause),
    With(WithClause),
    Merge(MergeClause),
    Unwind(UnwindClause),
    CreateIndex(CreateIndexClause),
    DropIndex(DropIndexClause),
    #[cfg(feature = "subgraph")]
    CreateSnapshot(CreateSnapshotClause),
}

/// Temporal predicate for time-travel queries.
#[derive(Debug, Clone, PartialEq)]
pub enum TemporalPredicate {
    /// AT TIME <expr> -- find the version current at a specific point in time.
    AsOf(Expression),
    /// BETWEEN TIME <expr> AND <expr> -- find all versions within a time range.
    Between(Expression, Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    pub optional: bool,
    pub pattern: Pattern,
    pub temporal_predicate: Option<TemporalPredicate>,
    pub where_clause: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
    pub order_by: Option<Vec<OrderItem>>,
    pub skip: Option<Expression>,
    pub limit: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnItem {
    pub expr: Expression,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderItem {
    pub expr: Expression,
    pub ascending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateClause {
    pub pattern: Pattern,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetClause {
    pub items: Vec<SetItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetItem {
    Property {
        target: Expression,
        value: Expression,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoveClause {
    pub items: Vec<RemoveItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoveItem {
    Property(Expression),
    Label { variable: String, label: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteClause {
    pub detach: bool,
    pub exprs: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
    pub where_clause: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeClause {
    pub pattern: Pattern,
    pub on_match: Vec<SetItem>,
    pub on_create: Vec<SetItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnwindClause {
    pub expr: Expression,
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

/// CREATE INDEX [name] ON :Label(property) or CREATE INDEX [name] ON :REL_TYPE(property)
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

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub chains: Vec<PatternChain>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternChain {
    pub elements: Vec<PatternElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    Node(NodePattern),
    Relationship(RelationshipPattern),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<MapLiteral>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub rel_types: Vec<String>,
    pub direction: RelDirection,
    pub properties: Option<MapLiteral>,
    /// Minimum hops for variable-length paths. None means regular 1-hop.
    pub min_hops: Option<u32>,
    /// Maximum hops for variable-length paths. None means unbounded (capped by planner).
    pub max_hops: Option<u32>,
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Variable(String),
    /// Property access: `expr.prop`
    Property(Box<Expression>, String),
    /// Parameter reference: `$name`
    Parameter(String),
    BinaryOp(BinaryOp, Box<Expression>, Box<Expression>),
    UnaryOp(UnaryOp, Box<Expression>),
    FunctionCall {
        name: String,
        distinct: bool,
        args: Vec<Expression>,
    },
    /// `expr IS [NOT] NULL` -- bool is true when IS NOT NULL
    IsNull(Box<Expression>, bool),
    /// `count(*)`
    CountStar,
    /// List literal: `[expr, expr, ...]`
    ListLiteral(Vec<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
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
