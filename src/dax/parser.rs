//! DAX expression parser.
//!
//! Parses DAX expressions into an AST. Handles `'Table Name'[Column]` syntax,
//! VAR/RETURN blocks, nested function calls, and operators.

use super::ast::{DaxExpr, VarDecl, DaxFunctionCall, ColumnRef, DaxLiteral, DaxOp};

/// Parse a DAX expression string into a `DaxExpr`.
pub fn parse_dax(input: &str) -> DaxExpr {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return DaxExpr::Raw(String::new());
    }

    // Try VAR/RETURN pattern
    if let Some(vr) = try_parse_var_return(trimmed) {
        return vr;
    }

    parse_dax_expr(trimmed)
}

/// Try to parse a VAR...RETURN block.
fn try_parse_var_return(input: &str) -> Option<DaxExpr> {
    let upper = input.to_uppercase();
    if !upper.starts_with("VAR ") {
        return None;
    }

    let mut vars = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        let upper_line = trimmed.to_uppercase();

        if upper_line.starts_with("VAR ") {
            let rest = &trimmed[4..].trim();
            if let Some(eq_pos) = rest.find('=') {
                let name = rest[..eq_pos].trim().to_string();
                let expr_str = rest[eq_pos + 1..].trim().to_string();

                // Collect multiline expression
                let mut full_expr = expr_str;
                i += 1;
                while i < lines.len() {
                    let next = lines[i].trim();
                    let next_upper = next.to_uppercase();
                    if next_upper.starts_with("VAR ") || next_upper.starts_with("RETURN") {
                        break;
                    }
                    full_expr.push('\n');
                    full_expr.push_str(next);
                    i += 1;
                }

                vars.push(VarDecl {
                    name,
                    expression: parse_dax_expr(&full_expr),
                });
            } else {
                i += 1;
            }
        } else if upper_line.starts_with("RETURN") {
            let rest = trimmed[6..].trim();
            let mut return_expr = rest.to_string();
            i += 1;
            while i < lines.len() {
                return_expr.push('\n');
                return_expr.push_str(lines[i].trim());
                i += 1;
            }

            return Some(DaxExpr::VarReturn(
                vars,
                Box::new(parse_dax_expr(&return_expr)),
            ));
        } else {
            i += 1;
        }
    }

    if vars.is_empty() {
        None
    } else {
        // No explicit RETURN found, use last expression
        Some(DaxExpr::VarReturn(vars, Box::new(DaxExpr::Raw(String::new()))))
    }
}

/// Parse a DAX expression (non-VAR/RETURN).
fn parse_dax_expr(input: &str) -> DaxExpr {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return DaxExpr::Raw(String::new());
    }

    // Try function call
    if let Some(fc) = try_parse_dax_function(trimmed) {
        return DaxExpr::FunctionCall(fc);
    }

    // Try column reference: 'Table'[Column] or Table[Column] or [Column]
    if let Some(col_ref) = try_parse_column_ref(trimmed) {
        return DaxExpr::ColumnRef(col_ref);
    }

    // Measure reference: [MeasureName]
    if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed[1..].contains('[') {
        let name = trimmed[1..trimmed.len() - 1].to_string();
        return DaxExpr::MeasureRef(name);
    }

    // Literal values
    if let Some(lit) = try_parse_literal(trimmed) {
        return DaxExpr::Literal(lit);
    }

    // Binary operation with simple operators
    if let Some(binop) = try_parse_binary_op(trimmed) {
        return binop;
    }

    DaxExpr::Raw(trimmed.to_string())
}

/// Try to parse a DAX function call.
fn try_parse_dax_function(input: &str) -> Option<DaxFunctionCall> {
    let paren_pos = input.find('(')?;
    let func_name = input[..paren_pos].trim();

    // Validate function name
    if func_name.is_empty() || !func_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    // Find matching close paren
    let args_str = &input[paren_pos + 1..];
    let close_pos = find_matching_close_paren(args_str)?;
    let args_content = &args_str[..close_pos];

    let arguments = split_dax_args(args_content)
        .into_iter()
        .map(|a| parse_dax_expr(a.trim()))
        .collect();

    Some(DaxFunctionCall {
        function_name: func_name.to_uppercase(),
        arguments,
    })
}

/// Try to parse a column reference.
fn try_parse_column_ref(input: &str) -> Option<ColumnRef> {
    // 'Table Name'[Column]
    if input.starts_with('\'') {
        let end_quote = input[1..].find('\'')?;
        let table = input[1..=end_quote].to_string();
        let rest = &input[2 + end_quote..];
        if rest.starts_with('[') && rest.ends_with(']') {
            let column = rest[1..rest.len() - 1].to_string();
            return Some(ColumnRef {
                table: Some(table),
                column,
            });
        }
    }

    // Table[Column]
    if let Some(bracket_pos) = input.find('[') {
        if input.ends_with(']') {
            let table = input[..bracket_pos].trim();
            let column = input[bracket_pos + 1..input.len() - 1].to_string();
            if !table.is_empty()
                && table
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == ' ')
            {
                return Some(ColumnRef {
                    table: Some(table.to_string()),
                    column,
                });
            }
        }
    }

    None
}

