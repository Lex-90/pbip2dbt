//! SQL Server (T-SQL) adapter implementation.

use super::SqlAdapter;
use crate::tmdl::ast::DataType;

/// SQL Server (T-SQL) dialect adapter.
#[derive(Debug)]
pub struct SqlServerAdapter;

impl SqlAdapter for SqlServerAdapter {
    fn name(&self) -> &'static str { "sqlserver" }

    fn quote_identifier(&self, name: &str) -> String { format!("[{name}]") }

    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String {
        format!("cast({expr} as {})", self.type_name(target_type))
    }

    fn date_trunc(&self, part: &str, expr: &str) -> String {
        format!("datetrunc({part}, {expr})")
    }

    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String {
        format!("dateadd({part}, {interval}, {expr})")
    }

    fn date_diff(&self, part: &str, start: &str, end: &str) -> String {
        format!("datediff({part}, {start}, {end})")
    }

    fn limit_clause(&self, n: usize) -> String { format!("top {n}") }
    fn concat_op(&self) -> &'static str { "+" }

    fn boolean_literal(&self, val: bool) -> String {
        if val { "1".to_string() } else { "0".to_string() }
    }

    fn type_name(&self, dt: &DataType) -> String {
        match dt {
            DataType::String => "nvarchar(max)".to_string(),
            DataType::Int64 => "bigint".to_string(),
            DataType::Double => "float".to_string(),
            DataType::Decimal => "decimal(38, 10)".to_string(),
            DataType::Boolean => "bit".to_string(),
            DataType::DateTime => "datetime2".to_string(),
            DataType::Date => "date".to_string(),
            DataType::Time => "time".to_string(),
            DataType::DateTimeZone => "datetimeoffset".to_string(),
            DataType::Duration => "nvarchar(100)".to_string(),
            DataType::Binary => "varbinary(max)".to_string(),
            DataType::Unknown => "nvarchar(max)".to_string(),
        }
    }

    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String {
        format!("iif({cond}, {true_val}, {false_val})")
    }

    fn current_date(&self) -> String { "cast(getdate() as date)".to_string() }
    fn current_timestamp(&self) -> String { "getdate()".to_string() }
    fn nullif(&self, expr: &str, val: &str) -> String { format!("nullif({expr}, {val})") }
    fn string_length(&self, expr: &str) -> String { format!("len({expr})") }
    fn left_substr(&self, expr: &str, n: &str) -> String { format!("left({expr}, {n})") }
    fn right_substr(&self, expr: &str, n: &str) -> String { format!("right({expr}, {n})") }
    fn extract_year(&self, expr: &str) -> String { format!("year({expr})") }
    fn extract_month(&self, expr: &str) -> String { format!("month({expr})") }
    fn extract_day(&self, expr: &str) -> String { format!("day({expr})") }
}
