//! AST types for Power Query M expressions.

/// A parsed M `let...in` expression.
#[derive(Debug, Clone)]
pub struct LetExpr {
    /// Named steps in the let binding.
    pub steps: Vec<MStep>,
    /// The final step name referenced in `in`.
    pub result_step: String,
}

/// A single step in an M let expression.
#[derive(Debug, Clone)]
pub struct MStep {
    /// Step name (variable binding).
    pub name: String,
    /// The expression for this step.
    pub expression: MExpr,
}

/// An M expression node.
#[derive(Debug, Clone)]
pub enum MExpr {
    /// A function call like `Table.SelectRows(...)`.
    FunctionCall(FunctionCall),
    /// A field/item access like `Source{[Schema="dbo"]}[Data]`.
    FieldAccess(FieldAccess),
    /// A literal value.
    Literal(LiteralValue),
    /// A reference to another step variable.
    Reference(String),
    /// A binary operation.
    BinaryOp(Box<MExpr>, String, Box<MExpr>),
    /// A record literal `{key=value, ...}`.
    Record(Vec<(String, MExpr)>),
    /// A list literal `{item, ...}`.
    List(Vec<MExpr>),
    /// An `each` lambda expression.
    Each(Box<MExpr>),
    /// Raw unparsed expression.
    Raw(String),
}

/// A function call in M.
#[derive(Debug, Clone)]
pub struct FunctionCall {
    /// Function name (e.g., "Table.SelectRows", "Sql.Database").
    pub function_name: String,
    /// Arguments to the function.
    pub arguments: Vec<MExpr>,
}

/// A field or item access expression.
#[derive(Debug, Clone)]
pub struct FieldAccess {
    /// The base expression being accessed.
    pub base: Box<MExpr>,
    /// The field or item being accessed.
    pub accessor: String,
}

/// A literal value in M.
#[derive(Debug, Clone)]
pub enum LiteralValue {
    /// String literal.
    String(String),
    /// Integer literal.
    Integer(i64),
    /// Float literal.
    Float(f64),
    /// Boolean literal.
    Boolean(bool),
    /// Null.
    Null,
    /// Date literal `#date(y, m, d)`.
    Date(i32, i32, i32),
}
