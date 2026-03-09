# CypherLite Query Engine Design

**Version:** 1.0
**Date:** 2026-03-10
**Status:** Design Specification

## Executive Summary

This document specifies the complete design of the CypherLite query execution engine, including parsing, logical planning, physical execution, optimization, and runtime. CypherLite targets a minimal but practical subset of Cypher that covers the most common query patterns for agent memory systems, knowledge graph applications, and basic graph analytics.

The architecture follows a classical database compiler pipeline: Lexer → Parser → AST → Logical Plan → Physical Plan → Execution. For v1.0, we prioritize correctness and simplicity over advanced optimizations, with clear extension points for future enhancements.

---

## 1. Cypher Subset for v1.0

### 1.1 Supported Clauses

CypherLite v1.0 supports the following clauses in combination:

#### Read Clauses
- **MATCH**: Pattern matching to find graph structures
- **OPTIONAL MATCH**: Left outer join semantics for optional patterns
- **WHERE**: Filtering predicates applied to patterns
- **RETURN**: Output specification with projection and aggregation
- **ORDER BY**: Result sorting by expressions (ASC/DESC)
- **SKIP**: Skip first N results
- **LIMIT**: Restrict result count
- **DISTINCT**: Remove duplicate results from output
- **WITH**: Intermediate result transformation and filtering (multi-stage queries)
- **UNWIND**: Expand collections into multiple rows

#### Write Clauses
- **CREATE**: Insert new nodes and relationships
- **MERGE**: Upsert operation (MATCH or CREATE)
  - Includes ON CREATE and ON MATCH subclauses
- **SET**: Update node/relationship properties and add labels
- **DELETE**: Remove nodes (only orphaned nodes)
- **DETACH DELETE**: Remove nodes and their relationships
- **REMOVE**: Drop properties and labels from entities

#### Set Operations
- **UNION**: Combine results from multiple queries (UNION ALL supported, UNION DISTINCT future)

#### Graph Traversal
- **Variable-length paths**: `-[*]->``, `-[*2..4]->`, `-[*..3]->`
- **Shortest path**: Not in v1.0 (requires specialized algorithm)

### 1.2 Supported Expressions

#### Comparison Operators
- Equality: `=`, `<>`
- Relational: `<`, `<=`, `>`, `>=`
- String contains: `CONTAINS`
- String starts/ends: `STARTS WITH`, `ENDS WITH`
- Pattern matching: `=~` (regex, basic support)
- Null checks: `IS NULL`, `IS NOT NULL`

#### Logical Operators
- AND, OR, NOT
- Short-circuit evaluation (NOT fully implemented in v1.0, TBD)

#### Arithmetic Operators
- `+`, `-`, `*`, `/`, `%`
- String concatenation: `+` (type-dependent)
- List concatenation: `+`

#### String Operations
- String functions: `UPPER`, `LOWER`, `TRIM`, `LTRIM`, `RTRIM`
- String info: `LENGTH`, `SUBSTRING`, `REPLACE`
- Case sensitivity flags in pattern matching

#### List Operations
- Indexing: `list[0]`, `list[-1]`
- Slicing: `list[1..3]`, `list[1..]`, `list[..5]`
- Membership: `IN`
- Length: `SIZE()`, `LENGTH()`

#### NULL Handling
- Three-valued logic (TRUE, FALSE, NULL)
- NULL coalescing: `COALESCE(expr1, expr2, ...)`
- NULL-safe operators where applicable
- Aggregation functions skip NULL values by default

#### Type Expressions
- `TYPE(rel)`: Get relationship type
- `LABELS(node)`: Get node labels
- `PROPERTIES(entity)`: Get all properties as map

### 1.3 Supported Functions

#### Aggregation Functions
- `COUNT(expr)`: Count non-null values
- `COUNT(DISTINCT expr)`: Count unique values
- `COUNT(*)`: Count all rows including nulls
- `SUM(expr)`: Sum numeric values (ignores NULL)
- `AVG(expr)`: Average numeric values
- `MIN(expr)`: Minimum value (comparable types)
- `MAX(expr)`: Maximum value (comparable types)
- `COLLECT(expr)`: Aggregate into list
- `COLLECT(DISTINCT expr)`: Unique values in list

Implicit GROUP BY: Non-aggregated expressions in RETURN become grouping keys.

#### String Functions
- `UPPER(str)`, `LOWER(str)`
- `LTRIM(str)`, `RTRIM(str)`, `TRIM(str)`
- `SUBSTRING(str, start [, length])`
- `REPLACE(str, find, replace)`
- `LENGTH(str)`, `SIZE(str)`
- `SPLIT(str, delimiter)`
- `CONTAINS(str, substring)`
- `STARTS WITH(str, prefix)`, `ENDS WITH(str, suffix)`

#### Mathematical Functions
- `ABS(n)`, `CEIL(n)`, `FLOOR(n)`, `ROUND(n [, precision])`
- `SQRT(n)`, `POW(n, exp)`
- `SIGN(n)`: Returns -1, 0, 1
- `MOD(n, divisor)`, `RAND()`, `PI()`, `E()`

#### List Functions
- `HEAD(list)`, `TAIL(list)`
- `REVERSE(list)`
- `RANGE(start, end [, step])`
- `FLATTEN(list_of_lists)`

#### Entity Functions
- `ID(entity)`: Get internal node/relationship ID
- `TYPE(relationship)`: Get relationship type
- `LABELS(node)`: Get node labels as list
- `PROPERTIES(entity)`: Get all properties as map
- `KEYS(map)`: Get map keys

#### Path Functions
- `LENGTH(path)`: Number of relationships in path
- `NODES(path)`: List of nodes in path
- `RELATIONSHIPS(path)`: List of relationships in path
- `LAST(path)`: Last node in path
- `EXTRACT(x IN path | x.prop)`: Project path elements (alternative to NODES/RELATIONSHIPS)

#### Scalar Functions
- `COALESCE(expr1, expr2, ...)`: Return first non-null value
- `CASE WHEN ... THEN ... ELSE ... END`: Conditional expressions
- `TIMESTAMP()`: Current Unix timestamp in milliseconds
- `DATETIME([ iso8601_string ])`: Parse or get current datetime (basic)
- `ID()`: Entity internal identifier (deterministic per session)

### 1.4 Pattern Matching

#### Node Patterns
- `(n)`: Variable node
- `(n:Label)`: Labeled node
- `(n:Label1:Label2)`: Multiple labels
- `(n:Label {prop: value, age: 25})`: Properties with literals
- `(n {name: $paramName})`: Property with parameter binding
- `()`: Anonymous node (only in patterns where binding not needed)

#### Relationship Patterns
- `-[r]->`: Directed relationship with variable
- `-[:TYPE]->`: Typed relationship
- `-[r:TYPE]->`: Both variable and type
- `-[r:TYPE {since: 2020}]->`: Properties on relationships
- `-[r:TYPE1|TYPE2]->`: Multiple types (v1.0: single type only)
- `--`: Undirected (same semantics as treating both directions)
- `-[*]->`: Variable-length (0 or more)
- `-[*2..4]->`: Variable-length with bounds
- `-[*..3]->`: Upper bound only

**v1.0 Limitation:** Only single relationship type per pattern. Multiple type matching via separate MATCH clauses or manual UNION.

#### Path Binding
- `p = (n)-[:REL]->(m)`: Bind entire path to variable
- Used for `length(p)`, `nodes(p)`, `relationships(p)`

#### Label Expressions (Future)
Not supported in v1.0. Alternative: Use WHERE clauses for label filtering.
```cypher
-- v1.0 workaround for label filtering
MATCH (n) WHERE n:Person OR n:Agent RETURN n
-- Not supported in v1.0:
-- MATCH (n:Person|Agent) RETURN n
```

### 1.5 Not in v1.0 (and Rationale)

#### Complex Clause Combinations
- **CALL** (procedure invocation): Requires plugin architecture; future extension
- **FOREACH**: Imperative iteration; complex state management
- **REDUCE**: Functional aggregation; complex semantics
- **IN subqueries**: Requires query nesting; future enhancement
- **EXISTS subqueries**: Predicate existence check; deferred

#### Advanced Pattern Features
- **Label expressions**: `(n:Person&!Inactive|Agent)` - requires expression evaluation in pattern context
- **Multiple relationship types in single pattern**: `-[r:TYPE1|TYPE2]->` - parser support only, full semantics deferred
- **Relationship properties in variable-length paths**: Ambiguous semantics on which rel property to access
- **Quantified path patterns**: `(n)-->+(m)` or similar - non-standard syntax

#### Advanced Operators
- **Shortest path functions**: `shortestPath()`, `allShortestPaths()` - requires Dijkstra/BFS specialized operator
- **Graph algorithms**: Degree, betweenness, closeness - future APOC-like procedure layer
- **Full-text search**: Requires indexing layer; future feature
- **Regex groups and captures**: Partial support only (basic matching works)

#### Advanced Expressions
- **Map projections**: `RETURN {prop: n.prop, name: n.name}` - deferred
- **List comprehensions**: `[x IN list WHERE x > 5 | x * 2]` - functional syntax, deferred
- **Window functions**: `ROW_NUMBER()`, `RANK()`, etc. - SQL-style, not in v1.0

#### Temporal Features
- **AT TIME syntax**: Requires temporal indexing and storage layer
- **BETWEEN temporal queries**: Future (see Section 6)
- **Temporal constraints in WHERE**: Future

#### RDF Features
- **Named graph queries**: Future (see Section 7)
- **SPARQL translation**: Future bridge layer

---

## 2. Lexer & Parser Design

### 2.1 Token Types Enumeration

The lexer converts the input Cypher string into a flat stream of tokens. Each token has:
- **Type**: Enumerated classification
- **Value**: String or parsed literal
- **Position**: Line and column for error reporting

```rust
// Pseudocode: Token types
enum TokenType {
    // Literals
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    BoolLiteral(bool),
    NullLiteral,

