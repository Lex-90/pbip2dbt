//! DAX measure → SQL translator with confidence scoring.
//!
//! Implements PRD Engine 2: translates DAX measures into SQL expressions
//! with per-measure confidence scores based on the constructs used.

use crate::adapter::SqlAdapter;
use crate::dax::ast::{DaxExpr, DaxOp, DaxUnaryOp, DaxFunctionCall, DaxLiteral};
use crate::dax::parser::parse_dax;
use crate::error::Warning;
use crate::naming::sanitize_identifier;
use crate::tmdl::ast::TranslationResult;

/// Score-0.0 DAX functions that cannot be translated.
const UNTRANSLATABLE_FUNCTIONS: &[&str] = &[
    "CALCULATETABLE", "SUMMARIZECOLUMNS", "PATH", "PATHITEM", "PATHLENGTH",
    "USERELATIONSHIP", "CROSSFILTER", "DETAILROWS", "SELECTEDVALUE", "HASONEVALUE",
    "ISFILTERED", "ISCROSSFILTERED", "GENERATESERIES", "NATURALLEFTOUTERJOIN",
    "SUBSTITUTEWITHINDEX", "TREATAS",
];

/// Score-0.2 iterator functions.
const ITERATOR_FUNCTIONS: &[&str] = &[
    "SUMX", "AVERAGEX", "MAXX", "MINX", "COUNTX", "RANKX", "TOPN",
];

/// Score-0.6 time intelligence functions.
const TIME_INTELLIGENCE_FUNCTIONS: &[&str] = &[
    "SAMEPERIODLASTYEAR", "DATEADD", "TOTALYTD", "DATESYTD", "TOTALMTD",
    "DATESQTD", "TOTALQTD", "PREVIOUSMONTH", "PREVIOUSQUARTER", "PREVIOUSYEAR",
    "NEXTMONTH", "NEXTQUARTER", "NEXTYEAR", "PARALLELPERIOD",
];

/// Score-0.4 filter modifier functions.
const FILTER_MODIFIER_FUNCTIONS: &[&str] = &[
    "ALL", "ALLEXCEPT", "REMOVEFILTERS", "KEEPFILTERS", "ALLSELECTED",
    "ALLNOBLANKROW",
];

/// Translate a DAX measure to SQL with a confidence score.
pub fn translate_measure(
    dax_expr: &str,
    adapter: &dyn SqlAdapter,
    confidence_threshold: f64,
) -> TranslationResult {
    let parsed = parse_dax(dax_expr);
    let mut min_confidence = 1.0_f64;
    let mut warnings = Vec::new();

    // Scan for constructs and compute minimum confidence
    scan_confidence(&parsed, &mut min_confidence, &mut warnings);

    // Generate SQL
    let sql = if min_confidence == 0.0 || min_confidence < confidence_threshold {
        // Documentation-only
        format!("-- Original DAX (not translated): {}", dax_expr.trim())
    } else {
        let translated = translate_dax_to_sql(&parsed, adapter, &mut warnings);
        if min_confidence < 0.8 {
            format!(
                "-- Best-effort translation (confidence: {min_confidence:.2})\n{translated}"
            )
        } else {
            translated
        }
    };

    TranslationResult {
        sql,
        confidence: min_confidence,
        warnings,
        manual_review: min_confidence < 0.8,
    }
}

