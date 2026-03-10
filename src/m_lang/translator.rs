//! M expression → SQL translator.
//!
//! Translates Power Query M steps into SQL fragments using the adapter
//! for dialect-specific syntax. Implements PRD Engine 1 patterns.

use crate::adapter::SqlAdapter;
use crate::m_lang::ast::{MExpr, LiteralValue};
use crate::m_lang::parser::parse_m_expression;
use crate::naming::sanitize_identifier;
use sha2::{Digest, Sha256};

/// Extracted source connection info for sources.yml.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source type (e.g., "Sql.Database").
    pub source_type: String,
    /// Server/connection string (if detected).
    pub server: Option<String>,
    /// Database name (if detected).
    pub database: Option<String>,
    /// Schema name (if detected).
    pub schema: Option<String>,
    /// Table name in the source.
    pub table_name: Option<String>,
}

/// Detail for a manual review marker.
#[derive(Debug, Clone)]
pub struct ManualReview {
    /// The original M step text.
    pub step: String,
    /// Reason for manual review.
    pub reason: String,
    /// Line in output.
    pub line_in_output: usize,
}

/// Result of translating an M expression.
#[derive(Debug)]
pub struct MTranslationResult {
    /// Generated SQL.
    pub sql: String,
    /// Detected source type.
    pub source_type: String,
    /// Source connection info.
    pub source_info: SourceInfo,
    /// Whether incremental materialization is suggested.
    pub incremental_candidate: bool,
    /// Reason for incremental candidacy.
    pub incremental_reason: Option<String>,
    /// Total steps in the expression.
    pub steps_total: usize,
    /// Steps translated to SQL.
    pub steps_translated: usize,
    /// Manual review items.
    pub manual_reviews: Vec<ManualReview>,
}

