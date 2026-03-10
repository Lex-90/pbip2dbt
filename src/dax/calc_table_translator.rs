//! DAX calculated table translator (PRD Engine 3).

use crate::adapter::SqlAdapter;
use crate::dax::parser::parse_dax;
use crate::dax::ast::{DaxExpr, DaxLiteral};
use crate::naming::sanitize_identifier;
use crate::tmdl::ast::TranslationResult;

/// Translate a DAX calculated table expression to SQL.
pub fn translate_calc_table(dax_expr: &str, adapter: &dyn SqlAdapter, source_name: &str) -> TranslationResult {
    let parsed = parse_dax(dax_expr);
    let mut confidence = 1.0;
    let warnings = Vec::new();

    let sql = if let DaxExpr::FunctionCall(fc) = &parsed {
        match fc.function_name.as_str() {
            "CALENDAR" | "CALENDARAUTO" => {
                confidence = 0.8;
                generate_calendar_sql(adapter)
            }
            "DISTINCT" => {
                if let Some(col) = fc.arguments.first() {
                    let col_sql = translate_calc_expr(col, adapter);
                    format!("select distinct {col_sql}")
                } else {
                    format!("-- MANUAL_REVIEW: DISTINCT without arguments\n-- Original DAX: {dax_expr}")
                }
            }
            "SELECTCOLUMNS" => {
                translate_select_columns(&fc.arguments, adapter, source_name)
            }
            "ADDCOLUMNS" => {
                translate_add_columns(&fc.arguments, adapter, source_name)
            }
            "UNION" => {
                let parts: Vec<String> = fc.arguments.iter()
                    .map(|a| translate_calc_expr(a, adapter))
                    .collect();
                parts.join("\nunion all\n")
            }
            "SUMMARIZE" => {
                translate_summarize(&fc.arguments, adapter, source_name)
            }
            "CROSSJOIN" => {
                let tables: Vec<String> = fc.arguments.iter()
                    .map(|a| translate_calc_expr(a, adapter))
                    .collect();
                if tables.len() >= 2 {
                    format!("select * from {} cross join {}", tables[0], tables[1])
                } else {
                    format!("-- MANUAL_REVIEW: CROSSJOIN\n-- Original DAX: {dax_expr}")
                }
            }
            "DATATABLE" => {
                confidence = 0.5;
                format!("-- MANUAL_REVIEW: DATATABLE should be converted to a dbt seed (CSV)\n-- Original DAX: {dax_expr}")
            }
            "ROW" => {
                translate_row(&fc.arguments, adapter)
            }
            _ => {
                confidence = 0.0;
                format!("-- MANUAL_REVIEW: Unsupported calculated table function: {}\n-- Original DAX: {dax_expr}", fc.function_name)
            }
        }
    } else {
        confidence = 0.0;
        format!("-- MANUAL_REVIEW: Cannot translate calculated table expression\n-- Original DAX: {dax_expr}")
    };

    TranslationResult {
        sql,
        confidence,
        warnings,
        manual_review: confidence < 1.0,
    }
}

/// Generate a date spine SQL for CALENDAR/CALENDARAUTO.
fn generate_calendar_sql(_adapter: &dyn SqlAdapter) -> String {
    r#"-- Generated from CALENDAR/CALENDARAUTO
-- Requires dbt_utils.date_spine macro
-- Adjust start_date and end_date as needed
{{ dbt_utils.date_spine(
    datepart="day",
    start_date="cast('2020-01-01' as date)",
    end_date="cast('2030-12-31' as date)"
) }}"#.to_string()
}