    // Keywords (case-insensitive)
    Match,
    OptionalMatch,
    Where,
    Return,
    Create,
    Merge,
    Set,
    Delete,
    DetachDelete,
    Remove,
    With,
    Unwind,
    Union,
    UnionAll,
    OrderBy,
    Asc,
    Desc,
    Skip,
    Limit,
    Distinct,
    OnCreate,
    OnMatch,

    // Operators
    Arrow,              // ->
    LeftArrow,          // <-
    Dash,               // --
    Pipe,               // |

    // Syntax
    LParen,             // (
    RParen,             // )
    LBracket,           // [
    RBracket,           // ]
    LBrace,             // {
    RBrace,             // }

    Comma,
    Colon,
    Dot,
    Asterisk,           // * (also variable-length marker)

    // Comparison
    Equals,             // =
    NotEquals,          // <>
    LessThan,           // <
    LessThanOrEqual,    // <=
    GreaterThan,        // >
    GreaterThanOrEqual, // >=
    Regex,              // =~

    // Logical
    And,
    Or,
    Not,

    // Arithmetic
    Plus,
    Minus,
    Div,                // /
    Percent,            // %

    // Special
    Dollar,             // $ (parameters)
    Variable(String),
    Identifier(String),

    // Functions (recognized by context)
    // Actual function names parsed as identifiers, type determined by parser

    Eof,
}
```

### 2.2 Grammar Specification

CypherLite uses a PEG (Parsing Expression Grammar) style parser, implemented as a hand-coded recursive descent parser in Rust. This allows flexibility and clear error reporting.

```
// High-level grammar overview (simplified EBNF)

Query = (MATCH | CREATE | MERGE | WITH | UNWIND)*
         (RETURN | CREATE | SET | DELETE | DETACH DELETE | REMOVE)
         (ORDER_BY)?
         (SKIP)?
         (LIMIT)?

MATCH = "MATCH" PatternList (WHERE Condition)?

OptionalMatch = "OPTIONAL" "MATCH" PatternList (WHERE Condition)?

WHERE = "WHERE" Expression

RETURN = "RETURN" (DISTINCT)? OutputList (ORDER_BY)? (SKIP)? (LIMIT)?

OutputList = OutputItem ("," OutputItem)*
OutputItem = Expression (AS Identifier)?

PatternList = Pattern ("," Pattern)*

Pattern = PatternElement (PatternConnection PatternElement)*
PatternElement = NodePattern | Variable
PatternConnection = "-" "[" RelPattern "]" "->" | ...
NodePattern = "(" Variable? (":" Label)* ("{" PropertyMap "}")? ")"
RelPattern = Variable? (":" RelType)? ("{" PropertyMap "}")?

Expression = OrExpression
OrExpression = AndExpression ("OR" AndExpression)*
AndExpression = ComparisonExpression ("AND" ComparisonExpression)*
ComparisonExpression = ArithmeticExpression (CompOp ArithmeticExpression)?
ArithmeticExpression = MultiplicativeExpression (("+" | "-") MultiplicativeExpression)*
MultiplicativeExpression = UnaryExpression (("*" | "/" | "%") UnaryExpression)*
UnaryExpression = ("NOT")? PrimaryExpression
PrimaryExpression = Atom | FunctionCall | CaseExpression | ...
Atom = Literal | Variable | Property | "(" Expression ")"

Property = Atom "." Identifier | Atom "[" Expression "]"

FunctionCall = Identifier "(" (Expression ("," Expression)*)? ")"

CREATE = "CREATE" PatternList
MERGE = "MERGE" Pattern (OnCreateClause)? (OnMatchClause)?
SET = "SET" SetItem ("," SetItem)*
DELETE = "DELETE" Identifier ("," Identifier)*
DETACH = "DETACH" "DELETE" Identifier ("," Identifier)*
REMOVE = "REMOVE" RemoveItem ("," RemoveItem)*
WITH = "WITH" OutputList (WHERE Condition)? (ORDER_BY)? (SKIP)? (LIMIT)?
UNWIND = "UNWIND" Expression "AS" Identifier
UNION = "UNION" Query | "UNION" "ALL" Query
```

### 2.3 AST Node Types

The parser produces an Abstract Syntax Tree with the following node types:

```rust
// Pseudocode: Core AST Nodes

pub enum AstNode {
    // Top-level
    Query(QueryNode),

    // Clauses
    Match(MatchClause),
    OptionalMatch(OptionalMatchClause),
    Where(WhereClause),
    Return(ReturnClause),
    With(WithClause),
    Create(CreateClause),
    Merge(MergeClause),
    Set(SetClause),
    Delete(DeleteClause),
    DetachDelete(DetachDeleteClause),
    Remove(RemoveClause),
    Unwind(UnwindClause),
    Union(UnionClause),
    OrderBy(OrderByClause),
    Skip(SkipClause),
    Limit(LimitClause),