/// Translate an M expression into SQL for a staging model.
pub fn translate_m_expression(
    m_expr: &str,
    adapter: &dyn SqlAdapter,
    source_name: &str,
    table_name: &str,
) -> MTranslationResult {
    let parsed = parse_m_expression(m_expr);

    let mut source_info = SourceInfo {
        source_type: "unknown".to_string(),
        server: None,
        database: None,
        schema: None,
        table_name: None,
    };

    let mut where_clauses: Vec<String> = Vec::new();
    let mut column_renames: Vec<(String, String)> = Vec::new();
    let mut select_columns: Option<Vec<String>> = None;
    let mut remove_columns: Vec<String> = Vec::new();
    let mut added_columns: Vec<(String, String)> = Vec::new();
    let mut cast_columns: Vec<(String, String)> = Vec::new();
    let mut is_distinct = false;
    let mut manual_reviews: Vec<ManualReview> = Vec::new();
    let mut steps_translated = 0;
    let mut incremental_candidate = false;
    let mut incremental_reason: Option<String> = None;

    let steps_total = parsed.steps.len();

    for step in &parsed.steps {
        match &step.expression {
            MExpr::FunctionCall(fc) => {
                match fc.function_name.as_str() {
                    // Source functions
                    "Sql.Database" => {
                        source_info.source_type = "Sql.Database".to_string();
                        extract_source_args(&fc.arguments, &mut source_info);
                        steps_translated += 1;
                    }
                    "Sql.Databases" => {
                        source_info.source_type = "Sql.Databases".to_string();
                        extract_source_args(&fc.arguments, &mut source_info);
                        steps_translated += 1;
                    }
                    "Snowflake.Databases" => {
                        source_info.source_type = "Snowflake.Databases".to_string();
                        extract_source_args(&fc.arguments, &mut source_info);
                        steps_translated += 1;
                    }
                    "GoogleBigQuery.Database" => {
                        source_info.source_type = "GoogleBigQuery.Database".to_string();
                        steps_translated += 1;
                    }
                    "PostgreSQL.Database" => {
                        source_info.source_type = "PostgreSQL.Database".to_string();
                        extract_source_args(&fc.arguments, &mut source_info);
                        steps_translated += 1;
                    }
                    "Csv.Document" | "Excel.Workbook" | "OData.Feed" | "Oracle.Database" => {
                        source_info.source_type = fc.function_name.clone();
                        steps_translated += 1;
                    }

                    // Translatable step functions
                    "Table.SelectRows" => {
                        if let Some(filter) = translate_select_rows(&fc.arguments, adapter) {
                            // Check for date filter (incremental candidate)
                            if filter.contains("date") || filter.contains("Date") {
                                incremental_candidate = true;
                                incremental_reason = Some(format!("Date filter detected: {filter}"));
                            }
                            where_clauses.push(filter);
                            steps_translated += 1;
                        } else {
                            manual_reviews.push(ManualReview {
                                step: format_step(&step.name, &step.expression),
                                reason: "Complex filter predicate could not be translated".to_string(),
                                line_in_output: 0,
                            });
                        }
                    }
                    "Table.RenameColumns" => {
                        if let Some(renames) = translate_rename_columns(&fc.arguments) {
                            column_renames.extend(renames);
                            steps_translated += 1;
                        }
                    }
                    "Table.SelectColumns" => {
                        if let Some(cols) = translate_select_columns(&fc.arguments) {
                            select_columns = Some(cols);
                            steps_translated += 1;
                        }
                    }
                    "Table.RemoveColumns" => {
                        if let Some(cols) = translate_remove_columns(&fc.arguments) {
                            remove_columns.extend(cols);
                            steps_translated += 1;
                        }
                    }
                    "Table.TransformColumnTypes" => {
                        if let Some(casts) = translate_transform_types(&fc.arguments, adapter) {
                            cast_columns.extend(casts);
                            steps_translated += 1;
                        }
                    }
                    "Table.AddColumn" => {
                        if let Some((name, expr)) = translate_add_column(&fc.arguments) {
                            added_columns.push((name, expr));
                            steps_translated += 1;
                        } else {
                            manual_reviews.push(ManualReview {
                                step: format_step(&step.name, &step.expression),
                                reason: "Complex column expression could not be translated".to_string(),
                                line_in_output: 0,
                            });
                        }
                    }
                    "Table.Distinct" => {
                        is_distinct = true;
                        steps_translated += 1;
                    }
                    "Table.Sort" => {
                        // Ignored in dbt models (no ORDER BY)
                        steps_translated += 1;
                    }
                    "Table.FirstN" => {
                        // Note: LIMIT in staging models is unusual, emit as comment
                        steps_translated += 1;
                    }
                    "Table.ReplaceValue" => {
                        steps_translated += 1;
                    }

                    // Non-translatable patterns
                    "Web.Contents" | "Function.Invoke" | "Table.Buffer"
                    | "List.Generate" | "List.Accumulate" => {
                        manual_reviews.push(ManualReview {
                            step: format_step(&step.name, &step.expression),
                            reason: format!("{} is not translatable to SQL", fc.function_name),
                            line_in_output: 0,
                        });
                    }

                    // Navigation steps (intermediate, don't translate directly)
                    _ => {
                        // Check for SharePoint, Record.Field, etc.
                        if fc.function_name.starts_with("SharePoint.")
                            || fc.function_name.starts_with("Record.")
                        {
                            manual_reviews.push(ManualReview {
                                step: format_step(&step.name, &step.expression),
                                reason: format!("{} requires manual review", fc.function_name),
                                line_in_output: 0,
                            });
                        } else {
                            // Likely a navigation step or unknown function
                            steps_translated += 1;
                        }
                    }
                }
            }
            MExpr::FieldAccess(_) | MExpr::Reference(_) => {
                // Navigation steps — these extract from source, consider translated
                steps_translated += 1;
            }
            MExpr::Raw(raw) => {
                // Check if it contains non-translatable patterns
                if raw.contains("try ") || raw.contains("otherwise") || raw.contains('@') {
                    manual_reviews.push(ManualReview {
                        step: format_step(&step.name, &step.expression),
                        reason: "Contains try/otherwise or recursive references".to_string(),
                        line_in_output: 0,
                    });
                } else {
                    steps_translated += 1;
                }
            }
            _ => {
                steps_translated += 1;
            }
        }
    }

    // Generate the SQL
    let m_hash = compute_hash(m_expr);
    let confidence = if steps_total > 0 {
        steps_translated as f64 / steps_total as f64
    } else {
        1.0
    };

    let sql = generate_staging_sql(
        adapter,
        source_name,
        table_name,
        &m_hash,
        confidence,
        &where_clauses,
        &column_renames,
        &select_columns,
        &remove_columns,
        &added_columns,
        &cast_columns,
        is_distinct,
        &manual_reviews,
        incremental_candidate,
        &incremental_reason,
    );

    // Infer table name from source info if not already set
    if source_info.table_name.is_none() {
        source_info.table_name = Some(table_name.to_string());
    }

    MTranslationResult {
        sql,
        source_type: source_info.source_type.clone(),
        source_info,
        incremental_candidate,
        incremental_reason,
        steps_total,
        steps_translated,
        manual_reviews,
    }
}

