//! `pbip2dbt` — Translate Power BI Desktop Projects (PBIP) into dbt projects.
//!
//! This is the library crate that contains all translation logic.
//! The binary crate (`main.rs`) is a thin CLI wrapper.

pub mod adapter;
pub mod config;
pub mod dax;
pub mod dbt_writer;
pub mod error;
pub mod m_lang;
pub mod naming;
pub mod tmdl;
pub mod zip_reader;

use config::Config;
use error::PbipError;
use log::{info, warn};
use std::collections::BTreeMap;
use tmdl::ast::SemanticModel;

/// Result of a full translation run, used for the translation report.
#[derive(Debug, serde::Serialize)]
pub struct TranslationReport {
    /// Tool version that produced this report.
    pub tool_version: String,
    /// Timestamp of report generation.
    pub generated_at: String,
    /// Input file path.
    pub input_file: String,
    /// Target SQL adapter used.
    pub adapter: String,
    /// dbt project name.
    pub project_name: String,
    /// Summary statistics.
    pub summary: ReportSummary,
    /// Per-table translation details.
    pub tables: Vec<TableReport>,
    /// Per-measure translation details.
    pub measures: Vec<MeasureReport>,
    /// Calculated table translation details.
    pub calculated_tables: Vec<CalcTableReport>,
    /// Calculated column translation details.
    pub calculated_columns: Vec<CalcColumnReport>,
    /// Relationship translation details.
    pub relationships: Vec<RelationshipReport>,
    /// Any errors encountered.
    pub errors: Vec<String>,
}

/// Summary statistics for the translation report.
#[derive(Debug, Default, serde::Serialize)]
pub struct ReportSummary {
    /// Total number of tables in the input.
    pub tables_total: usize,
    /// Number of tables successfully translated.
    pub tables_translated: usize,
    /// Total number of calculated tables.
    pub calculated_tables_total: usize,
    /// Number of calculated tables translated.
    pub calculated_tables_translated: usize,
    /// Total number of calculated columns.
    pub calculated_columns_total: usize,
    /// Number of calculated columns translated.
    pub calculated_columns_translated: usize,
    /// Number of calculated columns needing manual review.
    pub calculated_columns_manual_review: usize,
    /// Total number of measures.
    pub measures_total: usize,
    /// Number of measures translated.
    pub measures_translated: usize,
    /// Number of measures that are documentation-only.
    pub measures_documentation_only: usize,
    /// Average confidence score of translated measures.
    pub measures_avg_confidence: f64,
    /// Total number of relationships.
    pub relationships_total: usize,
    /// Number of dbt tests generated.
    pub tests_generated: usize,
    /// Total `MANUAL_REVIEW` markers in the output.
    pub manual_review_markers: usize,
    /// Number of incremental materialization candidates detected.
    pub incremental_candidates: usize,
}

/// Per-table translation details in the report.
#[derive(Debug, serde::Serialize)]
pub struct TableReport {
    /// Original Power BI table name.
    pub original_name: String,
    /// Generated dbt model name.
    pub dbt_model: String,
    /// M source function detected (e.g., "Sql.Database").
    pub source_type: String,
    /// Total M steps in the expression.
    pub m_steps_total: usize,
    /// Number of M steps translated to SQL.
    pub m_steps_translated: usize,
    /// Number of M steps needing manual review.
    pub m_steps_manual_review: usize,
    /// Details of manual review items.
    pub manual_review_details: Vec<ManualReviewDetail>,
    /// Whether this table is an incremental candidate.
    pub incremental_candidate: bool,
    /// Reason for incremental candidacy.
    pub incremental_reason: Option<String>,
}

/// Detail for a manual review marker.
#[derive(Debug, serde::Serialize)]
pub struct ManualReviewDetail {
    /// The original M/DAX step text.
    pub step: String,
    /// Reason it could not be translated.
    pub reason: String,
    /// Line in the output file where the marker appears.
    pub line_in_output: usize,
}

