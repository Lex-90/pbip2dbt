//! TMDL parser — converts tokenized TMDL files into a `SemanticModel` AST.
//!
//! Handles all TMDL constructs: tables, columns, calculated columns,
//! measures, partitions, relationships. Unknown properties are ignored.

use super::ast::{SemanticModel, Table, Column, DataType, CalculatedColumn, Measure, Partition, ImportMode, Relationship, CrossFilterBehavior};
use super::tokenizer::{tokenize, Token};
use crate::error::PbipError;
use log::{debug, warn};
use std::collections::BTreeMap;

/// Parse all TMDL files into a complete `SemanticModel`.
///
/// Takes a `BTreeMap` of filename → content (from the zip reader) and
/// parses each `.tmdl` file into the appropriate AST constructs.
///
/// # Errors
///
/// Returns `PbipError::TmdlParse` if all tables fail to parse.
/// Individual table parse failures produce warnings but don't abort.
pub fn parse_semantic_model(files: &BTreeMap<String, String>) -> Result<SemanticModel, PbipError> {
    let mut model = SemanticModel::default();

    // Try to infer the model name from model.tmdl or the folder structure
    if let Some(model_content) = files.get("model.tmdl") {
        model.name = extract_model_name(model_content);
    }

    // Parse relationships
    if let Some(rel_content) = files.get("relationships.tmdl") {
        let rels = parse_relationships(rel_content);
        model.relationships = rels;
    }

    // Parse table files
    let mut table_parse_errors = 0;
    let table_file_count = files
        .keys()
        .filter(|k| k.starts_with("tables/") && k.ends_with(".tmdl"))
        .count();

    for (filename, content) in files {
        if filename.starts_with("tables/") && filename.ends_with(".tmdl") {
            match parse_table(content) {
                Ok(table) => {
                    debug!("Parsed table: {}", table.name);
                    model.tables.push(table);
                }
                Err(msg) => {
                    warn!("Failed to parse {filename}: {msg}");
                    table_parse_errors += 1;
                }
            }
        }
    }

    // Also check for relationships within table files
    for (filename, content) in files {
        if filename == "relationships.tmdl" {
            continue; // Already parsed
        }
        // Some TMDL projects put relationships inline
        if !filename.starts_with("tables/") && filename.ends_with(".tmdl") && filename != "model.tmdl" {
            let rels = parse_relationships(content);
            model.relationships.extend(rels);
        }
    }

    if model.tables.is_empty() && table_file_count > 0 {
        return Err(PbipError::TmdlParse {
            message: format!(
                "All {table_parse_errors} table files failed to parse. Check TMDL format."
            ),
        });
    }

    Ok(model)
}

/// Extract the model name from model.tmdl content.
fn extract_model_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("database") {
            let name = rest.trim().trim_matches('\'').trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
        if let Some(rest) = trimmed.strip_prefix("model") {
            let name = rest.trim().to_string();
            if !name.is_empty() && !name.starts_with('{') {
                return Some(name);
            }
        }
    }
    None
}

