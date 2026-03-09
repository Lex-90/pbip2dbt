# CLAUDE.md — pbip2dbt

## What This Project Is

`pbip2dbt` is a deterministic, offline Rust CLI that reads a Power BI Desktop Project (PBIP) zip file containing TMDL-format semantic model definitions and outputs a complete, valid dbt project. It translates Power Query M expressions into dbt sources + staging SQL, DAX measures into SQL expressions with confidence scores, DAX calculated tables/columns into dbt models, and Power BI relationships into dbt tests.

Full specifications live in two companion documents — read them before writing any code:

- **`docs/PRD.md`** — Product requirements: CLI interface, translation engines, mapping tables, output structure, adapter differences
- **`docs/NFR.md`** — Non-functional requirements: performance budgets, determinism rules, security constraints, test coverage thresholds, CI pipeline

---

## Quick Commands

```bash
# Build
cargo build
cargo build --release

# Test
cargo test                          # All tests
cargo test --lib                    # Unit tests only
cargo test --test integration       # Integration tests only
cargo nextest run                   # Parallel test runner (preferred)

# Lint (CI mirrors this exactly — if it passes here, it passes there)
cargo fmt --check
cargo clippy -- -D warnings

# Coverage
cargo tarpaulin --out html --output-dir coverage/

# Snapshot tests
cargo insta test                    # Run and check
cargo insta test --review           # Run, then interactively review changes

# Fuzz (nightly only)
cargo +nightly fuzz run tmdl_parser -- -max_total_time=600
cargo +nightly fuzz run m_parser -- -max_total_time=600
cargo +nightly fuzz run dax_parser -- -max_total_time=600

# Dependency audit
cargo audit
cargo deny check licenses

# Generate docs
cargo doc --no-deps --open

# Run the tool
cargo run -- --input tests/fixtures/simple_import.zip --output /tmp/out --adapter postgres --project-name my_project
```

---

## Architecture

```
Input (zip) ──► Zip Reader ──► TMDL Parser ──► AST ──┬──► M Translator ──────┐
                                                      ├──► DAX Measure Trans. │
                                                      ├──► DAX CalcTable Trans│──► dbt Writer ──► Output dir
                                                      ├──► DAX CalcCol Trans. │
                                                      └──► Relationship Gen.──┘
                                                              ▲
                                                              │
                                                      Adapter trait (dialect)
```

All translators are pure functions: `(AST node, &dyn SqlAdapter) → TranslationResult`. No side effects. Only `dbt_writer/` touches the filesystem.

### Module Dependency Rules (ENFORCED)

```
m_lang/     ─╳─►  dax/          (no cross-engine imports)
dax/        ─╳─►  m_lang/       (no cross-engine imports)
m_lang/     ────►  tmdl::ast     (shared types OK)
dax/        ────►  tmdl::ast     (shared types OK)
m_lang/     ────►  adapter/      (dialect queries OK)
dax/        ────►  adapter/      (dialect queries OK)
dbt_writer/ ────►  everything    (sole consumer of translated output)
main.rs     ────►  lib.rs        (thin entry point, <50 lines)
```

No module outside `dbt_writer/` may perform file I/O. No module outside `main.rs` may read CLI args.

---

## Crate Layout