/// Per-measure translation details in the report.
#[derive(Debug, serde::Serialize)]
pub struct MeasureReport {
    /// Original measure name.
    pub original_name: String,
    /// Original DAX expression.
    pub original_dax: String,
    /// Translated SQL expression.
    pub translated_sql: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Translation warnings.
    pub warnings: Vec<String>,
}

/// Per-calculated-table translation details.
#[derive(Debug, serde::Serialize)]
pub struct CalcTableReport {
    /// Original table name.
    pub original_name: String,
    /// Generated dbt model name.
    pub dbt_model: String,
    /// Original DAX expression.
    pub original_dax: String,
    /// Translated SQL.
    pub translated_sql: String,
    /// Confidence score.
    pub confidence: f64,
}

/// Per-calculated-column translation details.
#[derive(Debug, serde::Serialize)]
pub struct CalcColumnReport {
    /// Table the column belongs to.
    pub table_name: String,
    /// Column name.
    pub column_name: String,
    /// Original DAX expression.
    pub original_dax: String,
    /// Translated SQL expression.
    pub translated_sql: String,
    /// Confidence score.
    pub confidence: f64,
    /// Whether manual review is needed.
    pub manual_review: bool,
}

/// Per-relationship translation details.
#[derive(Debug, serde::Serialize)]
pub struct RelationshipReport {
    /// Relationship name/ID.
    pub name: String,
    /// From table.column.
    pub from: String,
    /// To table.column.
    pub to: String,
    /// Whether the relationship is active.
    pub is_active: bool,
    /// dbt tests generated from this relationship.
    pub tests_generated: Vec<String>,
}

/// Run the full pbip2dbt pipeline.
///
/// # Errors
///
/// Returns `PbipError` for fatal errors (corrupt zip, missing TMDL folder, I/O failures).
pub fn run(config: &Config) -> Result<TranslationReport, PbipError> {
    info!("[1/5] Extracting zip...");
    let files = zip_reader::read_pbip_zip(&config.input)?;

    info!("[2/5] Parsing TMDL...");
    let model = tmdl::parser::parse_semantic_model(&files)?;

    let table_count = model.tables.len();
    let measure_count: usize = model.tables.iter().map(|t| t.measures.len()).sum();
    let relationship_count = model.relationships.len();

    info!(
        "  Found {table_count} tables, {measure_count} measures, {relationship_count} relationships"
    );

    // Build adapter
    let adapter = adapter::adapter_for(&config.adapter)?;

    // Source name: use override or infer from PBIP folder
    let source_name = config
        .source_name
        .clone()
        .unwrap_or_else(|| naming::sanitize_identifier(&model.name.clone().unwrap_or_else(|| "source".to_string())));

    if config.dry_run {
        info!("Dry-run mode: skipping file writes.");
        return Ok(build_report(config, &model, &source_name));
    }

    info!("[3/5] Translating Power Query M → SQL...");
    let m_translations = translate_m_expressions(&model, adapter.as_ref(), &source_name, config);

    info!("[4/5] Translating DAX...");
    let dax_translations = translate_dax(&model, adapter.as_ref(), &source_name, config);

    info!("[5/5] Writing dbt project to {}...", config.output.display());
    dbt_writer::write_dbt_project(
        config,
        &model,
        adapter.as_ref(),
        &source_name,
        &m_translations,
        &dax_translations,
    )?;

    let report = build_report(config, &model, &source_name);

    if !config.dry_run {
        dbt_writer::report::write_report(&config.output, &report)?;
    }

    let manual_review_count = report.summary.manual_review_markers;
    info!(
        "Done. {manual_review_count} MANUAL_REVIEW markers. See translation_report.json for details."
    );

    Ok(report)
}

