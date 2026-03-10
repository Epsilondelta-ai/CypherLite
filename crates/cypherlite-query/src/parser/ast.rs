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
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    pub optional: bool,
    pub pattern: Pattern,
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
