//! CLI entry point for `pbip2dbt`.
//!
//! This binary is a thin wrapper around the library. It parses CLI arguments,
//! sets up logging, and calls [`pbip2dbt::run`].

use clap::Parser;
use pbip2dbt::config::Config;
use std::path::PathBuf;
use std::process::ExitCode;

/// Translate Power BI Desktop Projects (PBIP) into dbt projects.
///
/// Reads a PBIP .zip file containing TMDL-format semantic model definitions
/// and outputs a complete, valid dbt project with sources, staging models,
/// translated measures, and relationship tests.
///
/// Examples:
///   pbip2dbt --input project.zip --output ./out --adapter postgres --project-name my_project
///   pbip2dbt --input project.zip --output ./out --adapter snowflake --project-name my_project --verbose
#[derive(Parser, Debug)]
#[command(name = "pbip2dbt", version, about, long_about = None)]
struct Cli {
    /// Path to the .zip file containing the PBIP project.
    #[arg(long)]
    input: PathBuf,

    /// Directory where the dbt project folder will be written. Created if it does not exist.
    #[arg(long)]
    output: PathBuf,

    /// Target SQL dialect: sqlserver, snowflake, bigquery, postgres.
    #[arg(long)]
    adapter: String,

    /// Value for `name:` in dbt_project.yml. Must be a valid dbt identifier (lowercase, snake_case).
    #[arg(long)]
    project_name: String,

    /// Override the dbt source name used in {{ source() }} references.
    #[arg(long)]
    source_name: Option<String>,

    /// Schema name used in generated sources.yml.
    #[arg(long, default_value = "raw")]
    schema: String,

    /// Default materialization for staging models (view or table).
    #[arg(long, default_value = "view")]
    materialization_default: String,

    /// Skip DAX measure translation entirely.
    #[arg(long, default_value_t = false)]
    skip_measures: bool,

    /// Skip DAX calculated table translation.
    #[arg(long, default_value_t = false)]
    skip_calculated_tables: bool,

    /// Skip DAX calculated column translation.
    #[arg(long, default_value_t = false)]
    skip_calculated_columns: bool,

    /// Skip dbt test generation from relationships and keys.
    #[arg(long, default_value_t = false)]
    skip_tests: bool,

    /// Only emit translated measures with confidence at or above this value.
    #[arg(long, default_value_t = 0.0)]
    confidence_threshold: f64,

    /// Print detailed translation decisions and warnings to stderr.
    #[arg(long, default_value_t = false)]
    verbose: bool,

    /// Parse and translate but do not write any files. Print the translation report to stdout.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Set up logging
    let log_level = if cli.verbose { "info" } else { "warn" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .target(env_logger::Target::Stderr)
        .init();

    let config = Config {
        input: cli.input,
        output: cli.output,
        adapter: cli.adapter,
        project_name: cli.project_name,
        source_name: cli.source_name,
        schema: cli.schema,
        materialization_default: cli.materialization_default,
        skip_measures: cli.skip_measures,
        skip_calculated_tables: cli.skip_calculated_tables,
        skip_calculated_columns: cli.skip_calculated_columns,
        skip_tests: cli.skip_tests,
        confidence_threshold: cli.confidence_threshold,
        verbose: cli.verbose,
        dry_run: cli.dry_run,
    };

    // Validate config
    if let Err(e) = config.validate() {
        eprintln!("Error: {e}");
        return ExitCode::from(2);
    }

    match pbip2dbt::run(&config) {
        Ok(report) => {
            if cli.dry_run {
                // Print report to stdout
                match serde_json::to_string_pretty(&report) {
                    Ok(json) => println!("{json}"),
                    Err(e) => {
                        eprintln!("Error serializing report: {e}");
                        return ExitCode::from(1);
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(1)
        }
    }
}
