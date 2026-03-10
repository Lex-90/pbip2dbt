//! Snowflake adapter implementation.

use super::SqlAdapter;
use crate::tmdl::ast::DataType;

/// Snowflake SQL dialect adapter.
#[derive(Debug)]
pub struct SnowflakeAdapter;

impl SqlAdapter for SnowflakeAdapter {
    fn name(&self) -> &'static str { "snowflake" }

    fn quote_identifier(&self, name: &str) -> String { format!("\"{name}\"") }

    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String {
        format!("cast({expr} as {})", self.type_name(target_type))
    }

    fn date_trunc(&self, part: &str, expr: &str) -> String {
        format!("date_trunc('{part}', {expr})")
    }

    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String {
        format!("dateadd('{part}', {interval}, {expr})")
    }

    fn date_diff(&self, part: &str, start: &str, end: &str) -> String {
        format!("datediff('{part}', {start}, {end})")
    }

    fn limit_clause(&self, n: usize) -> String { format!("limit {n}") }
    fn concat_op(&self) -> &'static str { "||" }

    fn boolean_literal(&self, val: bool) -> String {
        if val { "true".to_string() } else { "false".to_string() }
    }

    fn type_name(&self, dt: &DataType) -> String {
        match dt {
            DataType::String => "varchar".to_string(),
            DataType::Int64 => "integer".to_string(),
            DataType::Double => "float".to_string(),
            DataType::Decimal => "number(38, 10)".to_string(),
            DataType::Boolean => "boolean".to_string(),
            DataType::DateTime => "timestamp_ntz".to_string(),
            DataType::Date => "date".to_string(),
            DataType::Time => "time".to_string(),
            DataType::DateTimeZone => "timestamp_tz".to_string(),
            DataType::Duration => "varchar".to_string(),
            DataType::Binary => "binary".to_string(),
            DataType::Unknown => "variant".to_string(),
        }
    }

    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String {
        format!("iff({cond}, {true_val}, {false_val})")
    }

    fn current_date(&self) -> String { "current_date()".to_string() }
    fn current_timestamp(&self) -> String { "current_timestamp()".to_string() }
    fn nullif(&self, expr: &str, val: &str) -> String { format!("nullif({expr}, {val})") }
    fn string_length(&self, expr: &str) -> String { format!("length({expr})") }
    fn left_substr(&self, expr: &str, n: &str) -> String { format!("left({expr}, {n})") }
    fn right_substr(&self, expr: &str, n: &str) -> String { format!("right({expr}, {n})") }
    fn extract_year(&self, expr: &str) -> String { format!("year({expr})") }
    fn extract_month(&self, expr: &str) -> String { format!("month({expr})") }
    fn extract_day(&self, expr: &str) -> String { format!("day({expr})") }
}
