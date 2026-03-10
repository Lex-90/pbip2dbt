# pbip2dbt — Implementation Task List

**Status:** All Phases Complete ✅  
**Last Updated:** 2026-03-10

---

## Phase 1: Skeleton + ZIP + TMDL Parser

- [x] 1.1 Initialize Cargo project (`Cargo.toml`, `rust-toolchain.toml`, lints)
- [x] 1.2 `src/main.rs` — CLI entry point with clap derive
- [x] 1.3 `src/lib.rs` — crate root with `run()` orchestrator
- [x] 1.4 `src/config.rs` — `Config` struct, validation
- [x] 1.5 `src/error.rs` — `PbipError`, `ArgError`, `Warning`
- [x] 1.6 `src/naming.rs` — identifier sanitization, reserved words, Unicode
- [x] 1.7 `src/zip_reader.rs` — zip extraction, PBIP discovery, path traversal guard
- [x] 1.8 `src/tmdl/ast.rs` — core AST types
- [x] 1.9 `src/tmdl/tokenizer.rs` — TMDL tokenization
- [x] 1.10 `src/tmdl/parser.rs` — TMDL → AST

## Phase 2: Adapters + M Translator + dbt Writer

- [x] 2.1 `src/adapter/mod.rs` — `SqlAdapter` trait + factory
- [x] 2.2 `src/adapter/postgres.rs`
- [x] 2.3 `src/adapter/snowflake.rs`
- [x] 2.4 `src/adapter/bigquery.rs`
- [x] 2.5 `src/adapter/sqlserver.rs`
- [x] 2.6 `src/m_lang/ast.rs` — M language AST
- [x] 2.7 `src/m_lang/parser.rs` — M let-expression parser
- [x] 2.8 `src/m_lang/translator.rs` — M → SQL translation per adapter
- [x] 2.9 `src/dbt_writer/` — project, sources, models, schema, macros, report

## Phase 3: DAX Measure Translator

- [x] 3.1 `src/dax/ast.rs` — DAX expression AST
- [x] 3.2 `src/dax/parser.rs` — DAX expression parser
- [x] 3.3 `src/dax/measure_translator.rs` — DAX → SQL + confidence scoring
- [x] 3.4 `src/dbt_writer/schema.rs` — `_models.yml` with measures + tests

## Phase 4: Calc Tables + Columns + Relationships + Report

- [x] 4.1 `src/dax/calc_table_translator.rs` — CALENDAR, SELECTCOLUMNS, etc.
- [x] 4.2 `src/dax/calc_col_translator.rs` — calculated column DAX → SQL
- [x] 4.3 Relationship → dbt test generation in `schema.rs`
- [x] 4.4 `src/dbt_writer/macros.rs` — `macros/dax_helpers/`
- [x] 4.5 `src/dbt_writer/report.rs` — `translation_report.json`

## Phase 5: Hardening

- [x] 5.1 Integration tests (10 tests: simple import, measures, relationships, determinism, dry-run, all adapters, error cases, skip flags)
- [x] 5.2 `deny.toml` for license checking
- [x] 5.3 `rust-toolchain.toml` finalized (stable channel)
- [x] 5.4 `.github/workflows/ci.yml` — CI pipeline (test, clippy, fmt, deny, release)
- [x] 5.5 Release build verified: 1.86 MB, all 51 tests pass
