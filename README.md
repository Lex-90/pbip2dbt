# pbip2dbt

![License](https://img.shields.io/badge/license-MIT-blue)
![Build](https://img.shields.io/github/actions/workflow/status/owner/pbip2dbt/ci.yml?branch=main)
![Release](https://img.shields.io/github/v/release/owner/pbip2dbt)
![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey)

> Translate Power BI Desktop Projects into production-ready dbt projects — deterministically, offline, in milliseconds.

## Overview

`pbip2dbt` reads a zipped [PBIP](https://learn.microsoft.com/en-us/power-bi/developer/projects/projects-overview) project (TMDL format) and emits a complete dbt project with sources, staging models, translated DAX measures, calculated tables, relationship tests, and a full translation audit report. No cloud connection, no AI inference, no runtime dependencies — just a single static binary that converts Power BI data modeling into dbt SQL.

If you're migrating a Power BI semantic model to a modern analytics stack, `pbip2dbt` gives you an 80% head start instead of rewriting everything from scratch.

## Features

- **Power Query M → dbt staging SQL** — Translates filters, renames, type casts, joins, group-bys, and 20+ M step patterns into adapter-specific SQL. Non-translatable steps get `-- MANUAL_REVIEW:` markers with the original M code preserved.
- **DAX measures → SQL with confidence scores** — Every measure gets a 0.0–1.0 confidence score so you know exactly which translations are safe and which need manual review. Simple aggregations score 1.0; time intelligence and filter context manipulation score lower.
- **DAX calculated tables → dbt models** — `CALENDAR`, `DISTINCT`, `SELECTCOLUMNS`, `UNION`, and other table-generating DAX patterns translate to SQL or `dbt_utils` macro calls.
- **DAX calculated columns → SQL columns** — Row-level DAX expressions are injected directly into the corresponding staging model's SELECT list.
- **Relationship → dbt tests** — Power BI model relationships become `relationships`, `unique`, and `not_null` tests in dbt YAML automatically.
- **Incremental detection** — Date-filtered Power Query sources are flagged as incremental candidates with suggested dbt config blocks.
- **Four SQL dialects** — PostgreSQL/DuckDB, Snowflake, BigQuery, and SQL Server/Synapse via the `--adapter` flag.
- **Fully deterministic** — Same input + same flags = byte-identical output, every time, on every platform.
- **Zero network calls** — Works completely offline. No telemetry, no license checks, no API calls.
- **Single static binary** — Download, `chmod +x`, run. No Python, no Node.js, no Docker.

## Quick Start

### Install

Download the latest release for your platform from the [Releases](https://github.com/owner/pbip2dbt/releases) page:

```bash
# Linux (x86_64)
curl -L https://github.com/owner/pbip2dbt/releases/latest/download/pbip2dbt-x86_64-unknown-linux-musl.tar.gz | tar xz
chmod +x pbip2dbt

# macOS (Apple Silicon)
curl -L https://github.com/owner/pbip2dbt/releases/latest/download/pbip2dbt-aarch64-apple-darwin.tar.gz | tar xz
chmod +x pbip2dbt

# Move to PATH
sudo mv pbip2dbt /usr/local/bin/
```

Or build from source (requires Rust 1.82+):

```bash
git clone https://github.com/owner/pbip2dbt.git
cd pbip2dbt
cargo build --release
# Binary at target/release/pbip2dbt
```

### Run

```bash
pbip2dbt \
  --input my_project.zip \
  --output ./dbt_output \
  --adapter snowflake \
  --project-name my_analytics
```

Expected output:

```text
[1/5] Extracting zip...
[2/5] Parsing TMDL (12 tables, 45 measures, 10 relationships)...
[3/5] Translating Power Query M → SQL...
[4/5] Translating DAX measures → SQL...
[5/5] Writing dbt project to ./dbt_output/...

Done. 12 models, 45 measures (avg confidence: 0.78), 32 tests, 14 MANUAL_REVIEW markers.
See translation_report.json for details.
```

### Verify the output

```bash
cd dbt_output
dbt parse    # Validates syntax without a warehouse connection
```

## How It Works

```text
┌──────────────┐     ┌──────────────┐     ┌──────────────────┐     ┌────────────────┐
│  PBIP Zip    │────►│ TMDL Parser  │────►│ Translation      │────►│ dbt Project    │
│  (input)     │     │              │     │ Engines (×5)     │     │ (output)       │
└──────────────┘     │ Tables       │     │                  │     │                │
                     │ Columns      │     │ M → SQL          │     │ models/        │
                     │ Measures     │     │ DAX Meas → SQL   │     │ sources.yml    │
                     │ Calc Tables  │     │ DAX CalcTbl → SQL│     │ schema.yml     │
                     │ Calc Columns │     │ DAX CalcCol → SQL│     │ macros/        │
                     │ Relationships│     │ Rels → Tests     │     │ report.json    │
                     └──────────────┘     └───────┬──────────┘     └────────────────┘
                                                  │
                                          ┌───────▼──────────┐
                                          │ SQL Adapter      │
                                          │ (postgres │ snow-│
                                          │  flake │ bq │ ts)│
                                          └──────────────────┘
```

The tool reads every `.tmdl` file from the semantic model's `definition/` folder, parses tables, columns, measures, calculated objects, and relationships into an AST, then passes each object through the appropriate translation engine. An adapter trait handles dialect-specific SQL syntax (identifier quoting, date functions, type names). The output is a canonical dbt project that follows [dbt Labs' best practices](https://docs.getdbt.com/best-practices) for naming, folder structure, and testing.

## Input Requirements

The input must be a `.zip` file containing a PBIP project saved in **TMDL format** (not TMSL). The minimal required structure inside the zip:

```text
<ProjectName>.SemanticModel/
├── definition/
│   ├── model.tmdl
│   ├── tables/
│   │   ├── Sales.tmdl
│   │   ├── Customers.tmdl
│   │   └── ...
│   └── relationships.tmdl    (optional)
└── definition.pbism
```

> [!NOTE]
> If your PBIP uses the legacy `model.bim` (TMSL) format, open it in Power BI Desktop, enable **Options → Preview features → Store semantic model using TMDL format**, and re-save. TMDL is Microsoft's recommended format for source control.

## Output Structure

Given a PBIP with source `adventure_works` containing tables `Sales`, `Customers`, `Products`, and a calculated table `Calendar`:

```text
my_analytics/
├── dbt_project.yml
├── packages.yml
├── models/
│   ├── staging/
│   │   └── adventure_works/
│   │       ├── _adventure_works__sources.yml
│   │       ├── _adventure_works__models.yml
│   │       ├── stg_adventure_works__sales.sql
│   │       ├── stg_adventure_works__customers.sql
│   │       └── stg_adventure_works__products.sql
│   ├── intermediate/
│   │   └── adventure_works/
│   │       ├── _int_adventure_works__models.yml
│   │       └── int_adventure_works__calendar.sql
│   └── marts/
│       └── adventure_works/
│           └── _adventure_works__models.yml
├── macros/
│   └── dax_helpers/
│       ├── divide.sql
│       ├── related.sql
│       └── calendar.sql
└── translation_report.json
```

## CLI Reference

```text
pbip2dbt [OPTIONS] --input <PATH> --output <DIR> --adapter <DIALECT> --project-name <NAME>
```

### Required Flags

| Flag | Description |
|------|-------------|
| `--input <PATH>` | Path to the PBIP `.zip` file |
| `--output <DIR>` | Directory for the generated dbt project (created if absent) |
| `--adapter <DIALECT>` | Target SQL dialect: `postgres`, `snowflake`, `bigquery`, `sqlserver` |
| `--project-name <NAME>` | dbt project name (lowercase `snake_case`) |

### Optional Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--source-name <NAME>` | From PBIP folder name | Override the dbt source name |
| `--schema <NAME>` | `raw` | Schema name in `sources.yml` |
| `--materialization-default <TYPE>` | `view` | Default staging materialization (`view` or `table`) |
| `--confidence-threshold <0.0–1.0>` | `0.0` | Minimum confidence score for emitting measure SQL. Below-threshold measures are preserved as documentation only. |
| `--skip-measures` | — | Skip DAX measure translation |
| `--skip-calculated-tables` | — | Skip DAX calculated table translation |
| `--skip-calculated-columns` | — | Skip DAX calculated column translation |
| `--skip-tests` | — | Skip dbt test generation |
| `--verbose` | — | Show detailed per-object translation progress |
| `--dry-run` | — | Parse and translate without writing files; print report to stdout |
| `--no-color` | — | Disable colored terminal output |

### Exit Codes

| Code | Meaning |
|:----:|---------|
| `0` | Success (may include `MANUAL_REVIEW` markers — check `translation_report.json`) |
| `1` | Fatal error (corrupt zip, missing TMDL folder, I/O failure) |
| `2` | Argument error (invalid flags, bad project name) |

## Confidence Scores

Every DAX measure translation includes a confidence score from 0.0 to 1.0 that tells you how reliable the SQL translation is:

| Score | What It Means | Example DAX |
|:-----:|---------------|-------------|
| **1.0** | Direct 1:1 SQL mapping. Safe to use. | `SUM(Sales[Revenue])` |
| **0.8** | `CALCULATE` with simple static filters. Likely correct. | `CALCULATE([Revenue], Region[Name] = "US")` |
| **0.6** | Time intelligence. Correct if date table is properly joined. | `TOTALYTD([Revenue], 'Calendar'[Date])` |
| **0.4** | Dynamic filter context (`ALL`, `ALLEXCEPT`). Best guess. | `CALCULATE([Revenue], ALL(Products))` |
| **0.2** | Iterator functions. Structural translation, may differ semantically. | `SUMX(Orders, [Qty] * [Price])` |
| **0.0** | Untranslatable. Original DAX preserved as documentation. | `CALCULATETABLE(...)`, `USERELATIONSHIP(...)` |

Use `--confidence-threshold 0.8` to only emit SQL for high-confidence translations and preserve everything else as documentation:

```bash
pbip2dbt --input model.zip --output ./out --adapter postgres --project-name proj \
  --confidence-threshold 0.8
```

## Translation Report

Every run produces a `translation_report.json` at the project root with full audit details:

```json
{
  "summary": {
    "tables_total": 12,
    "tables_translated": 12,
    "measures_total": 45,
    "measures_translated": 38,
    "measures_documentation_only": 7,
    "measures_avg_confidence": 0.72,
    "relationships_total": 10,
    "tests_generated": 32,
    "manual_review_markers": 14,
    "incremental_candidates": 3
  },
  "measures": [
    {
      "original_name": "Total Revenue",
      "original_dax": "SUM(Sales[Revenue])",
      "translated_sql": "SUM(revenue)",
      "confidence": 1.0,
      "warnings": []
    }
  ]
}
```

Use this report to plan your manual review effort. Sort by confidence ascending to find the measures that need the most attention.

## Examples

### Basic: single-adapter translation

```bash
pbip2dbt \
  --input contoso_retail.zip \
  --output ./contoso_dbt \
  --adapter snowflake \
  --project-name contoso_analytics
```

### Conservative: only high-confidence translations

```bash
pbip2dbt \
  --input contoso_retail.zip \
  --output ./contoso_dbt \
  --adapter bigquery \
  --project-name contoso_analytics \
  --confidence-threshold 0.8 \
  --skip-calculated-columns
```

### Dry run: preview without writing files

```bash
pbip2dbt \
  --input contoso_retail.zip \
  --output ./contoso_dbt \
  --adapter postgres \
  --project-name contoso_analytics \
  --dry-run | jq '.summary'
```

### CI pipeline: generate for multiple adapters

```bash
for adapter in postgres snowflake bigquery sqlserver; do
  pbip2dbt \
    --input model.zip \
    --output "./dbt_${adapter}" \
    --adapter "$adapter" \
    --project-name "analytics_${adapter}"
done
```

## Supported Translations

### Power Query M → SQL

Filters, renames, type casts, column selection/removal, simple arithmetic columns, joins, group-bys, distinct, top-N, string functions (`Upper`, `Lower`, `Trim`, `Replace`), date part extraction (`Year`, `Month`), and rounding all translate to adapter-specific SQL. Non-translatable patterns (web sources, custom M functions, recursive logic, error handling) produce `-- MANUAL_REVIEW:` comments with the original M code.

### DAX Measures → SQL

40+ DAX functions translate directly. Time intelligence functions (`SAMEPERIODLASTYEAR`, `TOTALYTD`, `DATEADD`) produce window function equivalents. Filter context functions (`CALCULATE`, `ALL`, `ALLEXCEPT`) produce best-effort subquery patterns. Iterator functions (`SUMX`, `AVERAGEX`, `RANKX`) produce correlated subqueries. Untranslatable functions are documented in YAML with the original DAX preserved.

### DAX Calculated Tables → dbt Models

`CALENDAR`/`CALENDARAUTO` → `dbt_utils.date_spine`. `DISTINCT`, `SELECTCOLUMNS`, `ADDCOLUMNS` → SQL equivalents. `UNION` → `UNION ALL`. `DATATABLE` → dbt seed (CSV). `CROSSJOIN` → `CROSS JOIN`.

### Relationships → dbt Tests

Every active relationship produces a `relationships` test on the foreign key column and `unique` + `not_null` tests on the primary key column. Inactive relationships are emitted as commented-out tests with explanatory notes.

## Limitations

- **Power Query M is Turing-complete.** Full M → SQL translation is an undecidable problem. The tool covers the 80% of patterns found in typical enterprise models and flags the rest.
- **DAX filter context has no SQL equivalent.** Measures using `CALCULATE` produce approximations, not exact replicas. The confidence score quantifies this gap.
- **No runtime validation.** The tool cannot verify SQL correctness without a warehouse connection. Always run `dbt compile` and `dbt run` on the output.
- **TMDL format only.** Legacy TMSL (`model.bim`) is not supported. Convert via Power BI Desktop first.
- **Report layer is out of scope.** Visuals, pages, and bookmarks in the `.Report/` folder are ignored.
- **RLS/OLS roles are not translated.** Row-level and object-level security definitions are skipped.

## Building From Source

### Prerequisites

- [Rust](https://rustup.rs/) ≥ 1.82.0

### Build

```bash
git clone https://github.com/owner/pbip2dbt.git
cd pbip2dbt
cargo build --release
```

The binary is at `target/release/pbip2dbt`.

### Static Linux binary (no glibc dependency)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

### Run tests

```bash
cargo test                    # All tests
cargo nextest run             # Parallel (faster)
cargo insta test --review     # Snapshot tests with interactive review
```

### Lint

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo audit
cargo deny check licenses
```

## Project Structure

```text
pbip2dbt/
├── src/
│   ├── main.rs                 # CLI entry point (clap)
│   ├── lib.rs                  # Orchestration pipeline
│   ├── zip_reader.rs           # Zip extraction + PBIP discovery
│   ├── tmdl/                   # TMDL parser (tokenizer → AST)
│   ├── m_lang/                 # Power Query M parser + translator
│   ├── dax/                    # DAX parser + measure/table/column translators
│   ├── adapter/                # SQL dialect trait + 4 implementations
│   ├── dbt_writer/             # dbt project file generation
│   └── naming.rs               # Identifier sanitization
├── tests/
│   ├── fixtures/               # Sample PBIP zips
│   ├── integration/            # End-to-end snapshot tests
│   └── unit/                   # Parser + translator unit tests
├── docs/
│   ├── PRD.md                  # Product requirements
│   └── NFR.md                  # Non-functional requirements
├── CLAUDE.md                   # Claude Code project instructions
└── CHANGELOG.md
```

## Contributing

Contributions are welcome. Before submitting a PR:

1. Run `cargo fmt` and `cargo clippy -- -D warnings` — both must pass with zero issues.
2. Add or update tests for any changed behavior. Integration tests use [insta](https://insta.rs/) for snapshot testing.
3. If you add a new M step or DAX function translation, add a unit test for each supported adapter and update the relevant snapshot.
4. Update `CHANGELOG.md` with your change.
5. Ensure `cargo tarpaulin` coverage doesn't drop below the module thresholds listed in [NFR-7.1](./docs/NFR.md).

See [CLAUDE.md](./CLAUDE.md) for full architectural guidance, module dependency rules, and implementation patterns.

## Acknowledgements

This tool builds on the work of the Power BI, dbt, and Rust communities:

- [Power BI Desktop Projects (PBIP)](https://learn.microsoft.com/en-us/power-bi/developer/projects/projects-overview) — Microsoft's developer-oriented format for Power BI
- [TMDL](https://learn.microsoft.com/en-us/analysis-services/tmdl/tmdl-overview) — Tabular Model Definition Language
- [dbt](https://www.getdbt.com/) — the transformation framework that makes analytics engineering possible
- [dbt best practices](https://docs.getdbt.com/best-practices) — the naming and structural conventions this tool follows

## License

[MIT](./LICENSE)