    // Patterns
    Pattern(PatternNode),
    NodePattern(NodePatternNode),
    RelationshipPattern(RelPatternNode),
    PathPattern(PathPatternNode),

    // Expressions
    Expression(ExpressionNode),

    // Literals and Identifiers
    Literal(LiteralNode),
    Variable(String),
    Identifier(String),
}

pub struct QueryNode {
    pub clauses: Vec<AstNode>,  // Sequence of MATCH, WITH, RETURN, etc.
}

pub struct MatchClause {
    pub optional: bool,
    pub patterns: Vec<PatternNode>,
    pub where_clause: Option<Box<ExpressionNode>>,
}

pub struct PatternNode {
    pub elements: Vec<PatternElement>,  // Alternates: Node-Rel-Node-Rel-Node...
}

pub enum PatternElement {
    Node(NodePatternNode),
    Relationship(RelPatternNode),
}

pub struct NodePatternNode {
    pub variable: Option<String>,
    pub labels: Vec<String>,          // ["Person", "Employee"]
    pub properties: Option<Vec<(String, ExpressionNode)>>,  // name: "Alice", age: 30
}

pub struct RelPatternNode {
    pub variable: Option<String>,
    pub rel_type: Option<String>,     // Single type only in v1.0
    pub properties: Option<Vec<(String, ExpressionNode)>>,
    pub direction: Direction,          // Outgoing, Incoming, Undirected
    pub variable_length: Option<(Option<u32>, Option<u32>)>,  // (min, max) for [*m..n]
}

pub enum Direction {
    Outgoing,    // -->
    Incoming,    // <--
    Undirected,  // --
}

pub struct WhereClause {
    pub expression: Box<ExpressionNode>,
}

pub struct ReturnClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
}

pub struct ReturnItem {
    pub expression: Box<ExpressionNode>,
    pub alias: Option<String>,  // AS name
}

pub enum ExpressionNode {
    // Binary operations
    BinaryOp {
        op: BinaryOperator,
        left: Box<ExpressionNode>,
        right: Box<ExpressionNode>,
    },

    // Unary operations
    UnaryOp {
        op: UnaryOperator,
        operand: Box<ExpressionNode>,
    },

    // Function call
    FunctionCall {
        name: String,
        args: Vec<ExpressionNode>,
        distinct: bool,  // For COUNT(DISTINCT ...)
    },

    // Property access
    Property {
        entity: Box<ExpressionNode>,
        property: String,  // node.name, rel.since
    },

    // List/map indexing
    Index {
        collection: Box<ExpressionNode>,
        index: Box<ExpressionNode>,  // Can be expr for slice ranges
    },

    // Case expression
    Case {
        operand: Option<Box<ExpressionNode>>,  // CASE expr WHEN ...
        alternatives: Vec<(ExpressionNode, ExpressionNode)>,  // (condition, result)
        default: Option<Box<ExpressionNode>>,
    },

    // Literals
    Literal(LiteralNode),

    // Variable reference
    Variable(String),

    // Parenthesized expression
    Parens(Box<ExpressionNode>),
}

pub enum BinaryOperator {
    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Contains,
    StartsWith,
    EndsWith,
    Regex,
    In,

    // Logical
    And,
    Or,

    // Arithmetic
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
}

pub enum UnaryOperator {
    Not,
    UnaryMinus,
    UnaryPlus,
}

pub enum LiteralNode {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
    List(Vec<ExpressionNode>),
    Map(Vec<(String, ExpressionNode)>),  // {key: value, ...}
}

pub struct SetClause {
    pub items: Vec<SetItem>,
}

pub enum SetItem {
    Property {
        entity: String,      // Variable name
        property: String,
        value: ExpressionNode,
    },
    Label {
        entity: String,
        label: String,
    },
}

pub struct CreateClause {
    pub patterns: Vec<PatternNode>,
}

pub struct MergeClause {
    pub pattern: PatternNode,
    pub on_create: Option<Vec<SetItem>>,  // ON CREATE SET ...
    pub on_match: Option<Vec<SetItem>>,   // ON MATCH SET ...
}

pub struct UnwindClause {
    pub expression: Box<ExpressionNode>,
    pub variable: String,
}

pub struct WithClause {
    pub items: Vec<ReturnItem>,
    pub where_clause: Option<Box<ExpressionNode>>,
    pub order_by: Option<OrderByClause>,
    pub skip: Option<u64>,
    pub limit: Option<u64>,
}

pub struct OrderByClause {
    pub items: Vec<(ExpressionNode, SortDirection)>,
}

pub enum SortDirection {
    Ascending,
    Descending,
}
```

### 2.4 Example: Query Compilation to AST

Consider the query:
```cypher
MATCH (a:Person)-[:KNOWS]->(b)
WHERE a.age > 25
RETURN a.name, b.name
```

Compilation steps:

1. **Lexical Analysis (Tokenization)**:
   ```
   Token(Match, "MATCH")
   Token(LParen, "(")
   Token(Variable, "a")
   Token(Colon, ":")
   Token(Identifier, "Person")
   Token(RParen, ")")
   Token(Dash, "-")
   Token(LBracket, "[")
   Token(Colon, ":")
   Token(Identifier, "KNOWS")
   Token(RBracket, "]")
   Token(Arrow, "->")
   Token(LParen, "(")
   Token(Variable, "b")
   Token(RParen, ")")
   Token(Where, "WHERE")
   Token(Variable, "a")
   Token(Dot, ".")
   Token(Identifier, "age")
   Token(GreaterThan, ">")
   Token(IntLiteral, 25)
   Token(Return, "RETURN")
   ...
   Token(Eof)
   ```

2. **Syntax Analysis (Parsing to AST)**:
   ```
   QueryNode {
     clauses: [
       MatchClause {
         optional: false,
         patterns: [
           PatternNode {
             elements: [
               PatternElement::Node(NodePatternNode {
                 variable: Some("a"),
                 labels: ["Person"],
                 properties: None,
               }),
               PatternElement::Relationship(RelPatternNode {
                 variable: None,
                 rel_type: Some("KNOWS"),
                 properties: None,
                 direction: Outgoing,
                 variable_length: None,
               }),
               PatternElement::Node(NodePatternNode {
                 variable: Some("b"),
                 labels: [],
                 properties: None,
               }),
             ]
           }
         ],
         where_clause: Some(
           BinaryOp {
             op: GreaterThan,
             left: Property {
               entity: Variable("a"),
               property: "age",
             },
             right: Literal(Integer(25)),
           }
         ),
       },
       ReturnClause {
         distinct: false,
         items: [
           ReturnItem {
             expression: Property {
               entity: Variable("a"),
               property: "name",
             },
             alias: None,
           },
           ReturnItem {
             expression: Property {
               entity: Variable("b"),
               property: "name",
             },
             alias: None,
           },
         ],
       },
     ]
   }
   ```

3. **Semantic Analysis** (scope resolution, type inference):
   - Verify that `a` and `b` are bound in MATCH before use in RETURN
   - Infer types: `a` and `b` are nodes, `a.age` is comparable to 25 (numeric)
   - Check that functions exist and have correct arity
   - Validate that property access is on valid entities

4. **Output**: Validated AST ready for logical planning

---

## 3. Logical Plan

The logical plan is an intermediate representation that expresses the query semantically without implementation details. It consists of a tree of logical operators.

### 3.1 Logical Operators

```rust
pub enum LogicalOperator {
    // Scan operators
    Scan {
        entity_type: EntityType,      // Node or Relationship
        label: Option<String>,         // Filter by label/type
        properties: Vec<PropertyFilter>,  // Property predicates
        variable: String,              // Output variable name
    },

