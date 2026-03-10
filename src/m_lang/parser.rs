//! Power Query M expression parser.
//!
//! Parses M `let...in` expressions into a structured AST.
//! Handles `#"Quoted Name"` identifiers, nested function calls,
//! `each` shorthand, and list/record literals.

use super::ast::{LetExpr, MStep, MExpr, LiteralValue, FunctionCall};

/// Parse an M expression string into a `LetExpr`.
///
/// If the input doesn't contain a `let...in` structure, it wraps
/// the entire expression as a single raw step.
pub fn parse_m_expression(input: &str) -> LetExpr {
    let input = input.trim();

    // Try to parse as let...in
    if let Some(let_expr) = try_parse_let(input) {
        return let_expr;
    }

    // Fallback: wrap as a single raw expression
    LetExpr {
        steps: vec![MStep {
            name: "Source".to_string(),
            expression: MExpr::Raw(input.to_string()),
        }],
        result_step: "Source".to_string(),
    }
}

/// Try to parse a `let...in` expression.
fn try_parse_let(input: &str) -> Option<LetExpr> {
    // Find `let` keyword (case-insensitive, but M is case-sensitive so use exact)
    let lower = input.to_lowercase();
    let let_pos = lower.find("let")?;

    // Find the matching `in` — need to handle nested lets
    let after_let = &input[let_pos + 3..];
    let in_pos = find_matching_in(after_let)?;

    let bindings_str = after_let[..in_pos].trim();
    let result_str = after_let[in_pos + 2..].trim();

    let steps = parse_bindings(bindings_str);
    let result_step = result_str
        .trim_end_matches(|c: char| c.is_whitespace())
        .to_string();

    // Clean up the result step name
    let result_step = clean_step_name(&result_step);

    Some(LetExpr {
        steps,
        result_step,
    })
}

/// Find the matching `in` keyword for a `let`, handling nesting.
fn find_matching_in(input: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip strings
        if bytes[i] == b'"' {
            i += 1;
            while i < len && bytes[i] != b'"' {
                if bytes[i] == b'"' && i + 1 < len && bytes[i + 1] == b'"' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            i += 1;
            continue;
        }

        // Check for `let` (increase depth)
        if i + 3 <= len && &input[i..i + 3] == "let" {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + 3 >= len || !bytes[i + 3].is_ascii_alphanumeric();
            if before_ok && after_ok {
                depth += 1;
            }
        }

        // Check for `in` (decrease depth or match)
        if i + 2 <= len && &input[i..i + 2] == "in" {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + 2 >= len || !bytes[i + 2].is_ascii_alphanumeric();
            if before_ok && after_ok {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
        }

        i += 1;
    }
    None
}

/// Parse the bindings section of a let expression.
fn parse_bindings(input: &str) -> Vec<MStep> {
    let mut steps = Vec::new();
    let mut current_name = String::new();
    let mut current_expr = String::new();
    let mut in_string = false;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this is a new binding (Name = expr)
        if !in_string && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            if let Some((name, expr)) = try_split_binding(trimmed) {
                // Save previous binding
                if !current_name.is_empty() {
                    let expr_str = current_expr.trim_end_matches(',').trim().to_string();
                    steps.push(MStep {
                        name: current_name.clone(),
                        expression: parse_expr(&expr_str),
                    });
                }
                current_name = name;
                current_expr = expr.to_string();

                // Track nesting
                update_depth(&current_expr, &mut paren_depth, &mut bracket_depth, &mut brace_depth, &mut in_string);
                continue;
            }
        }

        // Continuation of current expression
        if !current_name.is_empty() {
            current_expr.push('\n');
            current_expr.push_str(trimmed);
        }

        update_depth(trimmed, &mut paren_depth, &mut bracket_depth, &mut brace_depth, &mut in_string);
    }

    // Save last binding
    if !current_name.is_empty() {
        let expr_str = current_expr.trim_end_matches(',').trim().to_string();
        steps.push(MStep {
            name: current_name,
            expression: parse_expr(&expr_str),
        });
    }

    steps
}

/// Try to split a line into a binding name and expression.
fn try_split_binding(line: &str) -> Option<(String, &str)> {
    // Handle #"Quoted Name" = expr
    if line.starts_with("#\"") {
        if let Some(end_quote) = line[2..].find('"') {
            let name = line[2..2 + end_quote].to_string();
            let rest = line[2 + end_quote + 1..].trim();
            if let Some(expr) = rest.strip_prefix('=') {
                return Some((name, expr.trim()));
            }
        }
        return None;
    }

    // Handle Name = expr
    if let Some(eq_pos) = line.find(" = ") {
        let name = line[..eq_pos].trim();
        // Ensure name is a valid identifier (no operators)
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '#')
        {
            return Some((name.to_string(), line[eq_pos + 3..].trim()));
        }
    }

    None
}

/// Update nesting depth counters for a line.
fn update_depth(
    line: &str,
    paren: &mut i32,
    bracket: &mut i32,
    brace: &mut i32,
    in_string: &mut bool,
) {
    for ch in line.chars() {
        if *in_string {
            if ch == '"' {
                *in_string = false;
            }
            continue;
        }
        match ch {
            '"' => *in_string = true,
            '(' => *paren += 1,
            ')' => *paren -= 1,
            '[' => *bracket += 1,
            ']' => *bracket -= 1,
            '{' => *brace += 1,
            '}' => *brace -= 1,
            _ => {}
        }
    }
}