/// Recursively scan a DAX expression to find the minimum confidence score.
fn scan_confidence(expr: &DaxExpr, min_confidence: &mut f64, warnings: &mut Vec<Warning>) {
    match expr {
        DaxExpr::FunctionCall(fc) => {
            let upper = fc.function_name.to_uppercase();

            if UNTRANSLATABLE_FUNCTIONS.contains(&upper.as_str()) {
                *min_confidence = min_confidence.min(0.0);
                warnings.push(Warning {
                    code: "W010",
                    message: format!("{upper} has no SQL equivalent"),
                    source_context: upper.clone(),
                    suggestion: "Manual conversion required".to_string(),
                });
            } else if ITERATOR_FUNCTIONS.contains(&upper.as_str()) {
                *min_confidence = min_confidence.min(0.2);
                warnings.push(Warning {
                    code: "W009",
                    message: format!("{upper} translation uses correlated subquery — may not be semantically equivalent"),
                    source_context: upper.clone(),
                    suggestion: "Review the generated SQL for correctness".to_string(),
                });
            } else if upper == "CALCULATE" {
                // Check if args contain filter modifiers
                let has_modifiers = fc.arguments.iter().skip(1).any(|arg| {
                    if let DaxExpr::FunctionCall(inner_fc) = arg {
                        let inner_upper = inner_fc.function_name.to_uppercase();
                        FILTER_MODIFIER_FUNCTIONS.contains(&inner_upper.as_str())
                    } else {
                        false
                    }
                });
                let has_time_intel = fc.arguments.iter().skip(1).any(|arg| {
                    if let DaxExpr::FunctionCall(inner_fc) = arg {
                        let inner_upper = inner_fc.function_name.to_uppercase();
                        TIME_INTELLIGENCE_FUNCTIONS.contains(&inner_upper.as_str())
                    } else {
                        false
                    }
                });

                if has_modifiers {
                    *min_confidence = min_confidence.min(0.4);
                } else if has_time_intel {
                    *min_confidence = min_confidence.min(0.6);
                } else {
                    *min_confidence = min_confidence.min(0.8);
                }
            } else if TIME_INTELLIGENCE_FUNCTIONS.contains(&upper.as_str()) {
                *min_confidence = min_confidence.min(0.6);
            } else if FILTER_MODIFIER_FUNCTIONS.contains(&upper.as_str()) {
                *min_confidence = min_confidence.min(0.4);
            }

            // Recurse into arguments
            for arg in &fc.arguments {
                scan_confidence(arg, min_confidence, warnings);
            }
        }
        DaxExpr::VarReturn(vars, ret) => {
            for var in vars {
                scan_confidence(&var.expression, min_confidence, warnings);
            }
            scan_confidence(ret, min_confidence, warnings);
        }
        DaxExpr::BinaryOp(left, _, right) => {
            scan_confidence(left, min_confidence, warnings);
            scan_confidence(right, min_confidence, warnings);
        }
        DaxExpr::UnaryOp(_, inner) => {
            scan_confidence(inner, min_confidence, warnings);
        }
        DaxExpr::ColumnRef(_) | DaxExpr::MeasureRef(_) | DaxExpr::Literal(_) | DaxExpr::Raw(_) => {}
    }
}