    // Pattern matching
    Expand {
        from_var: String,              // Source node variable
        rel_var: Option<String>,       // Relationship variable
        to_var: String,                // Target node variable
        rel_type: Option<String>,      // Relationship type
        direction: Direction,
        properties: Vec<PropertyFilter>,
        variable_length: Option<(Option<u32>, Option<u32>)>,
    },

    // Filtering
    Filter {
        input: Box<LogicalPlan>,
        predicate: ExpressionNode,
    },

    // Projection
    Project {
        input: Box<LogicalPlan>,
        expressions: Vec<(ExpressionNode, Option<String>)>,  // (expr, alias)
    },

    // Aggregation
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<ExpressionNode>,  // Non-aggregated expressions
        aggregates: Vec<(String, AggregateFunction)>,  // (alias, function)
    },

    // Sorting
    Sort {
        input: Box<LogicalPlan>,
        order_by: Vec<(ExpressionNode, SortDirection)>,
    },

    // Pagination
    Skip {
        input: Box<LogicalPlan>,
        count: u64,
    },

    Limit {
        input: Box<LogicalPlan>,
        count: u64,
    },

    // Set operations
    Union {
        inputs: Vec<LogicalPlan>,
        all: bool,  // UNION ALL vs UNION DISTINCT
    },

    // Optional matching (left outer join)
    Optional {
        input: Box<LogicalPlan>,
        optional_patterns: Vec<LogicalPlan>,
    },

    // Join for multiple patterns
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        on: Vec<(String, String)>,  // (left_var, right_var) matching
        join_type: JoinType,
    },

    // Write operations
    Create {
        patterns: Vec<PatternNode>,
        input: Option<Box<LogicalPlan>>,  // For CREATE with pattern from input
    },

    Update {
        input: Box<LogicalPlan>,
        updates: Vec<UpdateAction>,
    },

    Delete {
        input: Box<LogicalPlan>,
        variables: Vec<String>,
        detach: bool,  // DETACH DELETE
    },

    // UNWIND
    Unwind {
        input: Box<LogicalPlan>,
        expression: ExpressionNode,
        variable: String,
    },

    // WITH (intermediate result transformation)
    With {
        input: Box<LogicalPlan>,
        output_items: Vec<(ExpressionNode, Option<String>)>,
        filters: Vec<ExpressionNode>,  // WHERE in WITH clause
        next: Box<LogicalPlan>,        // Remaining query
    },
}

pub enum EntityType {
    Node,
    Relationship,
}

pub struct PropertyFilter {
    pub property: String,
    pub predicate: ExpressionNode,
}

pub enum AggregateFunction {
    Count { distinct: bool, all_rows: bool },  // COUNT(*) vs COUNT(expr) vs COUNT(DISTINCT)
    Sum,
    Avg,
    Min,
    Max,
    Collect { distinct: bool },
}

pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

pub enum UpdateAction {
    SetProperty { variable: String, property: String, value: ExpressionNode },
    AddLabel { variable: String, label: String },
    RemoveProperty { variable: String, property: String },
    RemoveLabel { variable: String, label: String },
}

pub struct LogicalPlan {
    pub operator: LogicalOperator,
}
```

### 3.2 Logical Plan Tree Structure

A logical plan is a tree (or DAG) where:
- Each node is a logical operator
- Edges represent data flow (inputs to operator)
- Leaves are scan/source operators
- Root is the final output operator

**Tree composition rules:**
- Sources (Scan) have no inputs
- Unary operators (Filter, Project, Sort, Limit, etc.) have one input
- Binary operators (Join) have two inputs
- N-ary operators (Union, Create with input) can have multiple inputs

### 3.3 Example Logical Plans

#### Example 1: Simple MATCH-WHERE-RETURN

Query:
```cypher
MATCH (a:Person)-[:KNOWS]->(b)
WHERE a.age > 25
RETURN a.name, b.name
```

Logical Plan:
```
Project(
  input=Filter(
    input=Join(
      left=Scan(entity_type=Node, label="Person", variable="a"),
      right=Scan(entity_type=Node, variable="b"),
      on=[],  // Connected via Expand, not here
    ),
    predicate=(a.age > 25)
  ),
  expressions=[(a.name, None), (b.name, None)]
)

// Alternative representation with Expand:
Project(
  input=Filter(
    input=Expand(
      from_var="a",
      rel_type="KNOWS",
      to_var="b",
      direction=Outgoing,
    ),
    predicate=(a.age > 25)
  ),
  expressions=[(a.name, None), (b.name, None)]
)
```

**Execution semantics:**
1. Scan for nodes labeled "Person", bind to `a`
2. For each `a`, expand relationship KNOWS to find connected `b` nodes
3. Filter rows where `a.age > 25`
4. Project columns: `a.name`, `b.name`

#### Example 2: Aggregation with GROUP BY

Query:
```cypher
MATCH (p:Person)-[:KNOWS]->(f:Person)
RETURN p.name, COUNT(f) AS friend_count
```

Logical Plan:
```
Aggregate(
  input=Expand(
    from_var="p",
    rel_type="KNOWS",
    to_var="f",
    direction=Outgoing,
  ),
  group_by=[p.name],
  aggregates=[("friend_count", Count{distinct=false, all_rows=false})]
)
```

**Execution:**
1. Scan Person nodes as `p`
2. Expand KNOWS relationships to `f` nodes
3. Group rows by `p.name`
4. For each group, count `f` nodes → friend_count
5. Output: (p.name, friend_count)

#### Example 3: OPTIONAL MATCH

Query:
```cypher
MATCH (a:Person)
OPTIONAL MATCH (a)-[:KNOWS]->(b)
RETURN a.name, b.name
```

Logical Plan:
```
Project(
  input=Optional(
    input=Scan(entity_type=Node, label="Person", variable="a"),
    optional_patterns=[
      Expand(
        from_var="a",
        rel_type="KNOWS",
        to_var="b",
        direction=Outgoing,
      )
    ]
  ),
  expressions=[(a.name, None), (b.name, None)]
)
```

**Execution:**
1. Scan all Person nodes as `a`
2. For each `a`, try to find KNOWS relationships
3. If found, pair `a` with `b`; if not found, pair `a` with NULL
4. Project: (a.name, b.name) — b.name is NULL when no match

#### Example 4: WITH (intermediate transformation)

Query:
```cypher
MATCH (a:Person)
WITH a, COUNT(*) AS cnt
WHERE cnt > 5
MATCH (a)-[:KNOWS]->(b)
RETURN a.name, b.name
```

Logical Plan:
```
Project(
  input=Expand(
    from_var="a",
    rel_type="KNOWS",
    to_var="b",
    direction=Outgoing,
    input=Filter(
      input=With(
        input=Scan(entity_type=Node, label="Person", variable="a"),
        output_items=[(a, Some("a")), (Count(*), Some("cnt"))],
        filters=[cnt > 5],
      ),
      predicate=(cnt > 5)
    )
  ),
  expressions=[(a.name, None), (b.name, None)]
)
```

---

## 4. Physical Plan & Operators

The physical plan maps logical operators to concrete execution algorithms and data structures. It includes cost estimates and specific index usage decisions.

### 4.1 Physical Operators Mapping

```rust
pub enum PhysicalOperator {
    // Scan operators
    TableScan {
        entity_type: EntityType,
        output: String,
    },
    IndexScan {
        entity_type: EntityType,
        label: Option<String>,
        index: String,
        predicate: ExpressionNode,
        output: String,
    },