```
src/
├── main.rs                     # CLI (clap derive). <50 lines. Parse args, call lib::run().
├── lib.rs                      # pub fn run(config: Config) → Result<Report>. Orchestrates pipeline.
├── config.rs                   # Config struct, adapter enum, CLI flag validation.
├── error.rs                    # Centralized error types. See Error Handling section.
│
├── zip_reader.rs               # Open zip, find *.SemanticModel/definition/, validate structure.
│
├── tmdl/
│   ├── mod.rs
│   ├── ast.rs                  # Table, Column, CalculatedColumn, Measure, Partition, Relationship
│   ├── tokenizer.rs            # TMDL → token stream
│   └── parser.rs               # Tokens → AST. Must handle BOM, CRLF, unknown properties.
│
├── m_lang/
│   ├── mod.rs
│   ├── ast.rs                  # LetExpr, MStep, FunctionCall, LiteralValue
│   ├── parser.rs               # M expression string → LetExpr with steps
│   └── translator.rs           # MStep → SqlFragment per adapter. See PRD Engine 1.
│
├── dax/
│   ├── mod.rs
│   ├── ast.rs                  # DaxExpr, FunctionCall, VarDecl, MeasureRef
│   ├── parser.rs               # DAX string → DaxExpr tree
│   ├── measure_translator.rs   # DaxExpr → SQL + confidence. See PRD Engine 2.
│   ├── calc_table_translator.rs # Calculated table DAX → SQL. See PRD Engine 3.
│   └── calc_col_translator.rs  # Calculated column DAX → SQL. See PRD Engine 4.
│
├── adapter/
│   ├── mod.rs                  # SqlAdapter trait + adapter_for(name) factory
│   ├── postgres.rs
│   ├── snowflake.rs
│   ├── bigquery.rs
│   └── sqlserver.rs
│
├── dbt_writer/
│   ├── mod.rs
│   ├── project.rs              # dbt_project.yml, packages.yml
│   ├── sources.rs              # _<source>__sources.yml
│   ├── models.rs               # .sql model files (staging, intermediate)
│   ├── schema.rs               # _<source>__models.yml with tests
│   ├── macros.rs               # macros/dax_helpers/*.sql
│   └── report.rs               # translation_report.json
│
└── naming.rs                   # sanitize_identifier(), reserved word list, Unicode transliteration

tests/
├── fixtures/                   # PBIP zip fixtures (committed, read-only)
├── integration/                # End-to-end snapshot tests
└── unit/                       # Parser + translator unit tests
```

---

## Core Types

These are the central types that flow through the entire pipeline. Define them first, stabilize their shape, then build parsers and translators around them.

```rust
// tmdl/ast.rs
pub struct SemanticModel {
    pub tables: Vec<Table>,
    pub relationships: Vec<Relationship>,
}

pub struct Table {
    pub name: String,
    pub description: Option<String>,
    pub lineage_tag: Option<String>,
    pub columns: Vec<Column>,
    pub calculated_columns: Vec<CalculatedColumn>,
    pub measures: Vec<Measure>,
    pub partition: Option<Partition>,     // Power Query M source
}

pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub source_column: Option<String>,
    pub description: Option<String>,
}

pub struct CalculatedColumn {
    pub name: String,
    pub dax_expression: String,
    pub data_type: DataType,
}

pub struct Measure {
    pub name: String,
    pub dax_expression: String,
    pub format_string: Option<String>,
    pub description: Option<String>,
    pub display_folder: Option<String>,
}

pub struct Partition {
    pub name: String,
    pub mode: ImportMode,             // Import, DirectQuery, Dual
    pub m_expression: String,         // Raw M code
}

pub struct Relationship {
    pub name: String,
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
    pub cross_filtering: CrossFilterBehavior,
    pub is_active: bool,
}

// Shared across all translators
pub struct TranslationResult {
    pub sql: String,
    pub confidence: f64,              // 0.0–1.0
    pub warnings: Vec<Warning>,
    pub manual_review: bool,
}

pub struct Warning {
    pub code: &'static str,           // "W001", "W002", ...
    pub message: String,
    pub source_context: String,       // Original M/DAX snippet
    pub suggestion: String,
}
```

---

## Determinism Rules (CRITICAL)

Every violation produces non-reproducible output. Treat these as hard errors.

1. **BTreeMap and BTreeSet everywhere.** `HashMap`/`HashSet` are banned in any code path that produces output. Use `#[cfg_attr(test, deny(...))]` or grep in CI.
2. **No timestamps in generated files.** Only `translation_report.json` has a `generated_at` field. Nothing else.
3. **Fixed float formatting.** Confidence scores: `format!("{:.2}", score)`. Always.
4. **Sorted YAML keys.** When writing YAML manually (not via serde), sort keys alphabetically. When using serde, use `#[serde(serialize_with = "ordered_map")]` for maps.
5. **Stable iteration.** When iterating over tables, columns, measures: preserve the order they appear in the source TMDL files. Parse order = output order.
6. **No environment reads in translation.** `std::env` is only used in `config.rs` for CLI defaults. Translators never touch it.
7. **No system clock in translation.** `std::time` or `chrono` only in `report.rs` for `generated_at`.
8. **LF line endings.** All output uses `\n`. Never `\r\n`. Normalize CRLF during input parsing.

---

## Error Handling

Use `thiserror` for all error types. Accumulate non-fatal errors; short-circuit only on fatal ones.