/// Parse a single table TMDL file into a `Table`.
fn parse_table(content: &str) -> Result<Table, String> {
    let tokens = tokenize(content);
    let mut table = Table {
        name: String::new(),
        description: None,
        lineage_tag: None,
        columns: Vec::new(),
        calculated_columns: Vec::new(),
        measures: Vec::new(),
        partition: None,
        calculated_table_expression: None,
    };

    let mut current_description: Option<String> = None;
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Description(desc) => {
                current_description = Some(desc.clone());
                i += 1;
            }

            Token::Keyword(kw) if kw == "table" => {
                i += 1;
                if let Some(Token::Identifier(name)) = tokens.get(i) {
                    table.name = name.clone();
                    table.description = current_description.take();
                    i += 1;
                } else {
                    return Err("Expected table name after 'table' keyword".to_string());
                }

                // Parse table-level properties
                while i < tokens.len() {
                    match &tokens[i] {
                        Token::Property(key, value) => {
                            match key.as_str() {
                                "lineageTag" => table.lineage_tag = Some(value.clone()),
                                _ => debug!("Ignoring table property: {key}"),
                            }
                            i += 1;
                        }
                        _ => break,
                    }
                }
            }

            Token::Keyword(kw) if kw == "column" => {
                i += 1;
                let desc = current_description.take();
                if let Some(Token::Identifier(name)) = tokens.get(i) {
                    let name = name.clone();
                    i += 1;

                    let mut column = Column {
                        name,
                        data_type: DataType::Unknown,
                        source_column: None,
                        description: desc,
                        lineage_tag: None,
                        summarize_by: None,
                    };

                    // Parse column properties
                    while i < tokens.len() {
                        match &tokens[i] {
                            Token::Property(key, value) => {
                                match key.as_str() {
                                    "dataType" => column.data_type = DataType::from_tmdl(value),
                                    "sourceColumn" => {
                                        column.source_column = Some(value.clone());
                                    }
                                    "lineageTag" => column.lineage_tag = Some(value.clone()),
                                    "summarizeBy" => {
                                        column.summarize_by = Some(value.clone());
                                    }
                                    "description" => column.description = Some(value.clone()),
                                    _ => debug!("Ignoring column property: {key}"),
                                }
                                i += 1;
                            }
                            _ => break,
                        }
                    }

                    table.columns.push(column);
                }
            }

            Token::Keyword(kw) if kw == "calculated_column" => {
                i += 1;
                let _desc = current_description.take();
                if let Some(Token::Identifier(name)) = tokens.get(i) {
                    let name = name.clone();
                    i += 1;

                    let dax_expression = if let Some(Token::Expression(expr)) = tokens.get(i) {
                        i += 1;
                        expr.clone()
                    } else {
                        String::new()
                    };

                    let mut calc_col = CalculatedColumn {
                        name,
                        dax_expression,
                        data_type: DataType::Unknown,
                        lineage_tag: None,
                        is_data_type_inferred: false,
                    };

                    // Parse properties
                    while i < tokens.len() {
                        match &tokens[i] {
                            Token::Property(key, value) => {
                                match key.as_str() {
                                    "dataType" => {
                                        calc_col.data_type = DataType::from_tmdl(value);
                                    }
                                    "lineageTag" => {
                                        calc_col.lineage_tag = Some(value.clone());
                                    }
                                    "isDataTypeInferred" => {
                                        calc_col.is_data_type_inferred =
                                            value.to_lowercase() == "true";
                                    }
                                    _ => debug!("Ignoring calc column property: {key}"),
                                }
                                i += 1;
                            }
                            _ => break,
                        }
                    }

                    table.calculated_columns.push(calc_col);
                }
            }

            Token::Keyword(kw) if kw == "measure" => {
                i += 1;
                let desc = current_description.take();
                if let Some(Token::Identifier(name)) = tokens.get(i) {
                    let name = name.clone();
                    i += 1;

                    let dax_expression = if let Some(Token::Expression(expr)) = tokens.get(i) {
                        i += 1;
                        expr.clone()
                    } else {
                        String::new()
                    };

                    let mut measure = Measure {
                        name,
                        dax_expression,
                        format_string: None,
                        description: desc,
                        display_folder: None,
                        lineage_tag: None,
                    };

                    // Parse properties
                    while i < tokens.len() {
                        match &tokens[i] {
                            Token::Property(key, value) => {
                                match key.as_str() {
                                    "lineageTag" => {
                                        measure.lineage_tag = Some(value.clone());
                                    }
                                    "formatString" => {
                                        measure.format_string = Some(
                                            value.trim_matches('"').to_string(),
                                        );
                                    }
                                    "displayFolder" => {
                                        measure.display_folder = Some(value.clone());
                                    }
                                    "description" => {
                                        measure.description = Some(value.clone());
                                    }
                                    _ => debug!("Ignoring measure property: {key}"),
                                }
                                i += 1;
                            }
                            _ => break,
                        }
                    }

                    table.measures.push(measure);
                }
            }

            Token::Keyword(kw) if kw == "partition" => {
                i += 1;
                current_description.take();
                if let Some(Token::Identifier(name)) = tokens.get(i) {
                    let name = name.clone();
                    i += 1;

                    let mut partition = Partition {
                        name,
                        mode: ImportMode::Default,
                        m_expression: String::new(),
                    };

                    // Parse partition properties
                    while i < tokens.len() {
                        match &tokens[i] {
                            Token::Property(key, value) => {
                                match key.as_str() {
                                    "mode" => {
                                        partition.mode = match value.to_lowercase().as_str() {
                                            "import" => ImportMode::Import,
                                            "directquery" | "directQuery" => {
                                                ImportMode::DirectQuery
                                            }
                                            "dual" => ImportMode::Dual,
                                            _ => ImportMode::Default,
                                        };
                                    }
                                    "expression" => {
                                        partition.m_expression = value.clone();
                                    }
                                    _ => debug!("Ignoring partition property: {key}"),
                                }
                                i += 1;
                            }
                            _ => break,
                        }
                    }

                    table.partition = Some(partition);
                }
            }

            Token::Property(key, value) => {
                // Table-level property found outside of a sub-block
                match key.as_str() {
                    "lineageTag" => table.lineage_tag = Some(value.clone()),
                    _ => debug!("Ignoring top-level property: {key}"),
                }
                i += 1;
            }

            _ => {
                i += 1;
            }
        }
    }

    if table.name.is_empty() {
        return Err("No table name found in TMDL content".to_string());
    }

    Ok(table)
}