    // Join operators
    NestedLoopJoin {
        left: Box<PhysicalPlan>,
        right: Box<PhysicalPlan>,
        on: Vec<(String, String)>,
    },
    HashJoin {
        left: Box<PhysicalPlan>,
        right: Box<PhysicalPlan>,
        left_keys: Vec<ExpressionNode>,
        right_keys: Vec<ExpressionNode>,
    },

    // Expand (traverse relationships)
    Expand {
        input: Box<PhysicalPlan>,
        from_var: String,
        rel_type: Option<String>,
        to_var: String,
        direction: Direction,
        variable_length: Option<(Option<u32>, Option<u32>)>,
    },

    // Filtering
    Filter {
        input: Box<PhysicalPlan>,
        predicate: ExpressionNode,
    },

    // Projection
    Project {
        input: Box<PhysicalPlan>,
        expressions: Vec<(ExpressionNode, Option<String>)>,
    },

    // Aggregation with specific algorithm
    HashAggregate {
        input: Box<PhysicalPlan>,
        group_by: Vec<ExpressionNode>,
        aggregates: Vec<(String, AggregateFunction)>,
    },
    StreamAggregate {
        input: Box<PhysicalPlan>,
        group_by: Vec<ExpressionNode>,
        aggregates: Vec<(String, AggregateFunction)>,
    },

    // Sorting
    Sort {
        input: Box<PhysicalPlan>,
        order_by: Vec<(ExpressionNode, SortDirection)>,
    },

    // Pagination
    Skip {
        input: Box<PhysicalPlan>,
        count: u64,
    },
    Limit {
        input: Box<PhysicalPlan>,
        count: u64,
    },

    // Set operations
    Union {
        inputs: Vec<PhysicalPlan>,
        all: bool,
    },

    // Write operations
    Create {
        input: Option<Box<PhysicalPlan>>,
        patterns: Vec<PatternNode>,
    },
    Update {
        input: Box<PhysicalPlan>,
        updates: Vec<UpdateAction>,
    },
    Delete {
        input: Box<PhysicalPlan>,
        variables: Vec<String>,
        detach: bool,
    },
}

pub struct PhysicalPlan {
    pub operator: PhysicalOperator,
    pub estimated_rows: u64,
    pub estimated_cost: f64,  // In arbitrary cost units
}
```

### 4.2 Iterator Model (Volcano/Pull-Based)

CypherLite uses the Volcano iterator model: each operator implements `next()` which returns one result tuple at a time.

```rust
pub trait Operator {
    /// Initialize the operator (called once before iteration)
    fn open(&mut self) -> Result<()>;

    /// Return the next result tuple, or None if iteration is exhausted
    fn next(&mut self) -> Result<Option<Tuple>>;

    /// Cleanup (called once after iteration or on error)
    fn close(&mut self) -> Result<()>;
}

// Tuple representation
pub type Tuple = HashMap<String, Value>;  // Variable name -> value

pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Node(NodeValue),
    Relationship(RelationshipValue),
    Path(PathValue),
}

pub struct NodeValue {
    pub id: u64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Value>,
}

pub struct RelationshipValue {
    pub id: u64,
    pub rel_type: String,
    pub from_node_id: u64,
    pub to_node_id: u64,
    pub properties: HashMap<String, Value>,
}

pub struct PathValue {
    pub nodes: Vec<NodeValue>,
    pub relationships: Vec<RelationshipValue>,
}

// Example implementation
pub struct ScanOperator {
    table: Arc<NodeTable>,  // Reference to node storage
    iterator: Option<NodeIterator>,
    output_var: String,
}

