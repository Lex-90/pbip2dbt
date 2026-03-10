//! dbt project file writer module.
//!
//! Generates all output files for the dbt project.

pub mod macros;
pub mod models;
pub mod project;
pub mod report;
pub mod schema;
pub mod sources;

use crate::adapter::SqlAdapter;
use crate::config::Config;
use crate::error::PbipError;
use crate::tmdl::ast::SemanticModel;
use crate::{DaxTranslations, MTranslation};
use log::info;
use std::fs;
use std::path::Path;

/// Write a complete dbt project to disk.
///
/// # Errors
///
/// Returns `PbipError::OutputWrite` on I/O failure.
pub fn write_dbt_project(
    config: &Config,
    model: &SemanticModel,
    _adapter: &dyn SqlAdapter,
    source_name: &str,
    m_translations: &[MTranslation],
    dax_translations: &DaxTranslations,
) -> Result<(), PbipError> {
    let output = &config.output;

    // Create directory structure
    create_dir(output)?;
    create_dir(&output.join("models"))?;
    create_dir(&output.join("models").join("staging"))?;
    create_dir(&output.join("models").join("intermediate"))?;
    create_dir(&output.join("macros"))?;
    create_dir(&output.join("macros").join("dax_helpers"))?;
    create_dir(&output.join("tests"))?;

    // Write dbt_project.yml
    info!("  Writing dbt_project.yml");
    project::write_project_yml(config, output)?;

    // Write packages.yml
    info!("  Writing packages.yml");
    project::write_packages_yml(output)?;

    // Write .gitignore for dbt project
    write_gitignore(output)?;

    // Write sources
    info!("  Writing sources");
    sources::write_sources(config, m_translations, source_name, output)?;

    // Write staging models
    info!("  Writing staging models");
    models::write_staging_models(m_translations, output)?;

    // Write schema (models YAML + tests)
    info!("  Writing schema");
    schema::write_schema(config, model, m_translations, dax_translations, source_name, output)?;

    // Write calculated table models
    if !dax_translations.calc_tables.is_empty() {
        info!("  Writing calculated table models");
        models::write_calc_table_models(&dax_translations.calc_tables, output)?;
    }

    // Write macros
    info!("  Writing macros");
    macros::write_macros(output)?;

    Ok(())
}

fn create_dir(path: &Path) -> Result<(), PbipError> {
    fs::create_dir_all(path).map_err(|e| PbipError::OutputWrite {
        path: path.display().to_string(),
        source: e,
    })
}

fn write_gitignore(output: &Path) -> Result<(), PbipError> {
    let content = r"target/
dbt_packages/
logs/
.user.yml
";
    write_file(&output.join(".gitignore"), content)
}

/// Write a file to disk.
pub fn write_file(path: &Path, content: &str) -> Result<(), PbipError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PbipError::OutputWrite {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    fs::write(path, content).map_err(|e| PbipError::OutputWrite {
        path: path.display().to_string(),
        source: e,
    })
}
