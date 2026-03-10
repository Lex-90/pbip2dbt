//! AST types for the TMDL parser.
//!
//! These types represent the structured output of parsing Power BI TMDL files.
//! They are the shared data contracts between the parsers and translators.

use serde::Serialize;

/// The complete semantic model parsed from TMDL files.
#[derive(Debug, Clone, Default)]
pub struct SemanticModel {
    /// Name of the semantic model (inferred from the folder name).
    pub name: Option<String>,
    /// Tables in the model.
    pub tables: Vec<Table>,
    /// Relationships between tables.
    pub relationships: Vec<Relationship>,
}

/// A table in the semantic model.
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name as defined in TMDL.
    pub name: String,
    /// Table description from `///` annotation.
    pub description: Option<String>,
    /// Lineage tag for tracking.
    pub lineage_tag: Option<String>,
    /// Regular columns (sourced from Power Query).
    pub columns: Vec<Column>,
    /// Calculated columns (DAX expressions).
    pub calculated_columns: Vec<CalculatedColumn>,
    /// Measures defined on this table.
    pub measures: Vec<Measure>,
    /// Power Query M partition (data source).
    pub partition: Option<Partition>,
    /// DAX expression if this is a calculated table (no partition).
    pub calculated_table_expression: Option<String>,
}

/// A regular column sourced from Power Query.
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name.
    pub name: String,
    /// Data type.
    pub data_type: DataType,
    /// Original source column name.
    pub source_column: Option<String>,
    /// Description from `///` annotation.
    pub description: Option<String>,
    /// Lineage tag.
    pub lineage_tag: Option<String>,
    /// Summarize-by setting.
    pub summarize_by: Option<String>,
}

/// A calculated column defined by a DAX expression.
#[derive(Debug, Clone)]
pub struct CalculatedColumn {
    /// Column name.
    pub name: String,
    /// DAX expression that computes this column's value.
    pub dax_expression: String,
    /// Data type.
    pub data_type: DataType,
    /// Lineage tag.
    pub lineage_tag: Option<String>,
    /// Whether the data type is inferred.
    pub is_data_type_inferred: bool,
}

/// A measure defined by a DAX expression.
#[derive(Debug, Clone)]
pub struct Measure {
    /// Measure name.
    pub name: String,
    /// DAX expression.
    pub dax_expression: String,
    /// Format string (e.g., "$#,##0.00").
    pub format_string: Option<String>,
    /// Description from `///` annotation.
    pub description: Option<String>,
    /// Display folder path.
    pub display_folder: Option<String>,
    /// Lineage tag.
    pub lineage_tag: Option<String>,
}

/// A Power Query M partition (data source).
#[derive(Debug, Clone)]
pub struct Partition {
    /// Partition name.
    pub name: String,
    /// Import mode.
    pub mode: ImportMode,
    /// Raw M expression code.
    pub m_expression: String,
}

/// A relationship between two tables.
#[derive(Debug, Clone)]
pub struct Relationship {
    /// Relationship name/identifier.
    pub name: String,
    /// Source table name (from side).
    pub from_table: String,
    /// Source column name.
    pub from_column: String,
    /// Target table name (to side).
    pub to_table: String,
    /// Target column name.
    pub to_column: String,
    /// Cross-filtering behavior.
    pub cross_filtering: CrossFilterBehavior,
    /// Whether the relationship is active.
    pub is_active: bool,
}

/// Import mode for a partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ImportMode {
    /// Full data import.
    Import,
    /// Direct query (live connection).
    DirectQuery,
    /// Dual mode (both import and direct query).
    Dual,
    /// Default/unspecified.
    Default,
}

/// Cross-filtering behavior for relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CrossFilterBehavior {
    /// One-direction filtering.
    OneDirection,
    /// Both-directions filtering.
    BothDirections,
    /// Automatic (default).
    Automatic,
}

/// Data types in the TMDL model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DataType {
    /// Text/string type.
    String,
    /// 64-bit integer.
    Int64,
    /// Double-precision floating point.
    Double,
    /// Decimal/numeric type.
    Decimal,
    /// Boolean type.
    Boolean,
    /// Date-only type.
    Date,
    /// Date and time type.
    DateTime,
    /// Date and time with timezone.
    DateTimeZone,
    /// Time-only type.
    Time,
    /// Duration type.
    Duration,
    /// Binary data type.
    Binary,
    /// Unknown or unspecified type.
    Unknown,
}

impl DataType {
    /// Parse a TMDL data type string into a `DataType`.
    pub fn from_tmdl(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "string" => Self::String,
            "int64" => Self::Int64,
            "double" => Self::Double,
            "decimal" => Self::Decimal,
            "boolean" => Self::Boolean,
            "dateTime" | "datetime" => Self::DateTime,
            "date" => Self::Date,
            "time" => Self::Time,
            "datetimezone" | "dateTimeZone" => Self::DateTimeZone,
            "duration" => Self::Duration,
            "binary" => Self::Binary,
            _ => Self::Unknown,
        }
    }
}

/// Result of a translation operation (shared across all translators).
#[derive(Debug, Clone)]
pub struct TranslationResult {
    /// Translated SQL.
    pub sql: String,
    /// Confidence score from 0.0 to 1.0.
    pub confidence: f64,
    /// Non-fatal warnings.
    pub warnings: Vec<crate::error::Warning>,
    /// Whether manual review is needed.
    pub manual_review: bool,
}