impl Operator for ScanOperator {
    fn open(&mut self) -> Result<()> {
        self.iterator = Some(self.table.iter());
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Tuple>> {
        if let Some(ref mut iter) = self.iterator {
            if let Some(node) = iter.next() {
                let mut tuple = HashMap::new();
                tuple.insert(self.output_var.clone(), Value::Node(node.into()));
                return Ok(Some(tuple));
            }
        }
        Ok(None)
    }

    fn close(&mut self) -> Result<()> {
        self.iterator = None;
        Ok(())
    }
}
```

### 4.3 Join Algorithms

For multi-pattern matching, CypherLite chooses between join algorithms:

#### Nested Loop Join (NLJ)
- **When to use:** When one side is small or highly selective
- **Cost:** O(N × M) where N is left cardinality, M is right cardinality
- **Implementation:** For each tuple from left, scan right input and check join condition

```rust
pub struct NestedLoopJoin {
    left: Box<dyn Operator>,
    right: Box<dyn Operator>,
    join_condition: Expression,
}

impl Operator for NestedLoopJoin {
    fn open(&mut self) -> Result<()> {
        self.left.open()?;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Tuple>> {
        loop {
            // Current tuple from left
            if self.current_left.is_none() {
                if let Some(left_tuple) = self.left.next()? {
                    self.current_left = Some(left_tuple);
                    self.right.open()?;
                } else {
                    return Ok(None);  // Exhausted left input
                }
            }

            // Try to find a matching tuple from right
            if let Some(right_tuple) = self.right.next()? {
                let left = self.current_left.as_ref().unwrap();
                if self.join_condition.evaluate(&left, &right_tuple)? {
                    let mut result = left.clone();
                    result.extend(right_tuple);
                    return Ok(Some(result));
                }
            } else {
                // Right exhausted for current left tuple
                self.right.close()?;
                self.current_left = None;
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.right.close()?;
        self.left.close()?;
        Ok(())
    }
}
```

#### Hash Join
- **When to use:** When both sides are large and join key is equijoin
- **Cost:** O(N + M) after hashing
- **Implementation:**
  1. Hash all tuples from left on join key
  2. For each tuple from right, probe hash table
  3. Emit matching pairs

```rust
pub struct HashJoin {
    left: Box<dyn Operator>,
    right: Box<dyn Operator>,
    left_key: Expression,
    right_key: Expression,
    hash_table: HashMap<Value, Vec<Tuple>>,
}

impl Operator for HashJoin {
    fn open(&mut self) -> Result<()> {
        self.left.open()?;

        // Build phase: hash all left tuples
        while let Some(tuple) = self.left.next()? {
            let key = self.left_key.evaluate(&tuple)?;
            self.hash_table.entry(key).or_insert_with(Vec::new).push(tuple);
        }
        self.left.close()?;

        self.right.open()?;
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Tuple>> {
        loop {
            // If we have matches from current right tuple, emit them
            if !self.current_matches.is_empty() {
                if let Some(left_tuple) = self.current_matches.pop() {
                    let right = self.current_right.as_ref().unwrap().clone();
                    let mut result = left_tuple;
                    result.extend(right.clone());
                    return Ok(Some(result));
                }
            }

            // Get next right tuple
            if let Some(right_tuple) = self.right.next()? {
                let key = self.right_key.evaluate(&right_tuple)?;
                self.current_right = Some(right_tuple);
                self.current_matches = self.hash_table.get(&key).cloned().unwrap_or_default();
            } else {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> Result<()> {
        self.right.close()?;
        Ok(())
    }
}
```

### 4.4 Index Scan vs Full Scan Decision

The physical planner decides whether to use an index:

```rust
fn choose_scan_strategy(
    entity_type: EntityType,
    label: Option<&str>,
    predicates: &[PropertyFilter],
    catalog: &Catalog,
) -> ScanStrategy {
    // Check if an index exists for the label
    if let Some(label) = label {
        if let Some(index) = catalog.get_index(entity_type, label) {
            // Index exists; estimate cost
            let index_cost = estimate_index_cost(index, predicates);
            let table_scan_cost = estimate_table_scan_cost(entity_type, label);

            if index_cost < table_scan_cost * 0.9 {  // 10% threshold
                return ScanStrategy::IndexScan { index_name: index.name.clone() };
            }
        }
    }

    ScanStrategy::TableScan
}

enum ScanStrategy {
    TableScan,
    IndexScan { index_name: String },
}
```

---

## 5. Query Optimization

### 5.1 Rule-Based Optimizations

Rule-based optimizations are applied to the logical plan using AST rewriting rules.

#### Predicate Pushdown
Move filter conditions as early as possible to reduce intermediate result size.

**Rule:**
```
Filter(input, predicate)
WHERE input is a Join or Expand
→ Pushdown predicate into input's source operators
```

**Example:**
```
BEFORE:
Filter(
  input=Expand(Scan(Person, "a"), "KNOWS", "b"),
  predicate=(a.age > 25 AND b.name = "Bob")
)

AFTER:
Expand(
  input=Filter(
    input=Scan(Person, "a", properties=[age > 25]),
    predicate=(a.age > 25)
  ),
  predicate=(b.name = "Bob")
)
```

**Implementation:**
- Analyze WHERE expression for conjuncts (AND clauses)
- For each conjunct, determine which input variables it references
- Push predicates that reference only input variables into the input's source

#### Projection Pushdown
Eliminate columns early to reduce memory usage.

**Rule:**
```
Project(input, columns)
WHERE input doesn't need all columns
→ Modify input's output to only compute needed columns
```

**Limitation in v1.0:** Simple implementation only; full projection pushdown deferred.

#### Filter Merge
Combine consecutive filters to reduce operator overhead.

**Rule:**
```
Filter(input, pred2) where input=Filter(..., pred1)
→ Filter(input, pred1 AND pred2)
```

**Implementation:**
```rust
fn merge_filters(plan: &mut LogicalPlan) {
    if let LogicalOperator::Filter { input, predicate } = &plan.operator {
        if let LogicalOperator::Filter {
            input: inner_input,
            predicate: inner_pred
        } = input.operator {
            // Combine predicates with AND
            let combined = BinaryOp {
                op: BinaryOperator::And,
                left: inner_pred.clone(),
                right: predicate.clone(),
            };
            plan.operator = LogicalOperator::Filter {
                input: inner_input.clone(),
                predicate: combined,
            };
        }
    }
}
```

### 5.2 Statistics-Based Cost Estimation

Cost-based optimization relies on statistics about the data distribution.

```rust
pub struct TableStatistics {
    pub row_count: u64,
    pub avg_row_size: u64,
}

pub struct ColumnStatistics {
    pub column_name: String,
    pub distinct_values: u64,
    pub min_value: Option<Value>,
    pub max_value: Option<Value>,
    pub null_fraction: f64,
}

pub struct CostModel {
    pub stats: HashMap<String, TableStatistics>,
    pub column_stats: HashMap<String, ColumnStatistics>,
}

impl CostModel {
    /// Estimate selectivity of a predicate (0.0 to 1.0)
    pub fn estimate_selectivity(&self, predicate: &Expression) -> f64 {
        match predicate {
            // Simple heuristics for v1.0
            Expression::BinaryOp { op, left, right } => {
                match op {
                    BinaryOperator::Equal => 0.1,         // ~10% rows
                    BinaryOperator::GreaterThan => 0.5,   // ~50% rows
                    BinaryOperator::And => {
                        let left_sel = self.estimate_selectivity(left);
                        let right_sel = self.estimate_selectivity(right);
                        left_sel * right_sel  // Independence assumption
                    },
                    BinaryOperator::Or => {
                        let left_sel = self.estimate_selectivity(left);
                        let right_sel = self.estimate_selectivity(right);
                        left_sel + right_sel - (left_sel * right_sel)
                    },
                    _ => 0.5,  // Default: 50%
                }
            },
            _ => 1.0,
        }
    }

    /// Estimate output rows from logical operator
    pub fn estimate_rows(&self, operator: &LogicalOperator) -> u64 {
        match operator {
            LogicalOperator::Scan { label, .. } => {
                // Estimate based on label statistics
                if let Some(label) = label {
                    self.stats.get(label).map(|s| s.row_count).unwrap_or(1000)
                } else {
                    self.stats.values().map(|s| s.row_count).sum()
                }
            },
            LogicalOperator::Filter { input, predicate } => {
                let input_rows = self.estimate_rows(input);
                let selectivity = self.estimate_selectivity(predicate);
                (input_rows as f64 * selectivity).ceil() as u64
            },
            LogicalOperator::Expand { from_var, .. } => {
                // Assume average degree of ~5 per node
                let input_rows = self.estimate_rows(&input);
                input_rows * 5
            },
            _ => 1000,  // Default estimate
        }
    }
}
```

### 5.3 Join Ordering for Multi-Hop Patterns

For queries with multiple MATCH patterns, determine the optimal join order.

**Strategy:**
1. Estimate selectivity of each pattern
2. Order patterns from most selective to least selective
3. Use dynamic programming for exhaustive search (small join graphs)
4. Use greedy heuristics for large join graphs

```rust
fn optimize_join_order(patterns: Vec<PatternNode>, cost_model: &CostModel) -> Vec<PatternNode> {
    // Simple greedy approach for v1.0
    let mut remaining = patterns;
    let mut ordered = Vec::new();

    while !remaining.is_empty() {
        // Find pattern with lowest estimated output rows
        let (idx, _) = remaining
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| cost_model.estimate_rows_for_pattern(p))
            .unwrap();

        ordered.push(remaining.remove(idx));
    }

    ordered
}
```

### 5.4 Short-Circuit Evaluation

For AND expressions, evaluate left-to-right and stop when any condition is false.

**Implementation:** Handled at execution time in expression evaluator, not during planning.

---

## 6. Temporal Query Extensions

Temporal support is planned for v1.1; v1.0 provides groundwork without full implementation.

### 6.1 Temporal MATCH Syntax (Future)

**Proposed syntax for v1.1:**
```cypher
-- Point-in-time query
MATCH (n:Person AT TIME '2024-01-01') RETURN n

-- Range query
MATCH (n:Person BETWEEN '2024-01-01' AND '2024-06-30') RETURN n

-- Relationship timing
MATCH (a)-[r:WORKS_FOR AT TIME '2024-03-15']->(b) RETURN a, b
```

**Lexer/Parser changes needed:**
- New token: `AT`, `BETWEEN`, `AND` (as range operator)
- AST extension: `temporal_bound` in NodePatternNode and RelPatternNode

```rust
pub struct NodePatternNode {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<Vec<(String, ExpressionNode)>>,
    pub temporal_bound: Option<TemporalBound>,  // NEW
}

pub enum TemporalBound {
    AtTime(String),                          // AT '2024-01-01'
    Between(String, String),                 // BETWEEN '...' AND '...'
}
```

### 6.2 History Queries (Future)

**Proposed syntax:**
```cypher
-- All versions of a person
MATCH (n:Person{id: 123}) RETURN HISTORY(n) AS versions

-- Changes between dates
MATCH (n:Person)
WHERE n.modified_between('2024-01-01', '2024-06-30')
RETURN n, n.valid_from, n.valid_to
```

### 6.3 Temporal Predicates in WHERE Clause (Future)

```cypher
WHERE n.valid_from <= '2024-03-15' AND n.valid_to >= '2024-03-15'
WHERE r.active_during('2024-01-01', '2024-12-31')
```

### 6.4 Temporal Operator Integration (Future)

**Logical plan extension:**
```rust
pub enum LogicalOperator {
    // ... existing operators ...

    TemporalFilter {
        input: Box<LogicalPlan>,
        time_point: Option<String>,      // AT TIME
        time_range: Option<(String, String)>,  // BETWEEN
    },

    TemporalJoin {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        time_overlap: bool,  // Require temporal overlap
    },
}
```

**Physical execution:**
- Temporal scan with index on (validFrom, validTo) ranges
- Range tree or interval tree data structure for efficient lookup
- Bitemporal support via secondary transaction time dimension

---

## 7. RDF Query Bridge

RDF support is planned for v1.2; this section outlines the design.

### 7.1 SPARQL-like Pattern Mapping to Cypher

**SPARQL triple pattern:**
```sparql
SELECT ?name
WHERE {
    ?person rdf:type ex:Person .
    ?person ex:name ?name .
    ?person ex:age ?age .
    FILTER (?age > 25)
}
```

**Translation to Cypher:**
```cypher
MATCH (person:ex_Person)-[:rdf_type]->(type {uri: "ex:Person"})
MATCH (person)-[:ex_name]->(name)
MATCH (person)-[:ex_age]->(age_val {value: ?age})
WHERE ?age > 25
RETURN name
```

**Mapping strategy:**
1. **RDF Classes** → Node labels (with namespace prefix)
2. **RDF Properties** → Relationships between nodes
3. **RDF Literals** → Property values on nodes (or separate value nodes)
4. **FILTER clauses** → WHERE predicates

### 7.2 Triple Pattern → Node-Edge-Node Translation

```rust
pub struct TriplePattern {
    pub subject: Term,      // Variable or IRI
    pub predicate: Term,    // IRI
    pub object: Term,       // Variable, IRI, or Literal
}

pub enum Term {
    Variable(String),
    Iri(String),
    Literal(String, Option<String>),  // (value, datatype)
}

impl TriplePattern {
    /// Convert RDF triple pattern to Cypher pattern
    pub fn to_cypher_pattern(&self) -> PatternNode {
        // Subject → node variable
        let subject_var = match &self.subject {
            Term::Variable(v) => v.clone(),
            Term::Iri(iri) => format!("subj_{}", iri.hash()),
        };

        // Predicate → relationship type
        let rel_type = match &self.predicate {
            Term::Iri(iri) => iri.clone(),
            _ => panic!("Predicate must be IRI"),
        };

        // Object → node or value
        let object_var = match &self.object {
            Term::Variable(v) => v.clone(),
            Term::Iri(iri) => format!("obj_{}", iri.hash()),
            Term::Literal(value, _) => {
                // Create a value node with property
                return PatternNode {
                    elements: vec![
                        PatternElement::Node(NodePatternNode {
                            variable: Some(subject_var),
                            labels: vec![],
                            properties: None,
                        }),
                        PatternElement::Relationship(RelPatternNode {
                            variable: None,
                            rel_type: Some(rel_type),
                            properties: None,
                            direction: Direction::Outgoing,
                            variable_length: None,
                        }),
                        PatternElement::Node(NodePatternNode {
                            variable: None,
                            labels: vec![],
                            properties: Some(vec![("value".to_string(), ExpressionNode::Literal(
                                LiteralNode::String(value.clone())
                            ))]),
                        }),
                    ],
                };
            },
        };

        // Build pattern: subject -[rel_type]-> object
        PatternNode {
            elements: vec![
                PatternElement::Node(NodePatternNode {
                    variable: Some(subject_var),
                    labels: vec![],
                    properties: None,
                }),
                PatternElement::Relationship(RelPatternNode {
                    variable: None,
                    rel_type: Some(rel_type),
                    properties: None,
                    direction: Direction::Outgoing,
                    variable_length: None,
                }),
                PatternElement::Node(NodePatternNode {
                    variable: Some(object_var),
                    labels: vec![],
                    properties: None,
                }),
            ],
        }
    }
}
```

### 7.3 Named Graph Support (Future)

**Extended quad model:**
```rust
pub struct Quad {
    pub subject: Term,
    pub predicate: Term,
    pub object: Term,
    pub graph: Term,  // NEW
}

pub enum GraphMatch {
    Default,                    // Match in default graph
    Named(String),             // Match in specific named graph
    All,                       // Match across all graphs
}
```

**Query syntax (future):**
```cypher
MATCH (s)-[p]->(o) IN GRAPH "http://example.org/data"
RETURN s, p, o
```

---

## 8. Query Execution Runtime

### 8.1 Register-Based vs Tuple-at-a-Time

CypherLite v1.0 uses **tuple-at-a-time** (Volcano model) execution:

**Advantages:**
- Simple to implement and understand
- Low memory overhead (processes one tuple at a time)
- Natural fit for streaming results
- Easy to parallelize operators

**Disadvantages:**
- Function call overhead between operators
- Poor CPU cache locality
- Difficult to vectorize

**Future (v2.0):** Consider batch-oriented execution (vectorized model) for better performance.

### 8.2 Memory Management During Execution

```rust
pub struct ExecutionContext {
    /// Allocator for intermediate result buffers
    pub buffer_pool: BufferPool,

    /// Statistics on memory usage
    pub memory_usage: MemoryStats,

    /// Maximum memory allowed for query execution
    pub memory_limit: u64,
}

pub struct BufferPool {
    /// Pool of reusable Tuple buffers
    buffers: Vec<Tuple>,

    /// Current allocation tracking
    allocated: u64,
}

impl BufferPool {
    pub fn acquire(&mut self) -> Tuple {
        if let Some(buffer) = self.buffers.pop() {
            buffer  // Reuse existing
        } else {
            HashMap::new()  // Allocate new
        }
    }

    pub fn release(&mut self, buffer: Tuple) {
        if self.buffers.len() < MAX_POOL_SIZE {
            self.buffers.push(buffer);
        }
        // Else: buffer is dropped
    }
}

pub struct MemoryStats {
    pub total_allocated: u64,
    pub peak_usage: u64,
    pub allocations: u64,
}

/// Check memory limit before allocating large structures
pub fn check_memory_limit(ctx: &ExecutionContext) -> Result<()> {
    if ctx.memory_usage.total_allocated > ctx.memory_limit {
        return Err(Error::MemoryLimitExceeded);
    }
    Ok(())
}
```

**Specific memory management patterns:**

1. **Hash aggregation:** Store intermediate hash table in BufferPool
2. **Sort:** Stream results to temporary file if hash table exceeds memory limit
3. **Join:** Use spillable hash table for large joins
4. **Buffer pools:** Maintain pools for Tuple, Value, and intermediate structures

### 8.3 Result Streaming / Cursor Model

Results are streamed to the client as they are produced, enabling interactive queries and out-of-core execution.

```rust
pub trait ResultCursor {
    /// Fetch the next result tuple
    fn next(&mut self) -> Result<Option<ResultRow>>;

    /// Get metadata about result columns
    fn columns(&self) -> Vec<ColumnMetadata>;

    /// Close the cursor and free resources
    fn close(&mut self) -> Result<()>;
}

pub struct QueryResult {
    cursor: Box<dyn ResultCursor>,
    columns: Vec<ColumnMetadata>,
}

pub struct ColumnMetadata {
    pub name: String,
    pub inferred_type: Option<Type>,  // May be unknown until execution
}

impl QueryResult {
    pub fn fetch_all(&mut self) -> Result<Vec<ResultRow>> {
        let mut rows = Vec::new();
        while let Some(row) = self.cursor.next()? {
            rows.push(row);
        }
        self.cursor.close()?;
        Ok(rows)
    }

    pub fn fetch_one(&mut self) -> Result<Option<ResultRow>> {
        self.cursor.next()
    }
}

pub type ResultRow = Vec<Value>;

/// Execution flow
pub fn execute_query(query: &str, db: &Database) -> Result<QueryResult> {
    // 1. Parse
    let ast = parse_cypher(query)?;

    // 2. Semantic analysis
    let validated_ast = validate_ast(&ast)?;

    // 3. Logical planning
    let logical_plan = plan_query(&validated_ast)?;

    // 4. Optimization
    let optimized_plan = optimize(&logical_plan)?;

    // 5. Physical planning
    let physical_plan = compile_physical(&optimized_plan)?;

    // 6. Execution
    let ctx = ExecutionContext {
        buffer_pool: BufferPool::new(MAX_BUFFERS),
        memory_usage: MemoryStats::default(),
        memory_limit: 1024 * 1024 * 1024,  // 1GB
    };

    let cursor = execute_physical(&physical_plan, db, ctx)?;

    Ok(QueryResult { cursor, columns: ... })
}
```

---

## 9. Query Compilation Pipeline Summary

The complete pipeline from query string to result:

```
User Query String
    ↓
[1] Lexer/Tokenizer
    ↓ (Token stream)
[2] Parser (Recursive Descent)
    ↓ (Abstract Syntax Tree)
[3] Semantic Analyzer
    ↓ (Validated AST with types)
[4] Logical Planner
    ↓ (Logical execution plan)
[5] Optimizer (Rules + Statistics)
    ↓ (Optimized logical plan)
[6] Physical Planner
    ↓ (Physical execution plan with costs)
[7] Executor (Volcano iterator model)
    ↓ (Result stream)
Client Results
```

---

## 10. Implementation Roadmap

### Phase 1 (v1.0 - Current)
- Lexer and recursive descent parser
- Basic logical planning
- Physical execution with nested loop joins
- Filter, project, limit, sort
- Basic aggregation (COUNT, SUM, AVG, MIN, MAX)
- Simple cost estimation

### Phase 2 (v1.1)
- Hash joins for performance
- Full aggregation support with COLLECT and DISTINCT
- Index scan integration
- WITH clause and multi-stage queries
- UNION support
- Rule-based optimizations (predicate pushdown, filter merge)
- Temporal syntax (AT TIME, BETWEEN)

### Phase 3 (v1.2)
- RDF/SPARQL bridge layer
- Named graph support
- Advanced cost-based optimization
- Query plan caching

### Phase 4 (v1.3+)
- Shortest path algorithms
- GraphQL interface
- Distributed execution
- Vectorized execution model
- Query profiling and explain plans

---

## 11. Error Handling and Debugging

### 11.1 Parse Errors

```rust
pub enum ParseError {
    UnexpectedToken { expected: String, got: TokenType, position: Position },
    UnknownKeyword(String),
    InvalidPattern(String),
    SyntaxError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnexpectedToken { expected, got, position } => {
                write!(f, "Parse error at {}:{}: expected {}, got {:?}",
                    position.line, position.column, expected, got)
            },
            // ... other variants
        }
    }
}
```

### 11.2 Runtime Errors

```rust
pub enum ExecutionError {
    VariableNotBound(String),
    TypeMismatch { expected: Type, got: Type },
    DivisionByZero,
    IndexOutOfBounds { index: i64, length: usize },
    InvalidFunction { name: String, arity: usize },
    MemoryLimitExceeded,
    IOError(String),
}
```

### 11.3 Query Explanation (EXPLAIN Plan)

**Proposed syntax (future):**
```cypher
EXPLAIN MATCH (a)-[:KNOWS]->(b) RETURN a, b
```

**Output:**
```
Execution Plan:
  Project
    ├─ expressions: [a.name, b.name]
    └─ input: Filter
         ├─ predicate: (a.age > 25)
         └─ input: Expand
              ├─ rel_type: KNOWS
              ├─ from_var: a
              ├─ to_var: b
              └─ input: Scan
                   ├─ entity_type: Node
                   ├─ label: Person
                   └─ output_var: a

Statistics:
  Scan rows: 1000
  After filter: 500
  Final output: 2500
  Total cost: 3500 units
```

---

## 12. Appendix: Example Queries and Execution

### Example 1: Simple Match

```cypher
MATCH (n:Person) RETURN n.name
```

**Parse tree:**
```
Query
├─ MatchClause
│  ├─ patterns: [Pattern { elements: [Node(Person)] }]
│  └─ where_clause: None
└─ ReturnClause
   ├─ distinct: false
   └─ items: [Property(n.name)]
```

**Logical plan:**
```
Project(
  input=Scan(entity_type=Node, label="Person", variable="n"),
  expressions=[(n.name, None)]
)
```

**Physical plan:**
```
Project(
  input=TableScan(entity_type=Node, output="n"),
  expressions=[(Property(Variable("n"), "name"), None)]
)
```

### Example 2: Multi-hop with Aggregation

```cypher
MATCH (a:Person)-[:KNOWS]->(b)-[:KNOWS]->(c)
WHERE a.name = "Alice"
RETURN a.name, COUNT(DISTINCT c.name) AS friend_count
```

**Logical plan:**
```
Aggregate(
  input=Filter(
    input=Expand(
      from_var="b",
      rel_type="KNOWS",
      to_var="c",
      input=Expand(
        from_var="a",
        rel_type="KNOWS",
        to_var="b",
        input=Scan(label="Person", variable="a")
      )
    ),
    predicate=(a.name = "Alice")
  ),
  group_by=[a.name],
  aggregates=[("friend_count", Count{distinct=true})]
)
```

---

**Document Version History:**
- v1.0 - 2026-03-10: Initial design specification for CypherLite query engine

---

END OF DOCUMENT