/// Translate a DAX expression for use in a calculated table.
fn translate_calc_expr(expr: &DaxExpr, adapter: &dyn SqlAdapter) -> String {
    match expr {
        DaxExpr::ColumnRef(cr) => {
            let col = sanitize_identifier(&cr.column);
            if let Some(ref table) = cr.table {
                let table_sanitized = sanitize_identifier(table);
                format!("{{{{ ref('stg_{table_sanitized}') }}}}.{col}")
            } else {
                col
            }
        }
        DaxExpr::FunctionCall(fc) => {
            format!("/* {} */ {}", fc.function_name, fc.function_name.to_lowercase())
        }
        DaxExpr::Literal(lit) => match lit {
            DaxLiteral::Integer(n) => n.to_string(),
            DaxLiteral::Float(f) => format!("{f:.2}"),
            DaxLiteral::String(s) => format!("'{s}'"),
            DaxLiteral::Boolean(b) => adapter.boolean_literal(*b),
            DaxLiteral::Blank => "null".to_string(),
        },
        DaxExpr::Raw(raw) => raw.clone(),
        _ => "/* untranslated */".to_string(),
    }
}

/// Translate SELECTCOLUMNS.
fn translate_select_columns(args: &[DaxExpr], adapter: &dyn SqlAdapter, _source_name: &str) -> String {
    if args.is_empty() {
        return "-- MANUAL_REVIEW: SELECTCOLUMNS without arguments".to_string();
    }

    let table = translate_calc_expr(&args[0], adapter);
    let mut columns = Vec::new();

    let mut i = 1;
    while i + 1 < args.len() {
        let alias = translate_calc_expr(&args[i], adapter);
        let expr = translate_calc_expr(&args[i + 1], adapter);
        let alias_clean = alias.trim_matches('\'').trim_matches('"');
        columns.push(format!("    {} as {}", expr, sanitize_identifier(alias_clean)));
        i += 2;
    }

    if columns.is_empty() {
        format!("select * from {table}")
    } else {
        format!("select\n{}\nfrom {table}", columns.join(",\n"))
    }
}

/// Translate ADDCOLUMNS.
fn translate_add_columns(args: &[DaxExpr], adapter: &dyn SqlAdapter, _source_name: &str) -> String {
    if args.is_empty() {
        return "-- MANUAL_REVIEW: ADDCOLUMNS without arguments".to_string();
    }

    let table = translate_calc_expr(&args[0], adapter);
    let mut extra_cols = Vec::new();

    let mut i = 1;
    while i + 1 < args.len() {
        let alias = translate_calc_expr(&args[i], adapter);
        let expr = translate_calc_expr(&args[i + 1], adapter);
        let alias_clean = alias.trim_matches('\'').trim_matches('"');
        extra_cols.push(format!("    {} as {}", expr, sanitize_identifier(alias_clean)));
        i += 2;
    }

    if extra_cols.is_empty() {
        format!("select * from {table}")
    } else {
        format!("select\n    *,\n{}\nfrom {table}", extra_cols.join(",\n"))
    }
}

/// Translate SUMMARIZE.
fn translate_summarize(args: &[DaxExpr], adapter: &dyn SqlAdapter, _source_name: &str) -> String {
    if args.len() < 2 {
        return "-- MANUAL_REVIEW: SUMMARIZE without enough arguments".to_string();
    }

    let table = translate_calc_expr(&args[0], adapter);
    let group_cols: Vec<String> = args[1..].iter()
        .map(|a| translate_calc_expr(a, adapter))
        .collect();
    let cols_str = group_cols.join(", ");

    format!("select {cols_str}\nfrom {table}\ngroup by {cols_str}")
}

/// Translate ROW.
fn translate_row(args: &[DaxExpr], adapter: &dyn SqlAdapter) -> String {
    let mut columns = Vec::new();
    let mut i = 0;
    while i + 1 < args.len() {
        let alias = translate_calc_expr(&args[i], adapter);
        let expr = translate_calc_expr(&args[i + 1], adapter);
        let alias_clean = alias.trim_matches('\'').trim_matches('"');
        columns.push(format!("{} as {}", expr, sanitize_identifier(alias_clean)));
        i += 2;
    }

    if columns.is_empty() {
        "select 1".to_string()
    } else {
        format!("select {}", columns.join(", "))
    }
}
