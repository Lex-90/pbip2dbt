//! Configuration types and validation for the pbip2dbt pipeline.

use crate::error::ArgError;
use std::path::PathBuf;

/// Full configuration for a pbip2dbt translation run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to the input PBIP .zip file.
    pub input: PathBuf,
    /// Output directory for the generated dbt project.
    pub output: PathBuf,
    /// Target SQL adapter name.
    pub adapter: String,
    /// dbt project name.
    pub project_name: String,
    /// Optional override for the dbt source name.
    pub source_name: Option<String>,
    /// Schema name for generated sources.yml.
    pub schema: String,
    /// Default materialization for staging models.
    pub materialization_default: String,
    /// Skip DAX measure translation.
    pub skip_measures: bool,
    /// Skip DAX calculated table translation.
    pub skip_calculated_tables: bool,
    /// Skip DAX calculated column translation.
    pub skip_calculated_columns: bool,
    /// Skip dbt test generation.
    pub skip_tests: bool,
    /// Minimum confidence threshold for emitting translated measures.
    pub confidence_threshold: f64,
    /// Enable verbose output.
    pub verbose: bool,
    /// Dry-run mode (no file writes).
    pub dry_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: PathBuf::new(),
            adapter: String::new(),
            project_name: String::new(),
            source_name: None,
            schema: "raw".to_string(),
            materialization_default: "view".to_string(),
            skip_measures: false,
            skip_calculated_tables: false,
            skip_calculated_columns: false,
            skip_tests: false,
            confidence_threshold: 0.0,
            verbose: false,
            dry_run: false,
        }
    }
}

impl Config {
    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns `ArgError` if:
    /// - The adapter name is not one of the supported adapters.
    /// - The project name is not a valid dbt identifier.
    /// - The confidence threshold is out of range.
    pub fn validate(&self) -> Result<(), ArgError> {
        let valid_adapters = ["postgres", "snowflake", "bigquery", "sqlserver"];
        if !valid_adapters.contains(&self.adapter.as_str()) {
            return Err(ArgError::InvalidAdapter(self.adapter.clone()));
        }

        if !is_valid_dbt_identifier(&self.project_name) {
            return Err(ArgError::InvalidProjectName(self.project_name.clone()));
        }

        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            return Err(ArgError::InvalidConfidenceThreshold(
                self.confidence_threshold,
            ));
        }

        let valid_materializations = ["view", "table"];
        if !valid_materializations.contains(&self.materialization_default.as_str()) {
            return Err(ArgError::InvalidMaterialization(
                self.materialization_default.clone(),
            ));
        }

        Ok(())
    }
}

/// Check if a string is a valid dbt project identifier.
///
/// Must be lowercase, `snake_case`, no hyphens, start with a letter or underscore.
fn is_valid_dbt_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let first = name.as_bytes()[0];
    if !first.is_ascii_lowercase() && first != b'_' {
        return false;
    }

    name.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_dbt_identifiers() {
        assert!(is_valid_dbt_identifier("my_project"));
        assert!(is_valid_dbt_identifier("_private"));
        assert!(is_valid_dbt_identifier("project_v2"));
    }

    #[test]
    fn invalid_dbt_identifiers() {
        assert!(!is_valid_dbt_identifier(""));
        assert!(!is_valid_dbt_identifier("My-Project"));
        assert!(!is_valid_dbt_identifier("2bad"));
        assert!(!is_valid_dbt_identifier("has spaces"));
        assert!(!is_valid_dbt_identifier("UPPER"));
    }

    #[test]
    fn validate_rejects_bad_adapter() {
        let config = Config {
            adapter: "oracle".to_string(),
            project_name: "my_project".to_string(),
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_good_config() {
        let config = Config {
            adapter: "postgres".to_string(),
            project_name: "my_project".to_string(),
            ..Config::default()
        };
        assert!(config.validate().is_ok());
    }
}