/// Translate a DAX expression into SQL.
fn translate_dax_to_sql(expr: &DaxExpr, adapter: &dyn SqlAdapter, warnings: &mut Vec<Warning>) -> String {
    match expr {
        DaxExpr::FunctionCall(fc) => translate_function(fc, adapter, warnings),
        DaxExpr::ColumnRef(cr) => {
            sanitize_identifier(&cr.column)
        }
        DaxExpr::MeasureRef(name) => {
            format!("/* [{name}] */")
        }
        DaxExpr::Literal(lit) => translate_literal(lit, adapter),
        DaxExpr::BinaryOp(left, op, right) => {
            let l = translate_dax_to_sql(left, adapter, warnings);
            let r = translate_dax_to_sql(right, adapter, warnings);
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
        DaxExpr::UnaryOp(op, inner) => {
            let inner_sql = translate_dax_to_sql(inner, adapter, warnings);
            match op {
                DaxUnaryOp::Not => format!("not {inner_sql}"),
                DaxUnaryOp::Neg => format!("-{inner_sql}"),
            }
        }
        DaxExpr::VarReturn(vars, ret) => {
            // Inline variables
            let mut sql_parts = Vec::new();
            for var in vars {
                let var_sql = translate_dax_to_sql(&var.expression, adapter, warnings);
                sql_parts.push(format!("/* VAR {} = */ {}", var.name, var_sql));
            }
            let ret_sql = translate_dax_to_sql(ret, adapter, warnings);
            if sql_parts.is_empty() {
                ret_sql
            } else {
                format!("{}\n{}", sql_parts.join("\n"), ret_sql)
            }
        }
        DaxExpr::Raw(raw) => {
            // Best-effort: keep as-is with a comment
            format!("/* raw DAX: {} */", raw.trim())
        }
    }
}

/// Translate a DAX function call to SQL.
fn translate_function(fc: &DaxFunctionCall, adapter: &dyn SqlAdapter, warnings: &mut Vec<Warning>) -> String {
    let args: Vec<String> = fc.arguments.iter()
        .map(|a| translate_dax_to_sql(a, adapter, warnings))
        .collect();

    match fc.function_name.as_str() {
        // Score 1.0 — Direct mappings
        "SUM" => format!("sum({})", args.first().unwrap_or(&String::new())),
        "AVERAGE" => format!("avg({})", args.first().unwrap_or(&String::new())),
        "MIN" => format!("min({})", args.first().unwrap_or(&String::new())),
        "MAX" => format!("max({})", args.first().unwrap_or(&String::new())),
        "COUNT" => format!("count({})", args.first().unwrap_or(&String::new())),
        "COUNTA" => format!("count({})", args.first().unwrap_or(&String::new())),
        "COUNTROWS" => "count(*)".to_string(),
        "DISTINCTCOUNT" => format!("count(distinct {})", args.first().unwrap_or(&String::new())),
        "DIVIDE" => {
            let a = args.first().cloned().unwrap_or_default();
            let b = args.get(1).cloned().unwrap_or_default();
            if let Some(alt) = args.get(2) {
                format!("coalesce({a} / {}, {alt})", adapter.nullif(&b, "0"))
            } else {
                format!("{a} / {}", adapter.nullif(&b, "0"))
            }
        }
        "IF" => {
            let cond = args.first().cloned().unwrap_or_default();
            let t = args.get(1).cloned().unwrap_or_default();
            let f = args.get(2).cloned().unwrap_or("null".to_string());
            adapter.iif(&cond, &t, &f)
        }
        "SWITCH" => {
            let expr = args.first().cloned().unwrap_or_default();
            let mut case_str = format!("case {expr}");
            let mut i = 1;
            while i + 1 < args.len() {
                case_str.push_str(&format!(" when {} then {}", args[i], args[i + 1]));
                i += 2;
            }
            if i < args.len() {
                case_str.push_str(&format!(" else {}", args[i]));
            }
            case_str.push_str(" end");
            case_str
        }
        "BLANK" => "null".to_string(),
        "ISBLANK" => format!("{} is null", args.first().unwrap_or(&String::new())),
        "CONCATENATE" => {
            let a = args.first().cloned().unwrap_or_default();
            let b = args.get(1).cloned().unwrap_or_default();
            format!("concat({a}, {b})")
        }
        "LEFT" => {
            let text = args.first().cloned().unwrap_or_default();
            let n = args.get(1).cloned().unwrap_or("1".to_string());
            adapter.left_substr(&text, &n)
        }
        "RIGHT" => {
            let text = args.first().cloned().unwrap_or_default();
            let n = args.get(1).cloned().unwrap_or("1".to_string());
            adapter.right_substr(&text, &n)
        }
        "LEN" => adapter.string_length(args.first().unwrap_or(&String::new())),
        "UPPER" => format!("upper({})", args.first().unwrap_or(&String::new())),
        "LOWER" => format!("lower({})", args.first().unwrap_or(&String::new())),
        "TRIM" => format!("trim({})", args.first().unwrap_or(&String::new())),
        "YEAR" => adapter.extract_year(args.first().unwrap_or(&String::new())),
        "MONTH" => adapter.extract_month(args.first().unwrap_or(&String::new())),
        "DAY" => adapter.extract_day(args.first().unwrap_or(&String::new())),
        "TODAY" => adapter.current_date(),
        "NOW" => adapter.current_timestamp(),
        "ROUND" => {
            let expr = args.first().cloned().unwrap_or_default();
            let n = args.get(1).cloned().unwrap_or("0".to_string());
            format!("round({expr}, {n})")
        }
        "ABS" => format!("abs({})", args.first().unwrap_or(&String::new())),
        "INT" => format!("cast(floor({}) as integer)", args.first().unwrap_or(&String::new())),
        "TRUE" => adapter.boolean_literal(true),
        "FALSE" => adapter.boolean_literal(false),
        "AND" => format!("{} and {}", args.first().unwrap_or(&String::new()), args.get(1).unwrap_or(&String::new())),
        "OR" => format!("{} or {}", args.first().unwrap_or(&String::new()), args.get(1).unwrap_or(&String::new())),
        "NOT" => format!("not {}", args.first().unwrap_or(&String::new())),
        "FORMAT" => format!("/* FORMAT: */ cast({} as varchar)", args.first().unwrap_or(&String::new())),
        "CONTAINSSTRING" => {
            let text = args.first().cloned().unwrap_or_default();
            let search = args.get(1).cloned().unwrap_or_default();
            format!("{text} like '%' {concat} {search} {concat} '%'", concat = adapter.concat_op())
        }

        // Score 0.8 — CALCULATE with simple filter
        "CALCULATE" => {
            let measure = args.first().cloned().unwrap_or_default();
            // Simple CALCULATE: just emit the measure with filter comments
            if args.len() > 1 {
                let filters: Vec<String> = args[1..].to_vec();
                format!("{measure} /* CALCULATE filters: {} */", filters.join(", "))
            } else {
                measure
            }
        }

        // Score 0.2 — Iterators
        "SUMX" => {
            let expr = args.get(1).cloned().unwrap_or_default();
            format!("sum({expr})")
        }
        "AVERAGEX" => {
            let expr = args.get(1).cloned().unwrap_or_default();
            format!("avg({expr})")
        }
        "FILTER" => {
            let table = args.first().cloned().unwrap_or_default();
            let cond = args.get(1).cloned().unwrap_or_default();
            format!("/* FILTER({table}, {cond}) */")
        }
        "RANKX" => {
            let expr = args.get(1).cloned().unwrap_or_default();
            format!("rank() over (order by {expr})")
        }

        _ => {
            // Unknown function: emit as-is with comment
            let args_str = args.join(", ");
            format!("/* {} */ {}({})", fc.function_name, fc.function_name.to_lowercase(), args_str)
        }
    }
}

/// Translate a DAX literal to SQL.
fn translate_literal(lit: &DaxLiteral, adapter: &dyn SqlAdapter) -> String {
    match lit {
        DaxLiteral::Integer(n) => n.to_string(),
        DaxLiteral::Float(f) => format!("{f:.2}"),
        DaxLiteral::String(s) => format!("'{s}'"),
        DaxLiteral::Boolean(b) => adapter.boolean_literal(*b),
        DaxLiteral::Blank => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::postgres::PostgresAdapter;

    #[test]
    fn translate_sum() {
        let result = translate_measure("SUM(Sales[Revenue])", &PostgresAdapter, 0.0);
        assert_eq!(result.confidence, 1.0);
        assert!(result.sql.contains("sum("));
    }

    #[test]
    fn translate_divide() {
        let result = translate_measure("DIVIDE(SUM(Sales[Revenue]), SUM(Sales[Cost]))", &PostgresAdapter, 0.0);
        assert_eq!(result.confidence, 1.0);
        assert!(result.sql.contains("nullif("));
    }

    #[test]
    fn translate_if() {
        let result = translate_measure("IF(SUM(Sales[Revenue]) > 0, SUM(Sales[Revenue]), 0)", &PostgresAdapter, 0.0);
        assert_eq!(result.confidence, 1.0);
        assert!(result.sql.contains("case when"));
    }

    #[test]
    fn untranslatable_function_scores_zero() {
        let result = translate_measure("CALCULATETABLE(Sales, Sales[Region] = \"West\")", &PostgresAdapter, 0.0);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn calculate_scores_0_8() {
        let result = translate_measure("CALCULATE(SUM(Sales[Revenue]), Sales[Region] = \"West\")", &PostgresAdapter, 0.0);
        assert!(result.confidence <= 0.8);
    }

    #[test]
    fn iterator_scores_0_2() {
        let result = translate_measure("SUMX(Sales, Sales[Price] * Sales[Qty])", &PostgresAdapter, 0.0);
        assert!(result.confidence <= 0.2);
    }
}