/// Translated M expression for a table.
#[derive(Debug)]
pub struct MTranslation {
    /// The sanitized table name.
    pub table_name: String,
    /// The original Power BI table name.
    pub original_name: String,
    /// Detected source type (e.g., "Sql.Database").
    pub source_type: String,
    /// Extracted source info for sources.yml.
    pub source_info: Option<m_lang::translator::SourceInfo>,
    /// Generated SQL for the staging model.
    pub staging_sql: String,
    /// Whether this is an incremental candidate.
    pub incremental_candidate: bool,
    /// Incremental reason if applicable.
    pub incremental_reason: Option<String>,
    /// Per-step translation details.
    pub steps_total: usize,
    /// Steps successfully translated.
    pub steps_translated: usize,
    /// Manual review details.
    pub manual_reviews: Vec<ManualReviewDetail>,
    /// Column definitions extracted from the model.
    pub columns: Vec<tmdl::ast::Column>,
    /// Calculated columns translated from DAX.
    pub calc_columns: Vec<CalcColumnTranslation>,
}

/// A translated calculated column.
#[derive(Debug)]
pub struct CalcColumnTranslation {
    /// Column name.
    pub name: String,
    /// Original DAX.
    pub original_dax: String,
    /// Translated SQL expression.
    pub sql_expr: String,
    /// Confidence score.
    pub confidence: f64,
    /// Whether manual review is needed.
    pub manual_review: bool,
}

/// DAX translation results.
#[derive(Debug)]
pub struct DaxTranslations {
    /// Translated measures per table.
    pub measures: BTreeMap<String, Vec<MeasureTranslation>>,
    /// Translated calculated tables.
    pub calc_tables: Vec<CalcTableTranslation>,
}

/// A translated measure.
#[derive(Debug)]
pub struct MeasureTranslation {
    /// Original measure name.
    pub original_name: String,
    /// Sanitized name.
    pub name: String,
    /// Original DAX expression.
    pub original_dax: String,
    /// Translated SQL.
    pub translated_sql: String,
    /// Confidence score.
    pub confidence: f64,
    /// Warnings.
    pub warnings: Vec<String>,
    /// Whether this is documentation-only.
    pub documentation_only: bool,
}

/// A translated calculated table.
#[derive(Debug)]
pub struct CalcTableTranslation {
    /// Original table name.
    pub original_name: String,
    /// Sanitized name.
    pub name: String,
    /// Original DAX expression.
    pub original_dax: String,
    /// Translated SQL.
    pub translated_sql: String,
    /// Confidence score.
    pub confidence: f64,
}

fn translate_m_expressions(
    model: &SemanticModel,
    adapter: &dyn adapter::SqlAdapter,
    source_name: &str,
    config: &Config,
) -> Vec<MTranslation> {
    let mut translations = Vec::new();
    for table in &model.tables {
        // Skip tables without partitions (calculated tables)
        let partition = match &table.partition {
            Some(p) => p,
            None => continue,
        };

        let sanitized_name = naming::sanitize_identifier(&table.name);

        let m_result =
            m_lang::translator::translate_m_expression(&partition.m_expression, adapter, source_name, &sanitized_name);

        // Translate calculated columns if not skipped
        let calc_columns = if config.skip_calculated_columns {
            Vec::new()
        } else {
            table
                .calculated_columns
                .iter()
                .map(|cc| {
                    let result = dax::calc_col_translator::translate_calc_column(&cc.dax_expression, adapter);
                    CalcColumnTranslation {
                        name: naming::sanitize_identifier(&cc.name),
                        original_dax: cc.dax_expression.clone(),
                        sql_expr: result.sql,
                        confidence: result.confidence,
                        manual_review: result.manual_review,
                    }
                })
                .collect()
        };

        translations.push(MTranslation {
            table_name: sanitized_name,
            original_name: table.name.clone(),
            source_type: m_result.source_type.clone(),
            source_info: Some(m_result.source_info),
            staging_sql: m_result.sql,
            incremental_candidate: m_result.incremental_candidate,
            incremental_reason: m_result.incremental_reason.clone(),
            steps_total: m_result.steps_total,
            steps_translated: m_result.steps_translated,
            manual_reviews: m_result
                .manual_reviews
                .into_iter()
                .map(|mr| ManualReviewDetail {
                    step: mr.step,
                    reason: mr.reason,
                    line_in_output: mr.line_in_output,
                })
                .collect(),
            columns: table.columns.clone(),
            calc_columns,
        });
    }
    translations
}

