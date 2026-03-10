//! SQL adapter abstraction layer.
//!
//! Defines the `SqlAdapter` trait for dialect-specific SQL generation
//! and provides implementations for `PostgreSQL`, Snowflake, `BigQuery`, and SQL Server.

pub mod bigquery;
pub mod postgres;
pub mod snowflake;
pub mod sqlserver;

use crate::error::ArgError;
use crate::tmdl::ast::DataType;

/// SQL adapter trait for dialect-specific SQL generation.
///
/// Adding a new adapter requires implementing this trait and adding
/// a match arm in [`adapter_for`].
pub trait SqlAdapter: Send + Sync {
    /// Adapter name (e.g., "postgres", "snowflake").
    fn name(&self) -> &'static str;

    /// Quote an identifier per the dialect's convention.
    fn quote_identifier(&self, name: &str) -> String;

    /// Generate a CAST expression.
    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String;

    /// Generate a `DATE_TRUNC` expression.
    fn date_trunc(&self, part: &str, expr: &str) -> String;

    /// Generate a `DATE_ADD` expression.
    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String;

    /// Generate a `DATE_DIFF` expression.
    fn date_diff(&self, part: &str, start: &str, end: &str) -> String;

    /// Generate a LIMIT clause.
    fn limit_clause(&self, n: usize) -> String;

    /// Get the string concatenation operator.
    fn concat_op(&self) -> &str;

    /// Generate a boolean literal.
    fn boolean_literal(&self, val: bool) -> String;

    /// Get the SQL type name for a data type.
    fn type_name(&self, dt: &DataType) -> String;

    /// Generate an IIF/IF/CASE WHEN expression.
    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String;

    /// Get the `CURRENT_DATE` expression.
    fn current_date(&self) -> String;

    /// Get the `CURRENT_TIMESTAMP` expression.
    fn current_timestamp(&self) -> String;

    /// Generate a NULLIF expression.
    fn nullif(&self, expr: &str, val: &str) -> String;

    /// Generate a string length expression.
    fn string_length(&self, expr: &str) -> String;

    /// Generate a LEFT/SUBSTR expression for extracting from the start.
    fn left_substr(&self, expr: &str, n: &str) -> String;

    /// Generate a RIGHT/SUBSTR expression for extracting from the end.
    fn right_substr(&self, expr: &str, n: &str) -> String;

    /// Generate a YEAR extract expression.
    fn extract_year(&self, expr: &str) -> String;

    /// Generate a MONTH extract expression.
    fn extract_month(&self, expr: &str) -> String;

    /// Generate a DAY extract expression.
    fn extract_day(&self, expr: &str) -> String;
}

/// Create an adapter for the given name.
///
/// # Errors
///
/// Returns `ArgError::InvalidAdapter` if the name is not recognized.
pub fn adapter_for(name: &str) -> Result<Box<dyn SqlAdapter>, ArgError> {
    match name {
        "postgres" => Ok(Box::new(postgres::PostgresAdapter)),
        "snowflake" => Ok(Box::new(snowflake::SnowflakeAdapter)),
        "bigquery" => Ok(Box::new(bigquery::BigQueryAdapter)),
        "sqlserver" => Ok(Box::new(sqlserver::SqlServerAdapter)),
        other => Err(ArgError::InvalidAdapter(other.to_string())),
    }
}
