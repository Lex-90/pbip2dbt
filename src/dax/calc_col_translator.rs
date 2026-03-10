//! DAX calculated column translator (PRD Engine 4).

use crate::adapter::SqlAdapter;
use crate::dax::parser::parse_dax;
use crate::dax::ast::{DaxExpr, DaxOp, DaxLiteral};
use crate::dax::measure_translator;
use crate::naming::sanitize_identifier;
use crate::tmdl::ast::TranslationResult;

/// Translate a DAX calculated column expression to SQL.
///
/// Calculated columns are row-level expressions that become column aliases
/// in the staging model's SELECT list.
pub fn translate_calc_column(dax_expr: &str, adapter: &dyn SqlAdapter) -> TranslationResult {
    let parsed = parse_dax(dax_expr);
    let mut warnings = Vec::new();
    let mut confidence = 1.0;

    let sql = translate_column_expr(&parsed, adapter, &mut confidence, &mut warnings);

    TranslationResult {
        sql,
        confidence,
        warnings,
        manual_review: confidence < 0.8,
    }
}

/// Translate a column-level DAX expression to SQL.
fn translate_column_expr(
    expr: &DaxExpr,
    adapter: &dyn SqlAdapter,
    confidence: &mut f64,
    warnings: &mut Vec<crate::error::Warning>,
) -> String {
    match expr {
        DaxExpr::FunctionCall(fc) => {
            let upper = fc.function_name.to_uppercase();
            if upper.as_str() == "RELATED" {
                // RELATED requires a JOIN — flag for manual review
                *confidence = confidence.min(0.2);
                if let Some(arg) = fc.arguments.first() {
                    let col = translate_column_expr(arg, adapter, confidence, warnings);
                    warnings.push(crate::error::Warning {
                        code: "W011",
                        message: format!("RELATED({col}) requires a JOIN — consider moving to an intermediate model"),
                        source_context: format!("RELATED({col})"),
                        suggestion: "Move this column to an intermediate model with the required JOIN".to_string(),
                    });
                    format!("/* MANUAL_REVIEW: RELATED — needs JOIN */ {col}")
                } else {
                    "/* MANUAL_REVIEW: RELATED() */".to_string()
                }
            } else {
                // Use the measure translator for other functions
                let result = measure_translator::translate_measure(
                    &format_dax_expr(expr),
                    adapter,
                    0.0,
                );
                *confidence = confidence.min(result.confidence);
                result.sql
            }
        }
        DaxExpr::ColumnRef(cr) => {
            sanitize_identifier(&cr.column)
        }
        DaxExpr::MeasureRef(name) => {
            *confidence = confidence.min(0.4);
            format!("/* [{name}] */")
        }
        DaxExpr::BinaryOp(left, op, right) => {
            let l = translate_column_expr(left, adapter, confidence, warnings);
            let r = translate_column_expr(right, adapter, confidence, warnings);
            let op_str = match op {
                DaxOp::Add => "+",
                DaxOp::Sub => "-",
                DaxOp::Mul => "*",
                DaxOp::Div => "/",
                DaxOp::Eq => "=",
                DaxOp::Neq => "<>",
                DaxOp::Gt => ">",
                DaxOp::Lt => "<",
                DaxOp::Gte => ">=",
                DaxOp::Lte => "<=",
                DaxOp::And => "and",
                DaxOp::Or => "or",
                DaxOp::Concat => adapter.concat_op(),
            };
            format!("{l} {op_str} {r}")
        }
        DaxExpr::Literal(lit) => match lit {
            DaxLiteral::Integer(n) => n.to_string(),
            DaxLiteral::Float(f) => format!("{f:.2}"),
            DaxLiteral::String(s) => format!("'{s}'"),
            DaxLiteral::Boolean(b) => adapter.boolean_literal(*b),
            DaxLiteral::Blank => "null".to_string(),
        },
        DaxExpr::Raw(raw) => {
            if raw.is_empty() {
                String::new()
            } else {
                *confidence = confidence.min(0.5);
                format!("/* raw: {raw} */")
            }
        }
        _ => {
            *confidence = confidence.min(0.3);
            "/* untranslated */".to_string()
        }
    }
}

/// Format a DAX expression back to string (for delegation to measure translator).
fn format_dax_expr(expr: &DaxExpr) -> String {
    match expr {
        DaxExpr::FunctionCall(fc) => {
            let args: Vec<String> = fc.arguments.iter().map(format_dax_expr).collect();
            format!("{}({})", fc.function_name, args.join(", "))
        }
        DaxExpr::ColumnRef(cr) => {
            if let Some(ref table) = cr.table {
                format!("{}[{}]", table, cr.column)
            } else {
                format!("[{}]", cr.column)
            }
        }
        DaxExpr::MeasureRef(name) => format!("[{name}]"),
        DaxExpr::Literal(lit) => match lit {
            DaxLiteral::Integer(n) => n.to_string(),
            DaxLiteral::Float(f) => format!("{f}"),
            DaxLiteral::String(s) => format!("\"{s}\""),
            DaxLiteral::Boolean(b) => b.to_string(),
            DaxLiteral::Blank => "BLANK()".to_string(),
        },
        DaxExpr::Raw(raw) => raw.clone(),
        _ => String::new(),
    }
}