fn translate_dax(
    model: &SemanticModel,
    adapter: &dyn adapter::SqlAdapter,
    source_name: &str,
    config: &Config,
) -> DaxTranslations {
    let mut measures: BTreeMap<String, Vec<MeasureTranslation>> = BTreeMap::new();
    let mut calc_tables = Vec::new();

    if !config.skip_measures {
        for table in &model.tables {
            let table_name = naming::sanitize_identifier(&table.name);
            let mut table_measures = Vec::new();
            for measure in &table.measures {
                if measure.dax_expression.trim().is_empty() {
                    warn!("Skipping measure '{}' with empty DAX expression", measure.name);
                    continue;
                }
                let result = dax::measure_translator::translate_measure(
                    &measure.dax_expression,
                    adapter,
                    config.confidence_threshold,
                );
                let name = naming::sanitize_identifier(&measure.name);
                table_measures.push(MeasureTranslation {
                    original_name: measure.name.clone(),
                    name,
                    original_dax: measure.dax_expression.clone(),
                    translated_sql: result.sql,
                    confidence: result.confidence,
                    warnings: result.warnings.iter().map(|w| w.message.clone()).collect(),
                    documentation_only: result.confidence < config.confidence_threshold
                        || result.confidence == 0.0,
                });
            }
            if !table_measures.is_empty() {
                measures.insert(table_name, table_measures);
            }
        }
    }

    if !config.skip_calculated_tables {
        for table in &model.tables {
            // Calculated tables have no partition but have a DAX expression at the table level
            if table.partition.is_none() {
                if let Some(ref dax_expr) = table.calculated_table_expression {
                    let result =
                        dax::calc_table_translator::translate_calc_table(dax_expr, adapter, source_name);
                    calc_tables.push(CalcTableTranslation {
                        original_name: table.name.clone(),
                        name: naming::sanitize_identifier(&table.name),
                        original_dax: dax_expr.clone(),
                        translated_sql: result.sql,
                        confidence: result.confidence,
                    });
                }
            }
        }
    }

    DaxTranslations {
        measures,
        calc_tables,
    }
}

fn build_report(config: &Config, model: &SemanticModel, _source_name: &str) -> TranslationReport {
    let tables_total = model
        .tables
        .iter()
        .filter(|t| t.partition.is_some())
        .count();
    let calc_tables_total = model
        .tables
        .iter()
        .filter(|t| t.partition.is_none() && t.calculated_table_expression.is_some())
        .count();
    let measures_total: usize = model.tables.iter().map(|t| t.measures.len()).sum();
    let calc_cols_total: usize = model
        .tables
        .iter()
        .map(|t| t.calculated_columns.len())
        .sum();

    let generated_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();

    TranslationReport {
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at,
        input_file: config.input.display().to_string(),
        adapter: config.adapter.clone(),
        project_name: config.project_name.clone(),
        summary: ReportSummary {
            tables_total,
            tables_translated: tables_total,
            calculated_tables_total: calc_tables_total,
            calculated_tables_translated: calc_tables_total,
            calculated_columns_total: calc_cols_total,
            calculated_columns_translated: 0,
            calculated_columns_manual_review: 0,
            measures_total,
            measures_translated: 0,
            measures_documentation_only: 0,
            measures_avg_confidence: 0.0,
            relationships_total: model.relationships.len(),
            tests_generated: 0,
            manual_review_markers: 0,
            incremental_candidates: 0,
        },
        tables: Vec::new(),
        measures: Vec::new(),
        calculated_tables: Vec::new(),
        calculated_columns: Vec::new(),
        relationships: Vec::new(),
        errors: Vec::new(),
    }
}
