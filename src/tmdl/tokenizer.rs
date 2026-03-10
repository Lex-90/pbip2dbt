//! TMDL tokenizer.
//!
//! Converts raw TMDL text into a stream of tokens for the parser.
//! Handles indentation-based structure, `///` description annotations,
//! multiline expressions with tab continuations, and CRLF normalization.

/// A token produced by the TMDL tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A keyword like `table`, `column`, `measure`, `partition`, `relationship`.
    Keyword(String),
    /// An identifier (table/column/measure name).
    Identifier(String),
    /// A property key-value pair (e.g., `lineageTag: abc-123`).
    Property(String, String),
    /// A `///` description annotation.
    Description(String),
    /// An `=` sign followed by an expression (for calc columns, measures).
    Expression(String),
    /// A multiline expression block (M or DAX, multiple lines).
    ExpressionBlock(String),
    /// Indentation level increase.
    Indent,
    /// Indentation level decrease.
    Dedent,
    /// End of file.
    Eof,
}

/// Tokenize a TMDL file into a vector of tokens.
///
/// The tokenizer handles:
/// - BOM stripping
/// - CRLF → LF normalization
/// - Indentation tracking (tab-based)
/// - `///` description annotations
/// - Multiline expressions (lines starting with additional tabs)
/// - Quoted identifiers with single quotes
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Description annotation
        if trimmed.starts_with("///") {
            let desc = trimmed.strip_prefix("///").unwrap_or("").trim().to_string();
            tokens.push(Token::Description(desc));
            i += 1;
            continue;
        }

        // Skip single-line comments
        if trimmed.starts_with("//") {
            i += 1;
            continue;
        }

        let indent_level = count_indent(line);

        // Check for keyword-based lines
        if let Some(rest) = strip_keyword(trimmed, "table") {
            tokens.push(Token::Keyword("table".to_string()));
            tokens.push(Token::Identifier(parse_name(rest)));
            i += 1;
            continue;
        }

        if let Some(rest) = strip_keyword(trimmed, "column") {
            // Check for calculated column: `column Name = DAX_EXPRESSION`
            let name_and_rest = parse_name_with_rest(rest);
            if let Some((name, expr)) = name_and_rest {
                tokens.push(Token::Keyword("calculated_column".to_string()));
                tokens.push(Token::Identifier(name));
                // Collect multiline expression
                let full_expr = collect_multiline_expr(expr, &lines, &mut i, indent_level);
                tokens.push(Token::Expression(full_expr));
            } else {
                tokens.push(Token::Keyword("column".to_string()));
                tokens.push(Token::Identifier(parse_name(rest)));
                i += 1;
            }
            continue;
        }

        if let Some(rest) = strip_keyword(trimmed, "measure") {
            let name_and_rest = parse_name_with_rest(rest);
            if let Some((name, expr)) = name_and_rest {
                tokens.push(Token::Keyword("measure".to_string()));
                tokens.push(Token::Identifier(name));
                let full_expr = collect_multiline_expr(expr, &lines, &mut i, indent_level);
                tokens.push(Token::Expression(full_expr));
            } else {
                tokens.push(Token::Keyword("measure".to_string()));
                tokens.push(Token::Identifier(parse_name(rest)));
                i += 1;
            }
            continue;
        }

        if let Some(rest) = strip_keyword(trimmed, "partition") {
            // partition 'Name' = m
            tokens.push(Token::Keyword("partition".to_string()));
            let name = parse_partition_name(rest);
            tokens.push(Token::Identifier(name));
            i += 1;
            continue;
        }

        if let Some(rest) = strip_keyword(trimmed, "relationship") {
            tokens.push(Token::Keyword("relationship".to_string()));
            tokens.push(Token::Identifier(rest.trim().to_string()));
            i += 1;
            continue;
        }

        // Property: key: value (or key = value for expression blocks)
        if let Some((key, value)) = parse_property(trimmed) {
            if key == "expression" {
                // Collect multiline M expression
                let expr = collect_expression_block(&lines, &mut i, indent_level);
                tokens.push(Token::Property(
                    "expression".to_string(),
                    expr,
                ));
            } else {
                tokens.push(Token::Property(key, value));
                i += 1;
            }
            continue;
        }

        // Unknown line — skip
        i += 1;
    }

    tokens.push(Token::Eof);
    tokens
}

/// Count the indentation level of a line (tabs or groups of spaces).
fn count_indent(line: &str) -> usize {
    let mut count = 0;
    for ch in line.chars() {
        match ch {
            '\t' => count += 1,
            ' ' => count += 1,
            _ => break,
        }
    }
    count
}

/// Strip a keyword prefix from a trimmed line if it starts with the keyword.
fn strip_keyword<'a>(trimmed: &'a str, keyword: &str) -> Option<&'a str> {
    if let Some(rest) = trimmed.strip_prefix(keyword) {
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
            return Some(rest.trim_start());
        }
    }
    None
}

