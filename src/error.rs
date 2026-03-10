//! Centralized error types for pbip2dbt.
//!
//! Fatal errors (`PbipError`) produce exit code 1.
//! Argument errors (`ArgError`) produce exit code 2.
//! Non-fatal translation issues are represented as `Warning` structs.

use thiserror::Error;

/// Fatal errors that terminate the pipeline (exit code 1).
#[derive(Debug, Error)]
pub enum PbipError {
    /// Cannot open or read the zip file.
    #[error("[E001] Cannot open zip: {path}: {source}")]
    ZipOpen {
        /// Path to the zip file.
        path: String,
        /// Underlying zip error.
        source: zip::result::ZipError,
    },

    /// Cannot read the zip file as I/O error.
    #[error("[E001] Cannot read zip: {path}: {source}")]
    ZipIo {
        /// Path to the zip file.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// No SemanticModel/definition/ folder found in the zip.
    #[error("[E002] No SemanticModel/definition/ folder found in zip. This PBIP may use TMSL format (model.bim). pbip2dbt requires TMDL. Re-save from Power BI Desktop with TMDL enabled.")]
    NoTmdlFolder,

    /// The zip file has a TMSL model.bim but no TMDL definition/ folder.
    #[error("[E002] This PBIP uses TMSL format (model.bim). pbip2dbt requires TMDL format. Open the project in Power BI Desktop, enable the TMDL preview feature, and re-save.")]
    TmslOnly,

    /// Zip contains a path traversal entry.
    #[error("[E003] Zip contains path traversal entry: {entry}. Aborting for safety.")]
    PathTraversal {
        /// The malicious path entry.
        entry: String,
    },

    /// Zip is password-encrypted.
    #[error("[E004] Zip is password-encrypted. pbip2dbt cannot read encrypted zips.")]
    EncryptedZip,

    /// Cannot write to the output directory.
    #[error("[E005] Cannot write to output directory: {path}: {source}")]
    OutputWrite {
        /// Path to the output directory.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// Empty zip file.
    #[error("[E001] Zip contains no files")]
    EmptyZip,

    /// No TMDL files found in the definition folder.
    #[error("[E002] No .tmdl files found in the definition/ folder")]
    NoTmdlFiles,

    /// TMDL parse error (non-fatal at the table level, but fatal
    /// if all tables fail to parse).
    #[error("[E006] Failed to parse TMDL: {message}")]
    TmdlParse {
        /// Description of the parse error.
        message: String,
    },

    /// Argument validation error (wraps `ArgError` for unified handling).
    #[error("{0}")]
    Arg(#[from] ArgError),
}

/// CLI argument errors (exit code 2).
#[derive(Debug, Error)]
pub enum ArgError {
    /// Invalid adapter name.
    #[error("Invalid adapter '{0}'. Must be one of: postgres, snowflake, bigquery, sqlserver")]
    InvalidAdapter(String),

    /// Invalid project name.
    #[error("Project name '{0}' is not a valid dbt identifier. Use lowercase snake_case, no hyphens.")]
    InvalidProjectName(String),

    /// Invalid confidence threshold.
    #[error("Confidence threshold {0} is out of range. Must be between 0.0 and 1.0.")]
    InvalidConfidenceThreshold(f64),

    /// Invalid materialization.
    #[error("Invalid materialization '{0}'. Must be one of: view, table")]
    InvalidMaterialization(String),
}

/// A non-fatal warning emitted during translation.
#[derive(Debug, Clone)]
pub struct Warning {
    /// Stable warning code (e.g., "W001").
    pub code: &'static str,
    /// Human-readable warning message.
    pub message: String,
    /// Original M/DAX snippet that caused the warning.
    pub source_context: String,
    /// Hint for how to fix the issue.
    pub suggestion: String,
}