/// Parse a single M expression string into an `MExpr`.
fn parse_expr(input: &str) -> MExpr {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return MExpr::Raw(String::new());
    }

    // Try to parse as a function call
    if let Some(fc) = try_parse_function_call(trimmed) {
        return MExpr::FunctionCall(fc);
    }

    // String literal
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return MExpr::Literal(LiteralValue::String(
            trimmed[1..trimmed.len() - 1].to_string(),
        ));
    }

    // Number literal
    if let Ok(n) = trimmed.parse::<i64>() {
        return MExpr::Literal(LiteralValue::Integer(n));
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        return MExpr::Literal(LiteralValue::Float(f));
    }

    // Boolean
    if trimmed == "true" {
        return MExpr::Literal(LiteralValue::Boolean(true));
    }
    if trimmed == "false" {
        return MExpr::Literal(LiteralValue::Boolean(false));
    }

    // Null
    if trimmed == "null" {
        return MExpr::Literal(LiteralValue::Null);
    }

    // Date literal #date(y, m, d)
    if trimmed.starts_with("#date(") {
        if let Some(date) = try_parse_date_literal(trimmed) {
            return MExpr::Literal(date);
        }
    }

    // Reference to step name
    if is_identifier(trimmed) {
        return MExpr::Reference(trimmed.to_string());
    }

    // Quoted reference #"Name"
    if trimmed.starts_with("#\"") && trimmed.ends_with('"') {
        let inner = &trimmed[2..trimmed.len() - 1];
        return MExpr::Reference(inner.to_string());
    }

    // Fallback: raw expression
    MExpr::Raw(trimmed.to_string())
}

/// Try to parse a function call like `Table.SelectRows(...)`.
fn try_parse_function_call(input: &str) -> Option<FunctionCall> {
    // Find the function name (dotted identifier followed by `(`)
    let paren_pos = input.find('(')?;
    let func_name = input[..paren_pos].trim();

    // Validate function name
    if func_name.is_empty() || !func_name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_') {
        return None;
    }

    // Find matching close paren
    let args_str = &input[paren_pos + 1..];
    let close_pos = find_matching_paren(args_str)?;
    let args_content = &args_str[..close_pos];

    let arguments = split_args(args_content)
        .into_iter()
        .map(|a| parse_expr(a.trim()))
        .collect();

    Some(FunctionCall {
        function_name: func_name.to_string(),
        arguments,
    })
}

/// Find the matching closing parenthesis.
fn find_matching_paren(input: &str) -> Option<usize> {
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

/// Split function arguments by commas, respecting nesting.
fn split_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
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
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            ',' if depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                args.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < input.len() {
        args.push(&input[start..]);
    }

    args
}

/// Check if a string is a valid M identifier.
fn is_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s.chars().next().unwrap_or(' ');
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// Try to parse a `#date(year, month, day)` literal.
fn try_parse_date_literal(input: &str) -> Option<LiteralValue> {
    let inner = input
        .strip_prefix("#date(")?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() == 3 {
        let y = parts[0].trim().parse::<i32>().ok()?;
        let m = parts[1].trim().parse::<i32>().ok()?;
        let d = parts[2].trim().parse::<i32>().ok()?;
        Some(LiteralValue::Date(y, m, d))
    } else {
        None
    }
}

/// Clean up a step name (remove quotes, whitespace).
fn clean_step_name(name: &str) -> String {
    let name = name.trim();
    if name.starts_with("#\"") && name.ends_with('"') {
        name[2..name.len() - 1].to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_let() {
        let input = r#"
            let
                Source = Sql.Database("server", "db"),
                dbo_Sales = Source{[Schema="dbo",Item="Sales"]}[Data]
            in
                dbo_Sales
        "#;
        let result = parse_m_expression(input);
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].name, "Source");
        assert_eq!(result.result_step, "dbo_Sales");
    }

    #[test]
    fn parse_function_call() {
        let fc = try_parse_function_call("Sql.Database(\"server\", \"db\")");
        assert!(fc.is_some());
        let fc = fc.unwrap();
        assert_eq!(fc.function_name, "Sql.Database");
        assert_eq!(fc.arguments.len(), 2);
    }

    #[test]
    fn parse_date_literal() {
        let result = try_parse_date_literal("#date(2020, 1, 1)");
        assert!(matches!(result, Some(LiteralValue::Date(2020, 1, 1))));
    }

    #[test]
    fn parse_quoted_step_names() {
        let input = r#"
            let
                Source = Sql.Database("server", "db"),
                #"Filtered Rows" = Table.SelectRows(Source, each [Date] > #date(2020, 1, 1)),
                #"Renamed Columns" = Table.RenameColumns(#"Filtered Rows", {{"OrderDate", "order_date"}})
            in
                #"Renamed Columns"
        "#;
        let result = parse_m_expression(input);
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.steps[1].name, "Filtered Rows");
        assert_eq!(result.steps[2].name, "Renamed Columns");
        assert_eq!(result.result_step, "Renamed Columns");
    }
}