```rust
// error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PbipError {
    // Fatal — exit code 1
    #[error("[E001] Cannot open zip: {path}: {source}")]
    ZipOpen { path: String, source: zip::result::ZipError },

    #[error("[E002] No SemanticModel/definition/ folder found in zip. This PBIP may use TMSL format (model.bim). pbip2dbt requires TMDL. Re-save from Power BI Desktop with TMDL enabled.")]
    NoTmdlFolder,

    #[error("[E003] Zip contains path traversal entry: {entry}. Aborting for safety.")]
    PathTraversal { entry: String },

    #[error("[E004] Zip is password-encrypted. pbip2dbt cannot read encrypted zips.")]
    EncryptedZip,

    #[error("[E005] Cannot write to output directory: {path}: {source}")]
    OutputWrite { path: String, source: std::io::Error },

    // Non-fatal — collected, reported, translation continues
    // These go into Vec<Warning> on TranslationResult, not here.
}

// CLI argument errors — exit code 2
#[derive(Debug, Error)]
pub enum ArgError {
    #[error("Invalid adapter '{0}'. Must be one of: postgres, snowflake, bigquery, sqlserver")]
    InvalidAdapter(String),

    #[error("Project name '{0}' is not a valid dbt identifier. Use lowercase snake_case, no hyphens.")]
    InvalidProjectName(String),
}
```

Rules:
- Every `PbipError` variant starts with a stable error code (`E001`, `E002`, ...).
- Non-fatal translation issues are `Warning` structs on `TranslationResult`, not `PbipError`.
- Use `.with_context(|| ...)` from `anyhow` at module boundaries in `lib.rs`.
- Never `unwrap()` or `expect()` outside of tests. `#![deny(clippy::unwrap_used, clippy::expect_used)]` is set at crate root.

---

## Adapter Trait

This is the single abstraction for dialect differences. Adding a new adapter = one new file implementing this trait + one match arm in `adapter_for()`.

```rust
// adapter/mod.rs
pub trait SqlAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn quote_identifier(&self, name: &str) -> String;
    fn cast_expr(&self, expr: &str, target_type: &DataType) -> String;
    fn date_trunc(&self, part: &str, expr: &str) -> String;
    fn date_add(&self, expr: &str, interval: i64, part: &str) -> String;
    fn date_diff(&self, part: &str, start: &str, end: &str) -> String;
    fn limit_clause(&self, n: usize) -> String;
    fn concat_op(&self) -> &str;
    fn boolean_literal(&self, val: bool) -> String;
    fn type_name(&self, dt: &DataType) -> String;
    fn iif(&self, cond: &str, true_val: &str, false_val: &str) -> String;
    fn current_date(&self) -> String;
    fn current_timestamp(&self) -> String;
    fn nullif(&self, expr: &str, val: &str) -> String;
    fn string_length(&self, expr: &str) -> String;
    fn left_substr(&self, expr: &str, n: &str) -> String;
    fn right_substr(&self, expr: &str, n: &str) -> String;
}

pub fn adapter_for(name: &str) -> Result<Box<dyn SqlAdapter>, ArgError> {
    match name {
        "postgres" => Ok(Box::new(PostgresAdapter)),
        "snowflake" => Ok(Box::new(SnowflakeAdapter)),
        "bigquery" => Ok(Box::new(BigQueryAdapter)),
        "sqlserver" => Ok(Box::new(SqlServerAdapter)),
        other => Err(ArgError::InvalidAdapter(other.to_string())),
    }
}
```

Consult the adapter dialect table in `docs/PRD.md` § "Adapter-Specific SQL Differences" for the exact return values per method per adapter.

---

## Implementation Order

Build the project in this sequence. Each phase produces testable output before the next begins.

### Phase 1: Skeleton + ZIP + TMDL Parser
1. `cargo init`, set up `Cargo.toml` with all dependencies, configure `[profile.release]`, `[lints]`.
2. `main.rs` with clap derive CLI. `lib.rs` with `pub fn run()` stub.
3. `config.rs` — parse and validate CLI args into `Config` struct.
4. `error.rs` — all error types.
5. `zip_reader.rs` — open zip, find `*.SemanticModel/definition/`, validate structure, return map of `filename → content`.
6. `tmdl/ast.rs` — all AST structs.
7. `tmdl/tokenizer.rs` + `tmdl/parser.rs` — parse TMDL files into `SemanticModel`.
8. `naming.rs` — sanitize identifiers.
9. **Tests:** Unit tests for TMDL parser against inline string fixtures. Test naming edge cases.
10. **Milestone:** `cargo run -- --input fixture.zip --dry-run` prints parsed table/measure/relationship counts.