/// Try to parse a literal.
fn try_parse_literal(input: &str) -> Option<DaxLiteral> {
    let upper = input.to_uppercase();

    if upper == "BLANK()" {
        return Some(DaxLiteral::Blank);
    }
    if upper == "TRUE()" || upper == "TRUE" {
        return Some(DaxLiteral::Boolean(true));
    }
    if upper == "FALSE()" || upper == "FALSE" {
        return Some(DaxLiteral::Boolean(false));
    }

    if input.starts_with('"') && input.ends_with('"') && input.len() >= 2 {
        return Some(DaxLiteral::String(input[1..input.len() - 1].to_string()));
    }

    if let Ok(n) = input.parse::<i64>() {
        return Some(DaxLiteral::Integer(n));
    }
    if let Ok(f) = input.parse::<f64>() {
        return Some(DaxLiteral::Float(f));
    }

    None
}

/// Try to parse a simple binary operation.
fn try_parse_binary_op(input: &str) -> Option<DaxExpr> {
    // Only handle simple `a op b` at the top level
    let operators = [
        (" + ", DaxOp::Add),
        (" - ", DaxOp::Sub),
        (" * ", DaxOp::Mul),
        (" / ", DaxOp::Div),
        (" && ", DaxOp::And),
        (" || ", DaxOp::Or),
        (" & ", DaxOp::Concat),
    ];

    for (op_str, op) in &operators {
        if let Some(pos) = find_top_level_op(input, op_str) {
            let left = parse_dax_expr(&input[..pos]);
            let right = parse_dax_expr(&input[pos + op_str.len()..]);
            return Some(DaxExpr::BinaryOp(Box::new(left), op.clone(), Box::new(right)));
        }
    }

    None
}

/// Find a top-level operator (not inside parens/brackets).
fn find_top_level_op(input: &str, op: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut in_string = false;
    let bytes = input.as_bytes();

    for i in 0..bytes.len() {
        if in_string {
            if bytes[i] == b'"' {
                in_string = false;
            }
            continue;
        }
        match bytes[i] {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            _ => {}
        }

        if depth == 0 && bracket_depth == 0 && i + op.len() <= bytes.len()
            && &input[i..i + op.len()] == op {
                return Some(i);
            }
    }
    None
}

/// Find matching close parenthesis.
fn find_matching_close_paren(input: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;

    for (i, ch) in input.char_indices() {
        if in_string {
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Split DAX function arguments by commas, respecting nesting.
fn split_dax_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut in_string = false;

    for (i, ch) in input.char_indices() {
        if in_string {
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            ',' if depth == 0 && bracket_depth == 0 => {
                args.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < input.len() {
        args.push(&input[start..]);
    } else if input.is_empty() {
        // No arguments
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_sum() {
        let result = parse_dax("SUM(Sales[Revenue])");
        assert!(matches!(result, DaxExpr::FunctionCall(_)));
        if let DaxExpr::FunctionCall(fc) = result {
            assert_eq!(fc.function_name, "SUM");
        }
    }

    #[test]
    fn parse_var_return() {
        let input = r#"VAR x = SUM(Sales[Revenue])
VAR y = SUM(Sales[Cost])
RETURN
    x - y"#;
        let result = parse_dax(input);
        assert!(matches!(result, DaxExpr::VarReturn(_, _)));
        if let DaxExpr::VarReturn(vars, _) = result {
            assert_eq!(vars.len(), 2);
            assert_eq!(vars[0].name, "x");
        }
    }

    #[test]
    fn parse_column_ref() {
        let result = try_parse_column_ref("Sales[Revenue]");
        assert!(result.is_some());
        let cr = result.unwrap();
        assert_eq!(cr.table, Some("Sales".to_string()));
        assert_eq!(cr.column, "Revenue");
    }

    #[test]
    fn parse_quoted_table_ref() {
        let result = try_parse_column_ref("'Date Table'[Date]");
        assert!(result.is_some());
        let cr = result.unwrap();
        assert_eq!(cr.table, Some("Date Table".to_string()));
        assert_eq!(cr.column, "Date");
    }

    #[test]
    fn parse_blank_literal() {
        let result = try_parse_literal("BLANK()");
        assert!(matches!(result, Some(DaxLiteral::Blank)));
    }

    #[test]
    fn parse_binary_op() {
        let result = parse_dax("[Revenue] - [Cost]");
        assert!(matches!(result, DaxExpr::BinaryOp(_, _, _)));
    }
}