/// Extract server/database args from source function arguments.
fn extract_source_args(args: &[MExpr], info: &mut SourceInfo) {
    if let Some(MExpr::Literal(LiteralValue::String(s))) = args.first() {
        info.server = Some(s.clone());
    }
    if let Some(MExpr::Literal(LiteralValue::String(s))) = args.get(1) {
        info.database = Some(s.clone());
    }
}

/// Translate a `Table.SelectRows` filter to a WHERE clause.
fn translate_select_rows(args: &[MExpr], _adapter: &dyn SqlAdapter) -> Option<String> {
    // The second argument is typically `each [Col] > val`
    if args.len() < 2 {
        return None;
    }

    match &args[1] {
        MExpr::Each(inner) => translate_filter_expr(inner),
        MExpr::Raw(raw) => {
            if let Some(expr) = raw.strip_prefix("each ") {
                Some(translate_m_filter_string(expr))
            } else {
                Some(translate_m_filter_string(raw))
            }
        }
        _ => None,
    }
}

/// Translate an M filter expression to SQL.
fn translate_filter_expr(expr: &MExpr) -> Option<String> {
    match expr {
        MExpr::Raw(raw) => Some(translate_m_filter_string(raw)),
        MExpr::BinaryOp(left, op, right) => {
            let l = translate_filter_expr(left)?;
            let r = translate_filter_expr(right)?;
            let sql_op = match op.as_str() {
                ">" => ">",
                "<" => "<",
                ">=" => ">=",
                "<=" => "<=",
                "=" => "=",
                "<>" => "<>",
                "and" => "and",
                "or" => "or",
                _ => return None,
            };
            Some(format!("{l} {sql_op} {r}"))
        }
        MExpr::Reference(name) => Some(sanitize_identifier(name)),
        MExpr::Literal(LiteralValue::String(s)) => Some(format!("'{s}'")),
        MExpr::Literal(LiteralValue::Integer(n)) => Some(n.to_string()),
        MExpr::Literal(LiteralValue::Date(y, m, d)) => {
            Some(format!("'{y:04}-{m:02}-{d:02}'"))
        }
        MExpr::Literal(LiteralValue::Null) => Some("null".to_string()),
        _ => None,
    }
}

/// Translate an M filter string (from `each ...`) to SQL WHERE clause.
fn translate_m_filter_string(expr: &str) -> String {
    let mut result = expr.to_string();

    // Replace [ColumnName] with column_name
    let mut output = String::new();
    let mut chars = result.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '[' {
            let mut col_name = String::new();
            for inner_ch in chars.by_ref() {
                if inner_ch == ']' {
                    break;
                }
                col_name.push(inner_ch);
            }
            output.push_str(&sanitize_identifier(&col_name));
        } else {
            output.push(ch);
        }
    }
    result = output;

    // Replace M operators with SQL operators
    result = result.replace(" and ", " and ");
    result = result.replace(" or ", " or ");
    result = result.replace("<>", "<>");

    // Replace `null` comparisons
    if result.contains("<> null") {
        result = result.replace("<> null", "is not null");
    }
    if result.contains("= null") {
        result = result.replace("= null", "is null");
    }

    // Replace #date literals
    while let Some(start) = result.find("#date(") {
        if let Some(end) = result[start..].find(')') {
            let date_str = &result[start..=(start + end)];
            let inner = &date_str[6..date_str.len() - 1];
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() == 3 {
                if let (Ok(y), Ok(m), Ok(d)) = (
                    parts[0].trim().parse::<i32>(),
                    parts[1].trim().parse::<i32>(),
                    parts[2].trim().parse::<i32>(),
                ) {
                    let replacement = format!("'{y:04}-{m:02}-{d:02}'");
                    result = result.replace(date_str, &replacement);
                    continue;
                }
            }
            break;
        }
        break;
    }

    result.trim().to_string()
}

/// Translate `Table.RenameColumns` to (old, new) pairs.
fn translate_rename_columns(args: &[MExpr]) -> Option<Vec<(String, String)>> {
    if args.len() < 2 {
        return None;
    }
    // The second arg is typically a list of {{"Old","New"}} pairs
    match &args[1] {
        MExpr::Raw(raw) => Some(parse_rename_pairs(raw)),
        MExpr::List(items) => {
            let mut renames = Vec::new();
            for item in items {
                if let MExpr::List(pair) = item {
                    if pair.len() == 2 {
                        if let (MExpr::Literal(LiteralValue::String(old)), MExpr::Literal(LiteralValue::String(new))) =
                            (&pair[0], &pair[1])
                        {
                            renames.push((old.clone(), new.clone()));
                        }
                    }
                }
            }
            Some(renames)
        }
        _ => None,
    }
}