### Phase 2: Adapters + M Translator + Source/Staging Writer
1. `adapter/mod.rs` + all four adapter implementations. Unit test every method.
2. `m_lang/ast.rs` + `m_lang/parser.rs` — parse M `let` expressions into step AST.
3. `m_lang/translator.rs` — translate each M step pattern. Unit test per pattern per adapter.
4. `dbt_writer/sources.rs` — generate `_sources.yml` from parsed partitions.
5. `dbt_writer/models.rs` — generate staging `.sql` files from M translation.
6. `dbt_writer/project.rs` — generate `dbt_project.yml` and `packages.yml`.
7. **Tests:** Integration test: `simple_import.zip` → snapshot entire output directory.
8. **Milestone:** Tool produces a dbt project with sources and staging models that could pass `dbt parse`.

### Phase 3: DAX Measure Translator
1. `dax/ast.rs` + `dax/parser.rs` — parse DAX expressions into AST.
2. `dax/measure_translator.rs` — implement confidence-scored translation. Start with score-1.0 functions (SUM, COUNT, etc.), then 0.8, 0.6, 0.4, 0.2, 0.0.
3. `dbt_writer/schema.rs` — generate `_models.yml` with measure documentation.
4. **Tests:** Unit test every DAX function in the mapping table. Snapshot test `multi_table.zip` and `complex_dax.zip`.
5. **Milestone:** Measures appear in YAML with translated SQL and confidence scores.

### Phase 4: Calculated Tables + Columns + Relationships + Report
1. `dax/calc_table_translator.rs` — CALENDAR, DISTINCT, SELECTCOLUMNS, etc.
2. `dax/calc_col_translator.rs` — row-level DAX → SQL column expressions.
3. `dbt_writer/schema.rs` — add relationship tests, unique/not_null tests.
4. `dbt_writer/macros.rs` — emit `macros/dax_helpers/` (divide, calendar, related).
5. `dbt_writer/report.rs` — generate `translation_report.json`.
6. **Tests:** Full integration test suite across all fixtures × all adapters. Determinism test (run twice, diff).
7. **Milestone:** Feature-complete. All engines operational.

### Phase 5: Hardening
1. Fuzz targets for all three parsers.
2. Edge case fixtures (malformed TMDL, Unicode names, empty model, TMSL-only, encrypted zip).
3. Coverage check — meet thresholds from NFR-7.1.
4. `cargo deny` configuration (`deny.toml`).
5. CI pipeline (GitHub Actions) with full matrix.
6. Cross-platform build verification.
7. **Milestone:** Production-ready. All NFRs met.

---

## Code Style

### Rust Conventions
- `#![forbid(unsafe_code)]` at crate root.
- `#![deny(clippy::unwrap_used, clippy::expect_used, dead_code)]` at crate root.
- `#![warn(missing_docs, clippy::pedantic)]` at crate root.
- All `pub` items have `///` doc comments with `# Errors` for fallible functions.
- `#[derive(Debug)]` on every type. `#[derive(Clone, PartialEq, Eq)]` where needed for tests.
- Functions accept `&str` not `String` unless ownership is needed.
- Use iterators over index loops. `collect::<Result<Vec<_>, _>>()?` for fallible maps.
- Pre-allocate: `Vec::with_capacity(n)` when size is known.
- Use `Cow<'_, str>` in naming.rs where input might not need transformation.

### SQL Generation Style
- Lowercase SQL keywords: `select`, `from`, `where`, `case when`, `as`.
- One column per line in SELECT clauses.
- Trailing commas in column lists.
- CTE pattern: `with source as (...), renamed as (...) select * from renamed`.
- `snake_case` for all generated identifiers.
- Two-space indentation inside SQL blocks.
- `-- MANUAL_REVIEW:` prefix (exactly this string) for untranslatable constructs. Always followed by the original expression as a quoted comment.
- `-- Auto-generated by pbip2dbt` header in every .sql file, with original table name and M expression hash.

### YAML Generation Style
- Two-space indentation.
- `version: 2` at the top of every schema YAML.
- Keys sorted alphabetically within each level (enforced by serialization).
- Multi-line descriptions use `|` block scalar style.
- Original Power BI names preserved in `description:` fields.

---

## Testing Patterns