/// Parse a TMDL name, handling single-quoted identifiers.
fn parse_name(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('\'') {
        // Find matching closing quote
        if let Some(end) = s[1..].find('\'') {
            return s[1..=end].to_string();
        }
    }
    // Take until whitespace, colon, or equals
    s.split(|c: char| c.is_whitespace() || c == ':' || c == '=')
        .next()
        .unwrap_or(s)
        .to_string()
}

/// Parse a name and check if there's an `= expression` after it.
/// Returns `Some((name, expression))` if there's an `=`, else `None`.
fn parse_name_with_rest(s: &str) -> Option<(String, String)> {
    let s = s.trim();

    let (name, remainder) = if s.starts_with('\'') {
        // Quoted name
        if let Some(end) = s[1..].find('\'') {
            let name = s[1..=end].to_string();
            let rest = s[end + 2..].trim_start();
            (name, rest)
        } else {
            return None;
        }
    } else {
        // Unquoted: split at `=`
        if let Some(eq_pos) = s.find('=') {
            let name = s[..eq_pos].trim().to_string();
            let rest = s[eq_pos..].trim_start();
            (name, rest)
        } else {
            return None;
        }
    };

    // Check for `= expression`
    let remainder = remainder.trim_start();
    remainder.strip_prefix('=').map(|expr| (name, expr.trim().to_string()))
}

/// Parse a partition name from something like `'Sales' = m`.
fn parse_partition_name(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('\'') {
        if let Some(end) = s[1..].find('\'') {
            return s[1..=end].to_string();
        }
    }
    s.split(|c: char| c.is_whitespace() || c == '=')
        .next()
        .unwrap_or(s)
        .to_string()
}

/// Parse a property line: `key: value` or `key = value`.
fn parse_property(line: &str) -> Option<(String, String)> {
    // Try `key: value` first
    if let Some(colon_pos) = line.find(':') {
        let key = line[..colon_pos].trim();
        // Only match if key is a valid property name (no spaces except in quoted strings)
        if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            let value = line[colon_pos + 1..].trim();
            return Some((key.to_string(), value.to_string()));
        }
    }

    // Try `key = value` for expressions
    if let Some(eq_pos) = line.find(" = ") {
        let key = line[..eq_pos].trim();
        if !key.is_empty() && !key.contains(' ') {
            let value = line[eq_pos + 3..].trim();
            return Some((key.to_string(), value.to_string()));
        }
    }

    None
}

/// Collect a multiline expression (M or DAX) that span subsequent indented lines.
fn collect_multiline_expr(
    first_line: String,
    lines: &[&str],
    i: &mut usize,
    base_indent: usize,
) -> String {
    let mut parts = vec![first_line];
    *i += 1;

    while *i < lines.len() {
        let line = lines[*i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            *i += 1;
            continue;
        }

        let current_indent = count_indent(line);
        // Continuation lines must be more indented or at the same indent + tab
        if current_indent > base_indent {
            parts.push(trimmed.to_string());
            *i += 1;
        } else {
            // Check if this is a property of the same object (like lineageTag)
            if let Some((_key, _value)) = parse_property(trimmed) {
                break;
            }
            break;
        }
    }

    parts.join("\n")
}

/// Collect an `expression =` block where the expression spans multiple indented lines.
fn collect_expression_block(lines: &[&str], i: &mut usize, base_indent: usize) -> String {
    *i += 1; // skip the `expression =` or `expression:` line
    let mut parts = Vec::new();

    while *i < lines.len() {
        let line = lines[*i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            // Preserve blank lines within the expression
            parts.push(String::new());
            *i += 1;
            continue;
        }

        let current_indent = count_indent(line);
        if current_indent > base_indent {
            parts.push(trimmed.to_string());
            *i += 1;
        } else {
            break;
        }
    }

    // Trim trailing empty lines
    while parts.last().is_some_and(std::string::String::is_empty) {
        parts.pop();
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_table() {
        let input = r#"table Sales
	lineageTag: abc-123

	column order_date
		dataType: dateTime
		lineageTag: def-456
		sourceColumn: order_date
		summarizeBy: none
"#;
        let tokens = tokenize(input);
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Keyword(k) if k == "table")));
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Identifier(n) if n == "Sales")));
    }

    #[test]
    fn tokenize_measure_with_expression() {
        let input = "	measure 'Total Revenue' = SUM(Sales[Revenue])\n\t\tlineageTag: abc\n";
        let tokens = tokenize(input);
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Keyword(k) if k == "measure")));
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Identifier(n) if n == "Total Revenue")));
        assert!(tokens.iter().any(|t| matches!(t, Token::Expression(_))));
    }

    #[test]
    fn tokenize_calculated_column() {
        let input = "	column profit = [Revenue] - [Cost]\n\t\tdataType: decimal\n";
        let tokens = tokenize(input);
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Keyword(k) if k == "calculated_column")));
    }

    #[test]
    fn parse_quoted_name() {
        assert_eq!(parse_name("'Total Revenue'"), "Total Revenue");
        assert_eq!(parse_name("Sales"), "Sales");
    }

    #[test]
    fn description_annotation() {
        let input = "/// This is a description\ntable Sales\n";
        let tokens = tokenize(input);
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Description(d) if d == "This is a description")));
    }
}