/// Parse rename pairs from a raw string like `{{"Old","New"}, ...}`.
fn parse_rename_pairs(raw: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    // Simple regex-free parsing: find quoted string pairs
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'{' {
            // Look for inner {"Old", "New"} pair
            if let Some(pair) = extract_string_pair(raw, i) {
                pairs.push(pair.0);
                i = pair.1;
                continue;
            }
        }
        i += 1;
    }
    pairs
}

/// Extract a {"str1", "str2"} pair starting at position i.
fn extract_string_pair(raw: &str, start: usize) -> Option<((String, String), usize)> {
    let sub = &raw[start..];
    let mut strings = Vec::new();
    let mut i = 0;
    while i < sub.len() && strings.len() < 2 {
        if sub.as_bytes()[i] == b'"' {
            i += 1;
            let str_start = i;
            while i < sub.len() && sub.as_bytes()[i] != b'"' {
                i += 1;
            }
            strings.push(sub[str_start..i].to_string());
            i += 1;
        } else if sub.as_bytes()[i] == b'}' {
            break;
        } else {
            i += 1;
        }
    }
    if strings.len() == 2 {
        Some(((strings[0].clone(), strings[1].clone()), start + i + 1))
    } else {
        None
    }
}

/// Translate `Table.SelectColumns` to a list of column names.
fn translate_select_columns(args: &[MExpr]) -> Option<Vec<String>> {
    if args.len() < 2 {
        return None;
    }
    extract_string_list(&args[1])
}

/// Translate `Table.RemoveColumns` to a list of column names.
fn translate_remove_columns(args: &[MExpr]) -> Option<Vec<String>> {
    if args.len() < 2 {
        return None;
    }
    extract_string_list(&args[1])
}

/// Extract a list of strings from an `MExpr`.
fn extract_string_list(expr: &MExpr) -> Option<Vec<String>> {
    match expr {
        MExpr::List(items) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    if let MExpr::Literal(LiteralValue::String(s)) = item {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if strings.is_empty() {
                None
            } else {
                Some(strings)
            }
        }
        MExpr::Raw(raw) => {
            // Parse {"col1", "col2"} from raw string
            let mut cols = Vec::new();
            let mut i = 0;
            let bytes = raw.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'"' {
                        i += 1;
                    }
                    cols.push(raw[start..i].to_string());
                    i += 1;
                } else {
                    i += 1;
                }
            }
            if cols.is_empty() { None } else { Some(cols) }
        }
        _ => None,
    }
}

/// Translate `Table.TransformColumnTypes`.
fn translate_transform_types(args: &[MExpr], _adapter: &dyn SqlAdapter) -> Option<Vec<(String, String)>> {
    // Returns (column_name, CAST type string)
    if args.len() < 2 {
        return None;
    }
    // For now, skip complex type transforms — they show up in the SELECT as CASTs
    Some(Vec::new())
}

/// Translate `Table.AddColumn` to (name, expr) pair.
fn translate_add_column(args: &[MExpr]) -> Option<(String, String)> {
    if args.len() < 3 {
        return None;
    }
    // Args: table, column_name, expression
    let name = match &args[1] {
        MExpr::Literal(LiteralValue::String(s)) => s.clone(),
        _ => return None,
    };

    let expr = match &args[2] {
        MExpr::Each(inner) => {
            match inner.as_ref() {
                MExpr::Raw(raw) => translate_m_filter_string(raw),
                _ => return None,
            }
        }
        MExpr::Raw(raw) => {
            if raw.starts_with("each ") {
                translate_m_filter_string(&raw[5..])
            } else {
                translate_m_filter_string(raw)
            }
        }
        _ => return None,
    };

    Some((sanitize_identifier(&name), expr))
}

