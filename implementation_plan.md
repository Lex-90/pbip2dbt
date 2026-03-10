# pbip2dbt — Full Implementation Plan

Implement the complete `pbip2dbt` Rust CLI tool from scratch, following the specifications in `docs/PRD.md`, `docs/NFR.md`, and `CLAUDE.md`. The project is greenfield — only documentation files exist in the repo.

## User Review Required

> [!IMPORTANT]
> This is a **very large implementation** (~50+ source files, ~15,000+ lines of Rust). I'll follow the 5-phase approach from CLAUDE.md, implementing incrementally so each phase produces testable output. Given the scope, I plan to:
> 1. Build **Phase 1** (Skeleton + ZIP + TMDL Parser) and **Phase 2** (Adapters + M Translator + dbt Writer) first — these produce a working tool that can parse PBIP zips and generate basic dbt projects.
> 2. Then **Phase 3** (DAX Measure Translator) and **Phase 4** (Calc Tables + Columns + Relationships + Report) for full feature coverage.
> 3. Finally **Phase 5** (Hardening) for edge cases, CI, and polish.

> [!WARNING]
> **Test fixtures**: I will create synthetic TMDL files zipped as test fixtures. These are minimal but representative of real PBIP output. Real-world PBIP zips from Power BI Desktop may have additional edge cases.

---

## Proposed Changes

### Phase 1: Project Foundation

#### [NEW] [Cargo.toml](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/Cargo.toml)
Rust project manifest with all dependencies from PRD (`clap`, `zip`, `serde`, `serde_json`, `serde_yaml`, `sha2`, `thiserror`, `log`, `env_logger`, `deunicode`) and dev dependencies (`insta`, `tempfile`, `pretty_assertions`). Release profile optimized for size per NFR-11.4. Crate-level lints configured per NFR-6.4.

#### [NEW] [rust-toolchain.toml](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/rust-toolchain.toml)
Pin Rust 1.82.0 per NFR-11.1.

#### [NEW] [src/main.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/main.rs)
Thin CLI entry point (<50 lines). Clap derive for argument parsing. Calls `lib::run()`. Maps errors to exit codes 0/1/2.

#### [NEW] [src/lib.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/lib.rs)
Crate root with `#![forbid(unsafe_code)]`, `#![deny(clippy::unwrap_used, clippy::expect_used)]`. Public `run(config: Config) -> Result<TranslationReport>` function that orchestrates the full pipeline.

#### [NEW] [src/config.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/config.rs)
`Config` struct with all CLI flags. Validation logic (project name must be snake_case, adapter must be valid).

#### [NEW] [src/error.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/error.rs)
`PbipError` (fatal, exit 1) and `ArgError` (exit 2) enums using `thiserror`. Warning struct for non-fatal issues.

#### [NEW] [src/naming.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/naming.rs)
`sanitize_identifier()` with Unicode transliteration via `deunicode`, SQL reserved word list, special char removal, consecutive underscore collapse, length truncation.

#### [NEW] [src/zip_reader.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/zip_reader.rs)
Open zip, find `*.SemanticModel/definition/`, validate structure, path traversal protection (NFR-8.3), return `BTreeMap<String, String>` of file contents.

#### [NEW] [src/tmdl/mod.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/tmdl/mod.rs)
Module re-exports.

#### [NEW] [src/tmdl/ast.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/tmdl/ast.rs)
Core types: `SemanticModel`, `Table`, `Column`, `CalculatedColumn`, `Measure`, `Partition`, `Relationship`, `DataType`, `ImportMode`, `CrossFilterBehavior`, `TranslationResult`, `Warning`.

#### [NEW] [src/tmdl/tokenizer.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/tmdl/tokenizer.rs)
Tokenize TMDL text into typed tokens. Handle CRLF normalization, BOM stripping, indentation-based structure, `///` descriptions, multiline expressions with tab continuations.

#### [NEW] [src/tmdl/parser.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/tmdl/parser.rs)
Parse tokens into `SemanticModel` AST. Handle all TMDL constructs: `table`, `column`, calculated columns (`column Name = expr`), `measure`, `partition...= m`, `relationship`. Gracefully skip unknown properties per NFR-12.1.

---

### Phase 2: Adapters + M Translation + dbt Writer

#### [NEW] [src/adapter/mod.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/adapter/mod.rs)
`SqlAdapter` trait with all methods from CLAUDE.md. `adapter_for()` factory function.

#### [NEW] [src/adapter/postgres.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/adapter/postgres.rs)
#### [NEW] [src/adapter/snowflake.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/adapter/snowflake.rs)
#### [NEW] [src/adapter/bigquery.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/adapter/bigquery.rs)
#### [NEW] [src/adapter/sqlserver.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/adapter/sqlserver.rs)