### Unit Tests (in-file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_sum() {
        let adapter = PostgresAdapter;
        let dax = parse_dax("SUM(Sales[Revenue])").unwrap();
        let result = translate_measure(&dax, &adapter);
        assert_eq!(result.sql, "SUM(revenue)");
        assert_eq!(result.confidence, 1.0);
        assert!(result.warnings.is_empty());
    }

    // Table-driven for exhaustive coverage
    #[test]
    fn sanitize_names() {
        let cases = [
            ("Sales", "sales"),
            ("Fact Sales", "fact_sales"),
            ("Sales (2024)", "sales_2024"),
            ("2024_Sales", "_2024_sales"),
            ("order", "order_"),  // reserved word
            ("", "_unnamed"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize_identifier(input), expected, "input: {input:?}");
        }
    }
}
```

### Integration Tests (snapshot)

```rust
// tests/integration/end_to_end.rs
use insta::assert_snapshot;
use pbip2dbt::{run, Config};
use tempfile::TempDir;

#[test]
fn simple_import_postgres() {
    let out = TempDir::new().unwrap();
    let config = Config {
        input: "tests/fixtures/simple_import.zip".into(),
        output: out.path().into(),
        adapter: "postgres".into(),
        project_name: "test_project".into(),
        ..Config::default()
    };
    let report = run(config).unwrap();
    
    // Snapshot the full directory tree
    let tree = list_files_recursive(out.path());
    assert_snapshot!("simple_import_postgres_tree", tree);
    
    // Snapshot key files
    let staging_sql = std::fs::read_to_string(
        out.path().join("models/staging/adventure_works/stg_adventure_works__sales.sql")
    ).unwrap();
    assert_snapshot!("simple_import_postgres_sales_sql", staging_sql);
    
    // Snapshot the report summary
    assert_snapshot!("simple_import_postgres_report", serde_json::to_string_pretty(&report.summary).unwrap());
}
```

### Determinism Test

```rust
#[test]
fn output_is_deterministic() {
    let out1 = TempDir::new().unwrap();
    let out2 = TempDir::new().unwrap();
    let config = |p: &Path| Config {
        input: "tests/fixtures/multi_table.zip".into(),
        output: p.into(),
        adapter: "snowflake".into(),
        project_name: "det_test".into(),
        ..Config::default()
    };
    run(config(out1.path())).unwrap();
    run(config(out2.path())).unwrap();
    
    assert_dirs_identical(out1.path(), out2.path(), &["generated_at"]);
}
```

---

## Security Checklist

Before every commit, verify:

- [ ] No `std::process::Command` anywhere in the codebase
- [ ] No `std::net`, `tokio::net`, or any network crate in `Cargo.toml`
- [ ] `zip_reader.rs` rejects path traversal (`../`, absolute paths)
- [ ] All writes go to the `--output` directory only (resolved to absolute, checked)
- [ ] No `unwrap()`/`expect()` outside `#[cfg(test)]` blocks
- [ ] `cargo audit` clean
- [ ] `cargo deny check licenses` clean

---

## Confidence Score Quick Reference

When implementing `dax/measure_translator.rs`, score is the **minimum** across all constructs:

| Score | Constructs |
|:-----:|------------|
| 1.0 | `SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`, `COUNTA`, `COUNTROWS`, `DISTINCTCOUNT`, `DIVIDE`, `IF`, `SWITCH`, `BLANK`, `ISBLANK`, `CONCATENATE`, `LEFT`, `RIGHT`, `LEN`, `UPPER`, `LOWER`, `TRIM`, `YEAR`, `MONTH`, `DAY`, `TODAY`, `NOW`, `ROUND`, `ABS`, `INT`, `TRUE`, `FALSE`, `AND`, `OR`, `NOT`, `FORMAT`, `IN`, `CONTAINSSTRING` |
| 0.8 | `CALCULATE` with static scalar filters |
| 0.6 | `SAMEPERIODLASTYEAR`, `DATEADD`, `TOTALYTD`, `DATESYTD` (time intelligence) |
| 0.4 | `CALCULATE` + `ALL`/`ALLEXCEPT`/`REMOVEFILTERS`/`KEEPFILTERS` |
| 0.2 | `SUMX`, `AVERAGEX`, `MAXX`, `COUNTROWS(FILTER(...))`, `RANKX`, `TOPN` |
| 0.0 | `CALCULATETABLE`, `SUMMARIZECOLUMNS` (in measure dependency), `PATH*`, `USERELATIONSHIP`, `CROSSFILTER`, `DETAILROWS`, `SELECTEDVALUE`, `HASONEVALUE`, `ISFILTERED`, `ISCROSSFILTERED`, `TREATAS` |

