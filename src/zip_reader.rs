//! ZIP file reader for PBIP projects.
//!
//! Opens a `.zip` file, locates the `*.SemanticModel/definition/` folder,
//! validates the structure, and returns the TMDL file contents.

use crate::error::PbipError;
use log::{debug, warn};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

/// Read a PBIP zip file and extract all TMDL file contents.
///
/// Returns a `BTreeMap` mapping relative file paths (within the `definition/` folder)
/// to their string contents.
///
/// # Errors
///
/// Returns `PbipError` if:
/// - The zip cannot be opened or is corrupt (`E001`)
/// - No `*.SemanticModel/definition/` folder is found (`E002`)
/// - A path traversal entry is detected (`E003`)
/// - The zip is encrypted (`E004`)
pub fn read_pbip_zip(path: &Path) -> Result<BTreeMap<String, String>, PbipError> {
    let file = std::fs::File::open(path).map_err(|e| PbipError::ZipIo {
        path: path.display().to_string(),
        source: e,
    })?;

    let mut archive = zip::ZipArchive::new(file).map_err(|e| PbipError::ZipOpen {
        path: path.display().to_string(),
        source: e,
    })?;

    if archive.is_empty() {
        return Err(PbipError::EmptyZip);
    }

    // Find the SemanticModel/definition/ path prefix
    let definition_prefix = find_definition_prefix(&mut archive, path)?;
    debug!("Found definition prefix: {definition_prefix}");

    let mut files = BTreeMap::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| PbipError::ZipOpen {
            path: path.display().to_string(),
            source: e,
        })?;

        // Normalize path separators
        let entry_name = entry.name().replace('\\', "/");

        // Path traversal guard
        if entry_name.contains("../") || entry_name.starts_with('/') {
            return Err(PbipError::PathTraversal {
                entry: entry_name.clone(),
            });
        }

        // Only process files under the definition prefix
        if !entry_name.starts_with(&definition_prefix) {
            continue;
        }

        // Skip directories
        if entry.is_dir() {
            continue;
        }

        // Read file contents as bytes, then convert
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| PbipError::ZipIo {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut content = String::from_utf8(bytes)
            .unwrap_or_else(|e| {
                warn!("Non-UTF-8 content in {entry_name}: {e}. Replacing invalid bytes.");
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            });

        // Strip BOM
        if content.starts_with('\u{feff}') {
            content = content[3..].to_string();
        }

        // Normalize CRLF to LF
        if content.contains('\r') {
            content = content.replace("\r\n", "\n").replace('\r', "\n");
        }

        // Store with relative path from definition/
        let relative_path = entry_name
            .strip_prefix(&definition_prefix)
            .unwrap_or(&entry_name)
            .to_string();

        if !relative_path.is_empty() {
            debug!("  Read: {relative_path} ({} bytes)", content.len());
            files.insert(relative_path, content);
        }
    }

    if files.is_empty() {
        return Err(PbipError::NoTmdlFiles);
    }

    Ok(files)
}

/// Find the `*.SemanticModel/definition/` prefix in the zip.
fn find_definition_prefix(
    archive: &mut zip::ZipArchive<std::fs::File>,
    zip_path: &Path,
) -> Result<String, PbipError> {
    let mut has_model_bim = false;
    let mut definition_prefix: Option<String> = None;

    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(|e| PbipError::ZipOpen {
            path: zip_path.display().to_string(),
            source: e,
        })?;

        let name = entry.name().replace('\\', "/");

        // Look for *.SemanticModel/definition/
        if let Some(idx) = name.find(".SemanticModel/definition/") {
            let prefix = &name[..idx + ".SemanticModel/definition/".len()];
            if definition_prefix.is_none() {
                definition_prefix = Some(prefix.to_string());
            }
        }

        // Also check just "definition/" at various nesting levels
        if name.ends_with("definition/") && name.contains(".SemanticModel/")
            && definition_prefix.is_none() {
                definition_prefix = Some(name.clone());
            }

        // Check for model.bim (TMSL format)
        if name.ends_with("model.bim") && name.contains(".SemanticModel/") {
            has_model_bim = true;
        }
    }

    match definition_prefix {
        Some(prefix) => Ok(prefix),
        None => {
            if has_model_bim {
                Err(PbipError::TmslOnly)
            } else {
                Err(PbipError::NoTmdlFolder)
            }
        }
    }
}