Four adapter implementations following the dialect table in PRD § "Adapter-Specific SQL Differences".

#### [NEW] [src/m_lang/mod.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/m_lang/mod.rs)
#### [NEW] [src/m_lang/ast.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/m_lang/ast.rs)
M language AST: `LetExpr`, `MStep`, `FunctionCall`, `LiteralValue`, source identification structs.

#### [NEW] [src/m_lang/parser.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/m_lang/parser.rs)
Parse M `let...in` expressions into step AST. Handle `#"Quoted Name"` identifiers, nested function calls, `each` shorthand, `{...}` list/record literals.

#### [NEW] [src/m_lang/translator.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/m_lang/translator.rs)
Translate M steps to SQL fragments. Implement all translatable patterns from PRD Engine 1. Emit `MANUAL_REVIEW` markers for non-translatable patterns.

#### [NEW] [src/dbt_writer/mod.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/mod.rs)
#### [NEW] [src/dbt_writer/project.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/project.rs)
Generate `dbt_project.yml` and `packages.yml`.

#### [NEW] [src/dbt_writer/sources.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/sources.rs)
Generate `_<source>__sources.yml` from parsed M partitions.

#### [NEW] [src/dbt_writer/models.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/models.rs)
Generate staging `.sql` model files with CTE pattern.

---

### Phase 3: DAX Translation

#### [NEW] [src/dax/mod.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/mod.rs)
#### [NEW] [src/dax/ast.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/ast.rs)
DAX expression AST: `DaxExpr`, function calls, table/column references, VAR declarations, measure references.

#### [NEW] [src/dax/parser.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/parser.rs)
Parse DAX expressions into AST. Handle `'Table Name'[Column]` syntax, VAR/RETURN blocks, nested function calls.

#### [NEW] [src/dax/measure_translator.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/measure_translator.rs)
Translate DAX measures to SQL with confidence scoring. Implement all tiers from PRD Engine 2 (1.0 → 0.0).

#### [NEW] [src/dbt_writer/schema.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/schema.rs)
Generate `_models.yml` with column definitions, measure documentation, and relationship tests.

---

### Phase 4: Calc Tables + Columns + Relationships + Report

#### [NEW] [src/dax/calc_table_translator.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/calc_table_translator.rs)
DAX calculated tables → intermediate dbt models (CALENDAR, DISTINCT, SELECTCOLUMNS, etc.).

#### [NEW] [src/dax/calc_col_translator.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dax/calc_col_translator.rs)
DAX calculated columns → SQL expressions in staging models.

#### [NEW] [src/dbt_writer/macros.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/macros.rs)
Generate `macros/dax_helpers/` (divide.sql, calendar.sql, related.sql).

#### [NEW] [src/dbt_writer/report.rs](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/src/dbt_writer/report.rs)
Generate `translation_report.json` with full audit log.

---

### Phase 5: Hardening

#### [NEW] [deny.toml](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/deny.toml)
License allowlist for `cargo deny`.

#### [NEW] [.github/workflows/ci.yml](file:///c:/Users/alexa/Dropbox/Business%20Intelligence/pbip2dbt/pbip2dbt/.github/workflows/ci.yml)
CI pipeline with lint, test, audit, build stages.

#### [NEW] Test fixtures
Synthetic PBIP zip files in `tests/fixtures/` for integration testing.

---

## Verification Plan

### Automated Tests

**Unit tests** (in-file `#[cfg(test)]` modules):
```bash
cargo test --lib
```
- TMDL parser: parse each construct from string fixtures
- M parser/translator: parse M expressions, verify SQL output per adapter
- DAX parser/translator: verify SQL + confidence for each DAX pattern
- Naming: edge cases (empty, special chars, reserved words, Unicode)
- Adapters: each method tested independently

**Integration tests** (in `tests/` directory):
```bash
cargo test --test '*'
```
- End-to-end with test fixture zips → snapshot entire output
- Determinism: run twice, diff output (excluding `generated_at`)

**All tests:**
```bash
cargo test
```

**Linting:**
```bash
cargo fmt --check
cargo clippy -- -D warnings
```

### Manual Verification

After Phase 2 is complete:
1. Run `cargo run -- --input tests/fixtures/simple_import.zip --output ./tmp_test_out --adapter postgres --project-name test_project`
2. Verify the output directory contains valid dbt project structure
3. Inspect generated `stg_*.sql` files for correct CTE pattern and `{{ source() }}` references
4. Inspect `_sources.yml` for correct source definitions