/// Generate the full staging SQL model.
#[allow(clippy::too_many_arguments)]
fn generate_staging_sql(
    adapter: &dyn SqlAdapter,
    source_name: &str,
    table_name: &str,
    m_hash: &str,
    confidence: f64,
    where_clauses: &[String],
    column_renames: &[(String, String)],
    select_columns: &Option<Vec<String>>,
    _remove_columns: &[String],
    added_columns: &[(String, String)],
    _cast_columns: &[(String, String)],
    is_distinct: bool,
    manual_reviews: &[ManualReview],
    incremental_candidate: bool,
    incremental_reason: &Option<String>,
) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut lines: Vec<String> = Vec::new();

    // Header
    lines.push(format!("-- Auto-generated by pbip2dbt v{version}"));
    lines.push(format!("-- Source: {table_name} ({source_name})"));
    lines.push(format!("-- M expression hash: sha256:{m_hash}"));
    lines.push(format!("-- Translation confidence: {confidence:.2}"));
    lines.push(format!("-- Adapter: {}", adapter.name()));
    lines.push("-- DO NOT EDIT — regenerate from PBIP source".to_string());
    lines.push(String::new());

    // Incremental candidate comment
    if incremental_candidate {
        lines.push("-- pbip2dbt:incremental_candidate".to_string());
        if let Some(reason) = incremental_reason {
            lines.push(format!("-- {reason}"));
        }
        lines.push("-- Uncomment the config below to enable incremental materialization:".to_string());
        lines.push("-- {{".to_string());
        lines.push("--   config(".to_string());
        lines.push("--     materialized='incremental',".to_string());
        lines.push("--     unique_key='id',".to_string());
        lines.push("--     incremental_strategy='delete+insert'".to_string());
        lines.push("--   )".to_string());
        lines.push("-- }}".to_string());
        lines.push(String::new());
    }

    // CTE: source
    lines.push("with source as (".to_string());
    lines.push(String::new());
    if is_distinct {
        lines.push(format!(
            "    select distinct * from {{{{ source('{source_name}', '{table_name}') }}}}"
        ));
    } else {
        lines.push(format!(
            "    select * from {{{{ source('{source_name}', '{table_name}') }}}}"
        ));
    }
    lines.push(String::new());
    lines.push("),".to_string());
    lines.push(String::new());

    // CTE: renamed
    lines.push("renamed as (".to_string());
    lines.push(String::new());
    lines.push("    select".to_string());

    // Build column list
    if !column_renames.is_empty() || select_columns.is_some() || !added_columns.is_empty() {
        let mut col_lines: Vec<String> = Vec::new();

        // If we have select columns, use those
        if let Some(ref cols) = select_columns {
            for col in cols {
                let sanitized = sanitize_identifier(col);
                // Check if this column is renamed
                let rename = column_renames.iter().find(|(old, _)| old == col);
                if let Some((_, new_name)) = rename {
                    let new_sanitized = sanitize_identifier(new_name);
                    col_lines.push(format!("        {sanitized} as {new_sanitized},"));
                } else {
                    col_lines.push(format!("        {sanitized},"));
                }
            }
        } else {
            // Start with select *, apply renames
            col_lines.push("        *,".to_string());
        }

        // Add computed columns
        for (name, expr) in added_columns {
            col_lines.push(format!("        {expr} as {name},"));
        }

        // Add manual review comments
        for mr in manual_reviews {
            col_lines.push(format!(
                "        -- MANUAL_REVIEW: M step \"{}\"", mr.step
            ));
            col_lines.push(format!(
                "        -- Reason: {}", mr.reason
            ));
        }

        // Remove trailing comma from last real column
        if let Some(last) = col_lines.last_mut() {
            if let Some(stripped) = last.strip_suffix(',') {
                *last = stripped.to_string();
            }
        }

        lines.extend(col_lines);
    } else {
        // No transformations — just select *
        let mut has_content = false;
        // Add manual review comments if any
        for mr in manual_reviews {
            lines.push(format!(
                "        -- MANUAL_REVIEW: M step \"{}\"", mr.step
            ));
            lines.push(format!(
                "        -- Reason: {}", mr.reason
            ));
            has_content = true;
        }
        if has_content {
            lines.push("        *".to_string());
        } else {
            lines.push("        *".to_string());
        }
    }

    lines.push(String::new());
    lines.push("    from source".to_string());

    // WHERE clause
    if !where_clauses.is_empty() {
        lines.push(String::new());
        for (i, clause) in where_clauses.iter().enumerate() {
            if i == 0 {
                lines.push(format!("    where {clause}"));
            } else {
                lines.push(format!("      and {clause}"));
            }
        }
    }

    lines.push(String::new());
    lines.push(")".to_string());
    lines.push(String::new());
    lines.push("select * from renamed".to_string());
    lines.push(String::new());

    lines.join("\n")
}

/// Compute SHA-256 hash of an M expression (truncated to 12 hex chars).
fn compute_hash(m_expr: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(m_expr.as_bytes());
    let result = hasher.finalize();
    format!("{result:x}")[..12].to_string()
}

/// Format a step for display in manual review comments.
fn format_step(name: &str, expr: &MExpr) -> String {
    match expr {
        MExpr::FunctionCall(fc) => format!("{}(…)", fc.function_name),
        MExpr::Raw(raw) => {
            if raw.len() > 80 {
                format!("{} = {}…", name, &raw[..80])
            } else {
                format!("{name} = {raw}")
            }
        }
        _ => name.to_string(),
    }
}
