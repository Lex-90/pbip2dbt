//! AST types for DAX expressions.

/// A parsed DAX expression tree.
#[derive(Debug, Clone)]
pub enum DaxExpr {
    /// A function call like `SUM(Table[Col])`.
    FunctionCall(DaxFunctionCall),
    /// A column reference like `Table[Column]` or `[Column]`.
    ColumnRef(ColumnRef),
    /// A measure reference like `[MeasureName]`.
    MeasureRef(String),
    /// A literal value.
    Literal(DaxLiteral),
    /// A binary operation (e.g., `a + b`).
    BinaryOp(Box<DaxExpr>, DaxOp, Box<DaxExpr>),
    /// A unary operation (e.g., `NOT expr`).
    UnaryOp(DaxUnaryOp, Box<DaxExpr>),
    /// A VAR/RETURN block.
    VarReturn(Vec<VarDecl>, Box<DaxExpr>),
    /// Raw unparsed DAX.
    Raw(String),
}

/// A DAX function call.
#[derive(Debug, Clone)]
pub struct DaxFunctionCall {
    /// Function name (e.g., "SUM", "CALCULATE", "IF").
    pub function_name: String,
    /// Arguments.
    pub arguments: Vec<DaxExpr>,
}

/// A column reference in DAX.
#[derive(Debug, Clone)]
pub struct ColumnRef {
    /// Table name (may be empty for unqualified references).
    pub table: Option<String>,
    /// Column name.
    pub column: String,
}

/// A DAX literal value.
#[derive(Debug, Clone)]
pub enum DaxLiteral {
    /// Integer.
    Integer(i64),
    /// Decimal/float.
    Float(f64),
    /// String.
    String(String),
    /// Boolean.
    Boolean(bool),
    /// `BLANK()` / null.
    Blank,
}

/// A binary operator in DAX.
#[derive(Debug, Clone)]
pub enum DaxOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Equals.
    Eq,
    /// Not equals.
    Neq,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
    /// Greater than or equal.
    Gte,
    /// Less than or equal.
    Lte,
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
    /// String concatenation `&`.
    Concat,
}

/// A unary operator in DAX.
#[derive(Debug, Clone)]
pub enum DaxUnaryOp {
    /// Logical NOT.
    Not,
    /// Negation.
    Neg,
}

/// A VAR declaration.
#[derive(Debug, Clone)]
pub struct VarDecl {
    /// Variable name.
    pub name: String,
    /// Variable expression.
    pub expression: DaxExpr,
}