/// Parse a relationships TMDL file.
fn parse_relationships(content: &str) -> Vec<Relationship> {
    let mut relationships = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if let Some(rest) = trimmed.strip_prefix("relationship") {
            let name = rest.trim().to_string();
            i += 1;

            let mut rel = Relationship {
                name,
                from_table: String::new(),
                from_column: String::new(),
                to_table: String::new(),
                to_column: String::new(),
                cross_filtering: CrossFilterBehavior::OneDirection,
                is_active: true,
            };

            // Parse relationship properties
            while i < lines.len() {
                let prop_line = lines[i].trim();
                if prop_line.is_empty() || prop_line.starts_with("relationship") {
                    break;
                }

                if let Some(value) = prop_line.strip_prefix("fromColumn:") {
                    let value = value.trim();
                    if let Some((table, col)) = parse_table_column_ref(value) {
                        rel.from_table = table;
                        rel.from_column = col;
                    }
                } else if let Some(value) = prop_line.strip_prefix("toColumn:") {
                    let value = value.trim();
                    if let Some((table, col)) = parse_table_column_ref(value) {
                        rel.to_table = table;
                        rel.to_column = col;
                    }
                } else if let Some(value) = prop_line.strip_prefix("crossFilteringBehavior:") {
                    rel.cross_filtering = match value.trim().to_lowercase().as_str() {
                        "onedirection" | "one" => CrossFilterBehavior::OneDirection,
                        "bothdirections" | "both" => CrossFilterBehavior::BothDirections,
                        _ => CrossFilterBehavior::Automatic,
                    };
                } else if let Some(value) = prop_line.strip_prefix("isActive:") {
                    rel.is_active = value.trim().to_lowercase() != "false";
                }

                i += 1;
            }

            if !rel.from_table.is_empty() && !rel.to_table.is_empty() {
                relationships.push(rel);
            }
        } else {
            i += 1;
        }
    }

    relationships
}

