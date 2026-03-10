//! `PostgreSQL` adapter implementation.

use super::SqlAdapter;
use crate::tmdl::ast::DataType;

/// `PostgreSQL` SQL dialect adapter.
#[derive(Debug)]
pub struct PostgresAdapter;

impl SqlAdapter for PostgresAdapter {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn quote_identifier(&self, name: &str) -> String {
        format!("\"{name}\"")
    }

    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String {
        format!("cast({expr} as {})", self.type_name(target_type))
    }

    fn date_trunc(&self, part: &str, expr: &str) -> String {
        format!("date_trunc('{part}', {expr})")
    }

    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String {
        format!("{expr} + interval '{interval} {part}'")
    }

    fn date_diff(&self, part: &str, start: &str, end: &str) -> String {
        format!("date_part('{part}', {end} - {start})")
    }

    fn limit_clause(&self, n: usize) -> String {
        format!("limit {n}")
    }

    fn concat_op(&self) -> &'static str {
        "||"
    }

    fn boolean_literal(&self, val: bool) -> String {
        if val {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }

    fn type_name(&self, dt: &DataType) -> String {
        match dt {
            DataType::String => "varchar".to_string(),
            DataType::Int64 => "integer".to_string(),
            DataType::Double => "double precision".to_string(),
            DataType::Decimal => "numeric(38, 10)".to_string(),
            DataType::Boolean => "boolean".to_string(),
            DataType::DateTime => "timestamp".to_string(),
            DataType::Date => "date".to_string(),
            DataType::Time => "time".to_string(),
            DataType::DateTimeZone => "timestamptz".to_string(),
            DataType::Duration => "interval".to_string(),
            DataType::Binary => "bytea".to_string(),
            DataType::Unknown => "text".to_string(),
        }
    }

    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String {
        format!("case when {cond} then {true_val} else {false_val} end")
    }

    fn current_date(&self) -> String {
        "current_date".to_string()
    }

    fn current_timestamp(&self) -> String {
        "current_timestamp".to_string()
    }

    fn nullif(&self, expr: &str, val: &str) -> String {
        format!("nullif({expr}, {val})")
    }

    fn string_length(&self, expr: &str) -> String {
        format!("length({expr})")
    }

    fn left_substr(&self, expr: &str, n: &str) -> String {
        format!("left({expr}, {n})")
    }

    fn right_substr(&self, expr: &str, n: &str) -> String {
        format!("right({expr}, {n})")
    }

    fn extract_year(&self, expr: &str) -> String {
        format!("extract(year from {expr})")
    }

    fn extract_month(&self, expr: &str) -> String {
        format!("extract(month from {expr})")
    }

    fn extract_day(&self, expr: &str) -> String {
        format!("extract(day from {expr})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_identifier() {
        let a = PostgresAdapter;
        assert_eq!(a.quote_identifier("my_col"), "\"my_col\"");
    }

    #[test]
    fn type_names() {
        let a = PostgresAdapter;
        assert_eq!(a.type_name(&DataType::String), "varchar");
        assert_eq!(a.type_name(&DataType::Int64), "integer");
        assert_eq!(a.type_name(&DataType::DateTime), "timestamp");
    }

    #[test]
    fn date_operations() {
        let a = PostgresAdapter;
        assert_eq!(
            a.date_trunc("month", "col"),
            "date_trunc('month', col)"
        );
        assert_eq!(
            a.date_add("col", 1, "month"),
            "col + interval '1 month'"
        );
    }

    #[test]
    fn boolean_literals() {
        let a = PostgresAdapter;
        assert_eq!(a.boolean_literal(true), "true");
        assert_eq!(a.boolean_literal(false), "false");
    }
}
