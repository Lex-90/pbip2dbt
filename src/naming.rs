//! Identifier sanitization for dbt-compatible names.
//!
//! Converts Power BI table/column/measure names into valid dbt identifiers
//! using Unicode transliteration, reserved word escaping, and formatting rules.

use deunicode::deunicode;
use std::borrow::Cow;

/// SQL reserved words that need escaping when used as dbt identifiers.
const RESERVED_WORDS: &[&str] = &[
    "all", "alter", "and", "as", "asc", "between", "by", "case", "cast", "check", "column",
    "constraint", "create", "cross", "current", "current_date", "current_time",
    "current_timestamp", "database", "default", "delete", "desc", "distinct", "drop", "else",
    "end", "exists", "false", "fetch", "for", "foreign", "from", "full", "grant", "group",
    "having", "if", "in", "index", "inner", "insert", "into", "is", "join", "key", "left",
    "like", "limit", "natural", "not", "null", "offset", "on", "or", "order", "outer", "primary",
    "references", "right", "select", "set", "table", "then", "to", "true", "union", "unique",
    "update", "using", "values", "view", "when", "where", "with",
];

/// Maximum length for dbt identifiers.
const MAX_IDENTIFIER_LENGTH: usize = 63;

/// Sanitize a Power BI name into a valid dbt identifier.
///
/// Rules applied (in order):
/// 1. Unicode transliteration to ASCII via `deunicode`
/// 2. Lowercase
/// 3. Replace spaces and non-alphanumeric characters with underscores
/// 4. Remove leading digits (prefix with underscore)
/// 5. Collapse consecutive underscores
/// 6. Trim trailing underscores
/// 7. Escape SQL reserved words (append underscore)
/// 8. Truncate to 63 characters
/// 9. Empty names become `_unnamed`
///
/// # Examples
///
/// ```
/// use pbip2dbt::naming::sanitize_identifier;
/// assert_eq!(sanitize_identifier("Sales"), "sales");
/// assert_eq!(sanitize_identifier("Fact Sales"), "fact_sales");
/// assert_eq!(sanitize_identifier("order"), "order_");
/// ```
pub fn sanitize_identifier(name: &str) -> String {
    if name.is_empty() {
        return "_unnamed".to_string();
    }

    // Step 1: Unicode transliteration
    let ascii = deunicode(name);

    // Step 2: Lowercase
    let lower = ascii.to_lowercase();

    // Step 3: Replace non-alphanumeric with underscores
    let mut result = String::with_capacity(lower.len());
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(ch);
        } else {
            result.push('_');
        }
    }

    // Step 4: Prefix with underscore if starts with digit
    if result.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        result.insert(0, '_');
    }

    // Step 5: Collapse consecutive underscores
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_underscore = false;
    for ch in result.chars() {
        if ch == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(ch);
            prev_underscore = false;
        }
    }

    // Step 6: Trim trailing underscores
    let trimmed = collapsed.trim_end_matches('_');
    let mut result = if trimmed.is_empty() {
        "_unnamed".to_string()
    } else {
        trimmed.to_string()
    };

    // Step 7: Escape reserved words
    if RESERVED_WORDS.contains(&result.as_str()) {
        result.push('_');
    }

    // Step 8: Truncate to max length
    if result.len() > MAX_IDENTIFIER_LENGTH {
        result.truncate(MAX_IDENTIFIER_LENGTH);
        // Ensure we don't end mid-character (all ASCII at this point)
        result = result.trim_end_matches('_').to_string();
    }

    result
}

/// Sanitize a name but return `Cow::Borrowed` if no changes needed.
///
/// Useful for performance when most names are already valid.
pub fn sanitize_cow(name: &str) -> Cow<'_, str> {
    let sanitized = sanitize_identifier(name);
    if sanitized == name {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(sanitized)
    }
}

/// Deduplicate sanitized names by appending `_2`, `_3`, etc.
///
/// Takes a list of original names, sanitizes them, and resolves collisions.
pub fn deduplicate_names(names: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeMap::new();
    let mut result = Vec::with_capacity(names.len());

    for name in names {
        let sanitized = sanitize_identifier(name);
        let count = seen.entry(sanitized.clone()).or_insert(0usize);
        *count += 1;
        if *count > 1 {
            result.push(format!("{sanitized}_{count}"));
        } else {
            result.push(sanitized);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_sanitization() {
        let cases = [
            ("Sales", "sales"),
            ("Fact Sales", "fact_sales"),
            ("Sales (2024)", "sales_2024"),
            ("2024_Sales", "_2024_sales"),
            ("Sales__Region", "sales_region"),
            ("Sales_", "sales"),
            ("", "_unnamed"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize_identifier(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn reserved_word_escape() {
        assert_eq!(sanitize_identifier("order"), "order_");
        assert_eq!(sanitize_identifier("select"), "select_");
        assert_eq!(sanitize_identifier("table"), "table_");
    }

    #[test]
    fn unicode_transliteration() {
        assert_eq!(sanitize_identifier("Données"), "donnees");
        assert_eq!(sanitize_identifier("Año_Fiscal"), "ano_fiscal");
    }

    #[test]
    fn special_chars_only() {
        assert_eq!(sanitize_identifier("!!!"), "_unnamed");
        assert_eq!(sanitize_identifier("@#$"), "_unnamed");
    }

    #[test]
    fn long_name_truncation() {
        let long_name = "a".repeat(100);
        let result = sanitize_identifier(&long_name);
        assert!(result.len() <= MAX_IDENTIFIER_LENGTH);
    }

    #[test]
    fn deduplication() {
        let names = vec![
            "Sales".to_string(),
            "sales".to_string(),
            "SALES".to_string(),
        ];
        let deduped = deduplicate_names(&names);
        assert_eq!(deduped, vec!["sales", "sales_2", "sales_3"]);
    }
}