Score-0.0 measures get documentation-only output (original DAX in YAML description, no SQL emitted).

---

## M Step Translation Quick Reference

When implementing `m_lang/translator.rs`, these M patterns have deterministic SQL mappings:

**Always translatable:** `Table.SelectRows` (→ WHERE), `Table.RenameColumns` (→ aliases), `Table.RemoveColumns` (→ omit), `Table.SelectColumns` (→ SELECT list), `Table.TransformColumnTypes` (→ CAST), `Table.AddColumn` simple arithmetic (→ expression alias), `Table.Group` standard aggs (→ GROUP BY), `Table.NestedJoin` (→ JOIN), `Table.ExpandTableColumn` (→ join column select), `Table.Distinct` (→ DISTINCT), `Table.FirstN` (→ LIMIT/TOP), `Table.ReplaceValue` (→ REPLACE), `Text.Upper/Lower/Trim`, `Date.Year/Month`, `Number.Round`, `Table.Sort` (→ ignored, no ORDER BY in dbt models).

**Never translatable (emit MANUAL_REVIEW):** `Web.Contents`, `Function.Invoke`, `@` recursion, `Table.Buffer`, `List.Generate`, `List.Accumulate`, parameter references, `try/otherwise`, complex `each` lambdas with nested calls, `Record.Field`/`Record.FieldOrDefault`.

See PRD § "Engine 1" for the complete mapping table with adapter-specific SQL.

---

## File Header Template

Every generated `.sql` file starts with:

```sql
-- Auto-generated by pbip2dbt v{{ version }}
-- Source: {{ original_table_name }} ({{ source_type }})
-- M expression hash: sha256:{{ hash }}
-- Translation confidence: {{ confidence }}
-- Adapter: {{ adapter_name }}
-- DO NOT EDIT — regenerate from PBIP source
```

Every generated `.yml` file starts with:

```yaml
# Auto-generated by pbip2dbt v{{ version }}
# DO NOT EDIT — regenerate from PBIP source
```

---

## Common Pitfalls

1. **HashMap iteration order.** Rust's `HashMap` has random iteration order. If you iterate a `HashMap` to generate output (file lists, YAML keys, column orders), the output will differ between runs. Use `BTreeMap` or sort before iterating. This is the #1 determinism bug.

2. **TMDL multiline expressions.** M expressions and DAX formulas in TMDL span multiple lines with tab-based continuation. The parser must handle this — don't split on newlines naively. A line starting with a tab (or multiple tabs) is a continuation of the previous property value.

3. **TMDL `=` in calculated columns.** A regular column has `sourceColumn: name`. A calculated column has `column Name = <DAX expression>` on the same line as the `column` keyword. The parser must distinguish these by the presence of `=` after the column name.

4. **M expression quoting.** Power Query M uses `#"Quoted Name"` for identifiers with spaces. The M parser must handle this — don't treat `#` as a comment character.

5. **DAX table references.** DAX uses `'Table Name'[Column]` syntax with single quotes around table names that contain spaces. The DAX parser must extract table and column names correctly from this syntax.

6. **Adapter quoting in generated SQL.** Don't hardcode `"` for identifier quoting — BigQuery uses backticks, SQL Server uses brackets. Always call `adapter.quote_identifier()`.

7. **Windows path separators in zips.** Zips created on Windows may use `\` as path separator. Normalize to `/` in `zip_reader.rs`.

8. **UTF-8 BOM.** TMDL files from Power BI Desktop on Windows may start with `\xEF\xBB\xBF`. Strip this in the parser before tokenizing.

9. **Empty partitions.** Some tables in a PBIP have no `partition` block (e.g., calculated tables). Don't error — just skip source/staging generation for these tables and route them through the DAX calculated table translator instead.

10. **Measure references in measures.** A DAX measure like `[YoY Growth]` can reference another measure `[Total Revenue]`. The translator must resolve these references to inline the referenced measure's SQL, or emit a `-- MANUAL_REVIEW:` if the reference chain is too deep or circular.

---

## Definition of Done

A feature is complete when:
1. Implementation exists with full doc comments.
2. Unit tests cover happy path + at least 2 error paths.
3. Integration snapshot tests updated and reviewed.
4. `cargo clippy -- -D warnings` passes.
5. `cargo fmt --check` passes.
6. Coverage for the changed module meets its threshold (see NFR-7.1).
7. No new `unwrap()`/`expect()` outside tests.
