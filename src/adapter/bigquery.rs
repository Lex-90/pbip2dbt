//! `BigQuery` adapter implementation.

use super::SqlAdapter;
use crate::tmdl::ast::DataType;

/// `BigQuery` SQL dialect adapter.
#[derive(Debug)]
pub struct BigQueryAdapter;

impl SqlAdapter for BigQueryAdapter {
    fn name(&self) -> &'static str { "bigquery" }

    fn quote_identifier(&self, name: &str) -> String { format!("`{name}`") }

    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String {
        format!("cast({expr} as {})", self.type_name(target_type))
    }

    fn date_trunc(&self, part: &str, expr: &str) -> String {
        format!("date_trunc({expr}, {part})")
    }

    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String {
        format!("date_add({expr}, interval {interval} {part})")
    }

    fn date_diff(&self, part: &str, start: &str, end: &str) -> String {
        format!("date_diff({end}, {start}, {part})")
    }

    fn limit_clause(&self, n: usize) -> String { format!("limit {n}") }
    fn concat_op(&self) -> &'static str { "||" }

    fn boolean_literal(&self, val: bool) -> String {
        if val { "true".to_string() } else { "false".to_string() }
    }

    fn type_name(&self, dt: &DataType) -> String {
        match dt {
            DataType::String => "string".to_string(),
            DataType::Int64 => "int64".to_string(),
            DataType::Double => "float64".to_string(),
            DataType::Decimal => "numeric".to_string(),
            DataType::Boolean => "bool".to_string(),
            DataType::DateTime => "timestamp".to_string(),
            DataType::Date => "date".to_string(),
            DataType::Time => "time".to_string(),
            DataType::DateTimeZone => "timestamp".to_string(),
            DataType::Duration => "string".to_string(),
            DataType::Binary => "bytes".to_string(),
            DataType::Unknown => "string".to_string(),
        }
    }

    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String {
        format!("if({cond}, {true_val}, {false_val})")
    }

    fn current_date(&self) -> String { "current_date()".to_string() }
    fn current_timestamp(&self) -> String { "current_timestamp()".to_string() }
    fn nullif(&self, expr: &str, val: &str) -> String { format!("nullif({expr}, {val})") }
    fn string_length(&self, expr: &str) -> String { format!("length({expr})") }
    fn left_substr(&self, expr: &str, n: &str) -> String { format!("substr({expr}, 1, {n})") }
    fn right_substr(&self, expr: &str, n: &str) -> String { format!("right({expr}, {n})") }
    fn extract_year(&self, expr: &str) -> String { format!("extract(year from {expr})") }
    fn extract_month(&self, expr: &str) -> String { format!("extract(month from {expr})") }
    fn extract_day(&self, expr: &str) -> String { format!("extract(day from {expr})") }
}
