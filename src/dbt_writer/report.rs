//! Generate `translation_report.json`.

use crate::error::PbipError;
use crate::TranslationReport;
use std::path::Path;

/// Write the translation report JSON file.
pub fn write_report(output: &Path, report: &TranslationReport) -> Result<(), PbipError> {
    let json = serde_json::to_string_pretty(report).map_err(|e| PbipError::OutputWrite {
        path: output.display().to_string(),
        source: std::io::Error::other(e.to_string()),
    })?;

    super::write_file(&output.join("translation_report.json"), &json)
}