/// Parse a `Table.Column` reference like `Sales.customer_id`.
fn parse_table_column_ref(value: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = value.splitn(2, '.').collect();
    if parts.len() == 2 {
        Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_files(files: Vec<(&str, &str)>) -> BTreeMap<String, String> {
        files
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parse_simple_table() {
        let tmdl = r#"table Sales
	lineageTag: abc-123

	column order_date
		dataType: dateTime
		lineageTag: def-456
		sourceColumn: order_date
		summarizeBy: none

	column customer_id
		dataType: int64
		lineageTag: ghi-789
		sourceColumn: customer_id

	measure 'Total Revenue' = SUM(Sales[Revenue])
		lineageTag: jkl-012
		formatString: "$#,##0.00"

	partition 'Sales' = m
		mode: import
		expression =
			let
				Source = Sql.Database("server", "db"),
				dbo_Sales = Source{[Schema="dbo",Item="Sales"]}[Data]
			in
				dbo_Sales
"#;
        let files = make_files(vec![("tables/Sales.tmdl", tmdl)]);
        let model = parse_semantic_model(&files).unwrap();

        assert_eq!(model.tables.len(), 1);
        let table = &model.tables[0];
        assert_eq!(table.name, "Sales");
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.measures.len(), 1);
        assert!(table.partition.is_some());

        let measure = &table.measures[0];
        assert_eq!(measure.name, "Total Revenue");
        assert!(measure.dax_expression.contains("SUM"));
    }

    #[test]
    fn parse_calculated_column() {
        let tmdl = r#"table Sales
	column profit = [Revenue] - [Cost]
		dataType: decimal
		lineageTag: abc
		isDataTypeInferred: true
"#;
        let files = make_files(vec![("tables/Sales.tmdl", tmdl)]);
        let model = parse_semantic_model(&files).unwrap();

        assert_eq!(model.tables[0].calculated_columns.len(), 1);
        let calc_col = &model.tables[0].calculated_columns[0];
        assert_eq!(calc_col.name, "profit");
        assert!(calc_col.dax_expression.contains("[Revenue]"));
    }

    #[test]
    fn parse_relationships_file() {
        let rels = r#"relationship abc-def-123
	fromColumn: Sales.customer_id
	toColumn: Customers.customer_id
	crossFilteringBehavior: oneDirection

relationship xyz-456
	fromColumn: Sales.product_id
	toColumn: Products.product_id
	crossFilteringBehavior: bothDirections
"#;
        let files = make_files(vec![
            ("tables/Sales.tmdl", "table Sales\n"),
            ("relationships.tmdl", rels),
        ]);
        let model = parse_semantic_model(&files).unwrap();

        assert_eq!(model.relationships.len(), 2);
        assert_eq!(model.relationships[0].from_table, "Sales");
        assert_eq!(model.relationships[0].to_table, "Customers");
        assert_eq!(model.relationships[0].from_column, "customer_id");
        assert_eq!(
            model.relationships[1].cross_filtering,
            CrossFilterBehavior::BothDirections
        );
    }

    #[test]
    fn parse_multiline_measure() {
        let tmdl = r#"table Sales
	measure 'YoY Growth' =
		VAR CurrentYear = [Total Revenue]
		VAR PriorYear = CALCULATE([Total Revenue], SAMEPERIODLASTYEAR('Calendar'[Date]))
		RETURN
			DIVIDE(CurrentYear - PriorYear, PriorYear)
		lineageTag: mno-345
		formatString: "0.00%"
"#;
        let files = make_files(vec![("tables/Sales.tmdl", tmdl)]);
        let model = parse_semantic_model(&files).unwrap();

        assert_eq!(model.tables[0].measures.len(), 1);
        let measure = &model.tables[0].measures[0];
        assert_eq!(measure.name, "YoY Growth");
        assert!(measure.dax_expression.contains("CALCULATE"));
        assert!(measure.dax_expression.contains("SAMEPERIODLASTYEAR"));
    }

    #[test]
    fn unknown_properties_ignored() {
        let tmdl = r#"table Sales
	lineageTag: abc-123
	fakeProperty: "value"
	anotherFake: true

	column id
		dataType: int64
		sourceColumn: id
		unknownProp: something
"#;
        let files = make_files(vec![("tables/Sales.tmdl", tmdl)]);
        let model = parse_semantic_model(&files).unwrap();
        assert_eq!(model.tables.len(), 1);
        assert_eq!(model.tables[0].columns.len(), 1);
    }

    #[test]
    fn description_annotations() {
        let tmdl = r#"/// Sales table description
table Sales
	lineageTag: abc

	/// Order date column
	column order_date
		dataType: dateTime
		sourceColumn: order_date
"#;
        let files = make_files(vec![("tables/Sales.tmdl", tmdl)]);
        let model = parse_semantic_model(&files).unwrap();
        assert_eq!(
            model.tables[0].description.as_deref(),
            Some("Sales table description")
        );
        assert_eq!(
            model.tables[0].columns[0].description.as_deref(),
            Some("Order date column")
        );
    }
}
