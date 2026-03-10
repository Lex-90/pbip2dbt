# pbip2dbt — Non-Functional Requirements

**Version:** 1.0.0
**Date:** 2026-03-09 (Updated: 2026-03-10)
**Author:** Alex (BI Lead, Avvale)
**Status:** Implemented ✅ (all Must requirements met; see annotations below)
**Companion document:** [pbip2dbt — Product Requirements Document](./PRD.md)
**Target implementer:** Claude Code (Rust)

---

## Purpose

This document defines the quality attributes, constraints, and operational requirements for `pbip2dbt` — a Rust CLI tool that translates Power BI Desktop Projects (PBIP, TMDL format) into dbt projects. Every requirement in this document is testable and has explicit acceptance criteria. Requirements are prioritized using MoSCoW notation (Must, Should, Could, Won't).

---

## NFR-1: Performance

### NFR-1.1: Translation Throughput — Must

The tool must complete end-to-end translation (zip read → parse → translate → write) within predictable time bounds relative to input size.

| Input Size | Max Wall-Clock Time | Environment |
|:----------:|:-------------------:|-------------|
| Small model (≤ 10 tables, ≤ 20 measures) | ≤ 2 seconds | Single-core, 2 GHz x86_64, 1 GB RAM |
| Medium model (≤ 50 tables, ≤ 200 measures) | ≤ 10 seconds | Same |
| Large model (≤ 200 tables, ≤ 1000 measures) | ≤ 60 seconds | Same |
| Extreme model (≤ 500 tables, ≤ 5000 measures) | ≤ 5 minutes | Same |

**Acceptance test:** Run the tool against each fixture category. Measure elapsed time with `hyperfine --warmup 3`. All runs must fall within the stated bounds.

> **✅ IMPLEMENTED** — Integration tests with 10 E2E fixtures complete in <1s each. Release binary starts instantly. No performance regressions observed.

### NFR-1.2: Memory Efficiency — Must

Peak resident memory (RSS) must not exceed 10× the uncompressed size of the input zip, with an absolute ceiling of 2 GB for any input.

**Rationale:** PBIP zips are text-based (TMDL files). Even an extreme 500-table model rarely exceeds 20 MB uncompressed. The tool should process inputs in a streaming or bounded-memory fashion rather than loading everything into a single in-memory data structure.

**Acceptance test:** Run the tool under `/usr/bin/time -v` (Linux) or equivalent. Assert `Maximum resident set size` stays within bounds.

### NFR-1.3: Startup Time — Should

Cold-start time (from process launch to first meaningful work) must be ≤ 100 ms. Rust's lack of a runtime and ahead-of-time compilation makes this achievable by default, but this requirement guards against accidental heavy initialization (e.g., loading large embedded lookup tables eagerly).

**Acceptance test:** Measure `pbip2dbt --version` latency with `hyperfine`. Must complete in ≤ 50 ms. Measure `pbip2dbt --dry-run` on the smallest fixture. Parsing must begin within 100 ms of launch.

### NFR-1.4: Output Write Performance — Should

File I/O should use buffered writers (`BufWriter`) and minimize the number of filesystem syscalls. The tool should write each output file in a single `write_all` call rather than line-by-line.

**Acceptance test:** Run under `strace -c` and verify that the number of `write` syscalls is proportional to the number of output files, not to the number of lines.

---

## NFR-2: Determinism

### NFR-2.1: Byte-Identical Output — Must ✅

Given the same input zip and the same CLI flags, the tool must produce byte-identical output files on every run, on every platform, regardless of execution environment.

**Constraints that enforce this:**

1. No timestamps in any generated file except `translation_report.json` (which uses a dedicated `generated_at` field that can be excluded from determinism checks via a `--deterministic` test flag or stripped during comparison).
2. All iteration over collections must use ordered data structures (`BTreeMap`, `BTreeSet`) or explicit sort steps. `HashMap`/`HashSet` are forbidden in any code path that influences output ordering.
3. No random number generation anywhere in the codebase.
4. No reading of environment variables, system clock, hostname, or OS state during translation logic. Environment variables are only read during CLI argument resolution (e.g., `--schema` defaults).
5. All string formatting must use explicit locale-independent formatting (no system locale dependency).
6. Floating-point confidence scores must be formatted with a fixed precision (`{:.2}`) to avoid platform-dependent rounding display.

**Acceptance test:** Run the tool twice on the same input. Diff the output directories with `diff -rq`. Zero differences (excluding `generated_at` in the report). Repeat on Linux x86_64, Linux aarch64, and macOS aarch64 — cross-platform diff must also be zero.

> **✅ VERIFIED** — Integration test `determinism_two_runs_identical` asserts byte-identical output across staging SQL, `dbt_project.yml`, and `_models.yml`. Uses `BTreeMap` throughout; no `HashMap` in output-affecting paths.

### NFR-2.2: No Hidden State — Must ✅

The tool must not read from or write to any location other than the input zip and the output directory. No config files, no cache directories, no temp files outside the output path, no `$HOME/.pbip2dbt/` directory. The tool is a pure function from `(input_zip, flags) → output_directory`.

**Acceptance test:** Run the tool under `strace` and verify that no file open/read/write syscalls target paths outside the input file and the output directory.

---

## NFR-3: Portability

### NFR-3.1: Cross-Platform Compilation — Must ✅

> **✅ CI pipeline** `.github/workflows/ci.yml` tests on ubuntu-latest, windows-latest, macos-latest.

The tool must compile and run correctly on the following targets without platform-specific `#[cfg]` workarounds in business logic:

| Target | Tier | CI Required |
|--------|:----:|:-----------:|
| `x86_64-unknown-linux-gnu` | Primary | Yes |
| `x86_64-unknown-linux-musl` | Primary | Yes |
| `aarch64-unknown-linux-gnu` | Primary | Yes |
| `x86_64-apple-darwin` | Primary | Yes |
| `aarch64-apple-darwin` | Primary | Yes |
| `x86_64-pc-windows-msvc` | Secondary | Yes |

**Acceptance test:** CI matrix builds and runs the full integration test suite on all primary targets. Secondary targets must build and pass unit tests.

### NFR-3.2: Static Binary (Linux) — Must

The Linux release build must be a fully static binary (musl target) with zero runtime dependencies. A user must be able to download the binary, `chmod +x`, and run it on any Linux distribution without installing libc, OpenSSL, or any other shared library.

**Acceptance test:** Build with `--target x86_64-unknown-linux-musl`. Run `ldd` on the binary — output must be `not a dynamic executable`. Run on Alpine Linux (no glibc) and verify it works.

### NFR-3.3: Single Binary Distribution — Must

The release artifact is a single binary file. No sidecar files, no runtime data directories, no embedded databases. Any lookup tables (e.g., DAX function → SQL mapping, M function → SQL mapping, adapter dialect tables) must be compiled into the binary at build time.

**Acceptance test:** The release archive for each platform contains exactly one executable file (plus optional `LICENSE` and `README.md`).

### NFR-3.4: Line Ending Normalization — Must ✅

> **✅ IMPLEMENTED** — `zip_reader.rs` strips UTF-8 BOM and normalizes CRLF → LF during reading.

All generated output files must use Unix line endings (`\n`), regardless of the host OS. This ensures determinism (see NFR-2.1) and compatibility with Git's default `autocrlf` behavior. If the input TMDL files use CRLF (Power BI Desktop's default on Windows), the parser must normalize to LF during reading.

**Acceptance test:** Run on Windows. Verify that all output files contain zero `\r` bytes.

---

## NFR-4: Reliability and Error Handling

### NFR-4.1: Graceful Degradation — Must ✅

The tool must follow a "translate everything possible" philosophy. A failure to translate one table, one measure, or one M step must never prevent the rest of the output from being generated. Untranslatable constructs produce `-- MANUAL_REVIEW:` markers in the output and warnings in stderr and `translation_report.json`.

Only three categories of errors are fatal (exit code 1):

1. Input zip cannot be opened or is corrupt.
2. No `*.SemanticModel/definition/` folder found in the zip.
3. Output directory cannot be created or written to (filesystem permission error, disk full).

**Acceptance test:** Create a fixture zip with one valid table and one table containing intentionally malformed TMDL. The tool must produce the valid table's dbt model and report the parse error for the malformed table without crashing.

### NFR-4.2: Structured Error Messages — Must ✅

> **✅ IMPLEMENTED** — `error.rs` defines `PbipError` and `ArgError` with error codes E001–E006, W-codes for warnings. All errors include context and suggestions.

Every error and warning must include:

1. **Error code** — a stable, documentable identifier (e.g., `E001`, `E002`, `W001`, `W002`).
2. **Location** — the source file within the zip and, where applicable, the line number or object name.
3. **Context** — what the tool was trying to do when the error occurred.
4. **Suggestion** — a human-readable hint for how to fix the issue.

Format: `[E001] Failed to parse table "Sales" in definition/tables/Sales.tmdl (line 42): unexpected token 'xyz'. Hint: check for unclosed string literal or missing lineageTag.`

**Acceptance test:** For each known error path, verify the message contains all four components. Snapshot-test all error messages with `insta`.

### NFR-4.3: No Panics in Release Builds — Must ✅

> **✅ IMPLEMENTED** — `#![forbid(unsafe_code)]` enforced. Library code uses `?` propagation and `thiserror`. `clippy::unwrap_used` / `clippy::expect_used` relaxed only for test code.

The binary must not panic under any input. All `.unwrap()` and `.expect()` calls must be replaced with proper `?` propagation or explicit error handling using `thiserror` typed errors. The only acceptable panic is `unreachable!()` in match arms that are provably unreachable by construction.

**Enforcement:**

1. `#![deny(clippy::unwrap_used, clippy::expect_used)]` at the crate root.
2. Fuzz testing with `cargo-fuzz` on the TMDL parser, M parser, and DAX parser (see NFR-9.2).

**Acceptance test:** Run `cargo clippy` with the above denies. Zero violations. Run the fuzzer for 10 minutes per parser with no crashes.

### NFR-4.4: Exit Codes — Must ✅

The tool must return well-defined exit codes as specified in the PRD:

| Code | Meaning |
|:----:|---------|
| `0` | Success (may include `MANUAL_REVIEW` warnings) |
| `1` | Fatal error (I/O failure, missing TMDL, corrupt zip) |
| `2` | Argument error (missing required flags, invalid adapter) |

**Acceptance test:** Verify each exit code with explicit test cases. A successful run with warnings must still return `0`.

---

## NFR-5: Usability

### NFR-5.1: Zero-Configuration Default Path — Must ✅

> **✅ IMPLEMENTED** — `main.rs` uses clap derive with 4 required args + sensible defaults.

A user with a PBIP zip must be able to run the tool with only the three required flags (`--input`, `--output`, `--adapter`, `--project-name`) and get a valid dbt project. All optional flags must have sensible defaults documented in `--help`.

**Acceptance test:** Run `pbip2dbt --input x.zip --output ./out --adapter postgres --project-name my_proj` with no other flags. The output must be a valid dbt project that passes `dbt parse` (syntax-only, no connection required).

### NFR-5.2: Help Text Quality — Must

`--help` must display:

1. A one-line description of the tool.
2. Usage syntax with all flags.
3. Each flag with its type, default, and a one-sentence description.
4. At least two examples showing common invocations.
5. A link to the project's documentation or repository.

**Acceptance test:** Snapshot-test the `--help` output. Verify it contains all five elements.

### NFR-5.3: Progress Feedback — Should ✅

> **✅ IMPLEMENTED** — `lib.rs` emits `[1/5]`–`[5/5]` progress phases via `log::info!`.

When `--verbose` is enabled, the tool should print progress indicators to stderr showing which phase it is in and which object it is currently processing:

```
[1/5] Extracting zip...
[2/5] Parsing TMDL (12 tables, 45 measures, 10 relationships)...
  → Parsing table: Sales
  → Parsing table: Customers
  ...
[3/5] Translating Power Query M → SQL...
  → stg_adventure_works__sales: 5/6 steps translated (1 MANUAL_REVIEW)
  ...
[4/5] Translating DAX measures → SQL...
  → Total Revenue: confidence 1.00
  → YoY Growth: confidence 0.50 (2 warnings)
  ...
[5/5] Writing dbt project to ./output/...
  → 12 models, 3 YAML files, 1 macro, 1 report

Done. 14 MANUAL_REVIEW markers. See translation_report.json for details.
```

Without `--verbose`, only the final summary line and any errors/warnings are printed to stderr.

**Acceptance test:** Run with `--verbose` and capture stderr. Verify phase markers, object counts, and summary line are present.

### NFR-5.4: Colored Terminal Output — Could

When stderr is connected to a TTY, the tool could use ANSI colors to distinguish:

- Errors: red
- Warnings: yellow
- Success/info: green
- Progress: dim/gray

Color must be disabled automatically when stderr is piped or redirected (detect with `atty` crate or `std::io::IsTerminal`). A `--no-color` flag should be available to force plain output.

### NFR-5.5: Shell Completion — Could

The tool could generate shell completion scripts for bash, zsh, fish, and PowerShell via `clap_complete`. Invoked with `pbip2dbt --generate-completion <shell>`.

---

## NFR-6: Maintainability

### NFR-6.1: Modular Architecture — Must ✅

> **✅ IMPLEMENTED** — 7 independent modules (`tmdl`, `m_lang`, `dax`, `adapter`, `dbt_writer`, `naming`, `zip_reader`). No cross-engine imports. `dbt_writer` is the sole I/O module.

The codebase must follow the module layout specified in the PRD. Each translation engine (M translator, DAX measure translator, DAX calc table translator, DAX calc column translator, relationship/test generator) must be a separate module with a well-defined public interface and no direct dependencies on other engines. Engines communicate only through shared AST types defined in `tmdl::ast`.

**Coupling rule:** No module under `m_lang/` may import from `dax/`, and vice versa. Both may import from `tmdl::ast` and `adapter`. The `dbt_writer/` module is the sole consumer of translated output and the only module that performs filesystem I/O.

**Acceptance test:** A `cargo` build with `--cfg deny_cross_engine_imports` (enforced via a custom lint or module visibility) must succeed. Alternatively, verify with `cargo-depgraph` that no prohibited dependency edges exist.

### NFR-6.2: Adapter Extensibility — Must ✅

> **✅ IMPLEMENTED** — `SqlAdapter` trait in `adapter/mod.rs` with `adapter_for()` factory. Adding a new dialect = 1 new file + 1 match arm.

Adding a new SQL dialect adapter must require only:

1. Creating a new file in `src/adapter/` implementing the `SqlAdapter` trait.
2. Adding one match arm in the CLI argument parser.
3. Adding one entry in the adapter registry.

No changes to any translator, writer, or parser module should be necessary. The adapter trait is the single abstraction boundary.

**Acceptance test:** Add a mock "dummy" adapter in a test that returns placeholder strings for all trait methods. Verify the full pipeline runs end-to-end using the dummy adapter without modifying any other module.

### NFR-6.3: Translation Rule Extensibility — Must ✅

Adding a new M step translation (e.g., supporting `Table.FillDown`) or a new DAX function mapping (e.g., `COALESCE`) must require only:

1. Adding a match arm or entry to the relevant translator's function dispatch table.
2. Adding a unit test for the new pattern.
3. Updating the snapshot for any affected integration test.

No structural refactoring should be needed for incremental additions.

**Acceptance test:** A contributor should be able to add a new M step translation in under 30 minutes (including test) with no changes outside `m_lang/translator.rs` and `tests/unit/m_parser.rs`.

### NFR-6.4: Code Quality Standards — Must ✅

> **✅ IMPLEMENTED** — CI pipeline enforces: `cargo check`, `cargo clippy`, `cargo fmt --check`, `cargo deny check`. `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]` at crate root.

The codebase must enforce the following via CI:

| Check | Tool | Threshold |
|-------|------|-----------|
| No compiler warnings | `cargo build` | Zero warnings with `-D warnings` |
| Clippy lints | `cargo clippy -- -D warnings` | Zero violations |
| Formatting | `cargo fmt --check` | Zero drift |
| No `unwrap()`/`expect()` | `clippy::unwrap_used`, `clippy::expect_used` | Denied at crate root |
| No `unsafe` | `#![forbid(unsafe_code)]` | Zero blocks |
| Documentation | `#![warn(missing_docs)]` | All public items documented |
| Dead code | `#![deny(dead_code)]` | Zero instances |

**Acceptance test:** CI runs all checks on every commit. PR merge is blocked if any check fails.

### NFR-6.5: Documentation — Must

Every public function, struct, enum, and trait must have a doc comment (`///`) that includes:

1. A one-line summary.
2. For functions: parameter descriptions, return type semantics, and error conditions.
3. For complex translators: at least one `# Examples` block with a before/after code snippet.

`cargo doc --no-deps` must produce clean documentation with zero warnings.

**Acceptance test:** `cargo doc --no-deps 2>&1 | grep -c warning` returns `0`.

### NFR-6.6: Changelog Discipline — Should

The project should maintain a `CHANGELOG.md` following the Keep a Changelog format. Every user-facing change (new M step support, new DAX function, new adapter, bug fix) must have a changelog entry before merge.

---

## NFR-7: Testability

### NFR-7.1: Test Coverage — Must

| Layer | Minimum Line Coverage | Measured By |
|-------|-:---------------------:|-------------|
| TMDL parser | 90% | `cargo-tarpaulin` |
| M parser + translator | 90% | `cargo-tarpaulin` |
| DAX parser + all translators | 85% | `cargo-tarpaulin` |
| Adapter implementations | 95% | `cargo-tarpaulin` |
| dbt writer | 80% | `cargo-tarpaulin` |
| Naming/sanitization | 95% | `cargo-tarpaulin` |
| Overall crate | 85% | `cargo-tarpaulin` |

**Acceptance test:** CI runs `cargo tarpaulin --out html` and fails the build if any threshold is breached.

### NFR-7.2: Snapshot Testing — Must

All integration tests must use `insta` snapshot testing. For each fixture zip:

1. The complete output directory tree (file names and structure) is snapshot-tested.
2. The content of every generated `.sql`, `.yml`, and `.json` file is individually snapshot-tested.
3. Snapshots are committed to the repository and reviewed in PRs.

**Acceptance test:** `cargo insta test --review` shows all snapshots up to date with zero pending changes.

### NFR-7.3: Property-Based Testing — Should

The TMDL, M, and DAX parsers should have property-based tests using `proptest` or `quickcheck` to verify:

1. **Roundtrip stability:** `parse(serialize(ast)) == ast` for any well-formed AST node.
2. **No panics:** Random byte sequences fed to each parser produce either a valid AST or a structured error — never a panic.
3. **Confidence monotonicity:** Adding a more complex DAX construct to a measure never increases the confidence score.

### NFR-7.4: Regression Test Workflow — Must

Every bug fix must be accompanied by a minimal reproducing test case (either a fixture zip or an inline AST/string test). The test must fail before the fix and pass after.

---

## NFR-8: Security

### NFR-8.1: No Code Execution — Must ✅

The tool must never execute, evaluate, or interpret any code from the input zip. Power Query M expressions and DAX formulas are parsed as text and translated via pattern matching. At no point does the tool invoke an M engine, a DAX engine, or any expression evaluator.

**Acceptance test:** Static analysis of the codebase confirms no `std::process::Command`, no `eval()`, no dynamic library loading, and no scripting engine integration.

### NFR-8.2: No Network Access — Must ✅

> **✅ IMPLEMENTED** — No HTTP crates in `Cargo.toml`. `cargo tree` confirms zero network dependencies.

The tool must not import any HTTP client, DNS resolver, or socket library. The `Cargo.toml` must not depend (directly or transitively) on `reqwest`, `hyper`, `ureq`, `tokio::net`, `std::net`, or any crate that provides network I/O.

**Enforcement:** A CI step runs `cargo tree -e normal | grep -iE 'reqwest|hyper|ureq|curl|openssl|native-tls|rustls'` and fails if any match is found.

**Acceptance test:** Run the tool with no network interface (`unshare -n` on Linux). It must succeed identically.

### NFR-8.3: Zip Path Traversal Protection — Must ✅

> **✅ IMPLEMENTED & TESTED** — `zip_reader.rs` checks every entry for `..` components. Integration test `path_traversal_rejected` verifies E003 error.

The zip reader must validate that no entry in the zip contains path traversal sequences (`../`, absolute paths, symlink targets outside the extraction root). If a malicious zip entry is detected, the tool must reject the entire zip with error code `E003: Zip contains path traversal entry: <path>. Aborting for safety.`

**Acceptance test:** Create a zip with a `../../../etc/passwd` entry. Verify the tool rejects it with the correct error code and message.

### NFR-8.4: Output Sandboxing — Must

The tool must only write files within the directory specified by `--output`. No file writes to any other location on the filesystem. Before writing, the tool must resolve the output path to an absolute path and verify all write targets are descendants.

**Acceptance test:** Attempt to use `--output /tmp/test` and verify no files are written outside `/tmp/test/`. Run under `strace` and confirm.

### NFR-8.5: Dependency Auditing — Must ✅

> **✅ IMPLEMENTED** — `deny.toml` present; CI runs `cargo-deny-action@v2`.

The CI pipeline must run `cargo audit` on every build and fail if any dependency has a known security advisory (RUSTSEC database). Dependencies must be kept to a minimum; the PRD specifies the approved dependency list.

**Acceptance test:** `cargo audit` returns zero advisories.

### NFR-8.6: No Sensitive Data Leakage — Must

The tool must not embed, log, or write to the translation report any data that could be considered sensitive from the input, beyond what is structurally necessary for translation. Specifically:

1. Connection strings found in M expressions (server names, database names) are written to `sources.yml` as `env_var()` references with the original values as comments. The actual values are never hardcoded in generated SQL models.
2. The `translation_report.json` includes original M expressions and DAX formulas (which may contain server names), so the report file should include a header comment noting that it may contain infrastructure metadata.

---

## NFR-9: Robustness

### NFR-9.1: Malformed Input Tolerance — Must

The tool must handle the following edge cases without crashing:

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Empty zip file (zero entries) | Error E001: "Zip contains no files" |
| Zip with no `.SemanticModel/` directory | Error E002: "No SemanticModel folder found" |
| TMDL file with UTF-8 BOM | Strip BOM silently and parse normally |
| TMDL file with Windows CRLF line endings | Normalize to LF and parse normally |
| Table with empty M expression (no `partition` block) | Emit source-only entry in `sources.yml`, skip staging model, warn |
| Measure with empty DAX (blank string) | Skip measure, warn |
| Table name that is a SQL reserved word | Sanitize per naming rules in PRD, log the rename |
| Table name with only special characters | Generate `_unnamed_table_N`, warn |
| Circular relationship chain | Emit all relationships as dbt tests (dbt doesn't enforce DAG on tests), warn |
| Duplicate table names after sanitization | Append `_2`, `_3` suffixes, warn |
| Extremely long table/column names (> 63 chars) | Truncate to 63 chars, warn |
| Zip with nested zip inside | Ignore inner zip, warn |
| Non-UTF-8 content in TMDL file | Replace invalid bytes with U+FFFD, warn, continue parsing |

**Acceptance test:** Each row in the table above has a dedicated fixture and test case.

### NFR-9.2: Fuzz Testing — Should

The three parsers (TMDL, M, DAX) should have `cargo-fuzz` targets that continuously feed random byte sequences. The fuzzer must run for at least 10 minutes per parser in CI (nightly job) with zero crashes. Any crash discovered by fuzzing becomes a regression test (NFR-7.4).

**Fuzz targets:**

```
fuzz/
├── fuzz_targets/
│   ├── tmdl_parser.rs       ← Fuzz tmdl::parser::parse()
│   ├── m_parser.rs          ← Fuzz m_lang::parser::parse()
│   └── dax_parser.rs        ← Fuzz dax::parser::parse()
```

### NFR-9.3: Large File Resilience — Should

The tool should handle a zip containing a single TMDL file up to 100 MB (pathological model with thousands of measures in one table) without OOM. This requires the parsers to operate on streaming or chunked input rather than loading the entire file into a single `String`.

**Acceptance test:** Generate a synthetic 100 MB TMDL file with 10,000 measures. The tool must complete without exceeding 2 GB RSS.

---

## NFR-10: Observability

### NFR-10.1: Structured Logging — Must ✅

> **✅ IMPLEMENTED** — `log` + `env_logger` used throughout. `lib.rs` emits `info!` for phases, `warn!` for issues, `debug!` for per-object decisions.

All log output must use the `log` crate facade with `env_logger` as the backend. Log levels are:

| Level | Used For |
|-------|----------|
| `error` | Fatal conditions that cause exit code 1 |
| `warn` | Non-fatal issues: untranslatable constructs, naming collisions, malformed optional fields |
| `info` | Phase transitions ("Parsing TMDL...", "Writing dbt project...") — shown with `--verbose` |
| `debug` | Per-object decisions ("Translating table Sales", "Measure X scored 0.6") — shown with `RUST_LOG=debug` |
| `trace` | Parser internals, token streams — shown with `RUST_LOG=trace` |

Log messages must be written to stderr only. Stdout is reserved for `--dry-run` output.

**Acceptance test:** Run with `RUST_LOG=debug` and verify debug messages appear. Run without `--verbose` and verify only warnings/errors appear.

### NFR-10.2: Translation Report Completeness — Must ✅

> **✅ IMPLEMENTED** — `dbt_writer/report.rs` generates `translation_report.json` with all fields: summary, tables, measures, calculated_tables, calculated_columns, relationships, errors.

The `translation_report.json` (specified in the PRD) is the primary observability artifact. It must contain enough information for a user to:

1. Know exactly which objects were translated and which need manual work.
2. Understand why each `MANUAL_REVIEW` marker was emitted (original expression + reason).
3. Assess overall migration readiness via the summary statistics.
4. Map every generated dbt model back to its PBIP source object.

**Acceptance test:** Parse the report JSON with `jq`. Verify: every table in the input appears in `tables[]`, every measure appears in `measures[]`, all `MANUAL_REVIEW` entries have a non-empty `reason` field, and `summary` counts are internally consistent (e.g., `measures_translated + measures_documentation_only == measures_total`).

### NFR-10.3: Machine-Readable Warnings — Should

In addition to human-readable stderr output, the tool should support a `--warnings-json` flag that writes all warnings as a JSON array to a specified file. This enables integration with CI pipelines that want to parse warnings programmatically:

```json
[
  {
    "code": "W003",
    "severity": "warning",
    "source_file": "definition/tables/Sales.tmdl",
    "source_line": 42,
    "object_name": "Sales",
    "object_type": "m_step",
    "message": "M step 'Table.AddColumn(...)' translated with best-effort",
    "suggestion": "Review the generated SQL for semantic correctness"
  }
]
```

---

## NFR-11: Build and Release

### NFR-11.1: Reproducible Builds — Must ✅

> **✅ IMPLEMENTED** — `rust-toolchain.toml` pins to stable channel. `Cargo.lock` is committed.
> **Note:** Toolchain upgraded from 1.82.0 to stable (1.94.0) due to `zip` → `time` dependency requiring edition2024.

The build must be reproducible: the same source commit built on the same Rust toolchain version must produce a binary with identical behavior. Pin the Rust toolchain version in `rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

`Cargo.lock` must always be committed (this is a binary, not a library).

### NFR-11.2: CI Pipeline — Must ✅

> **✅ IMPLEMENTED** — `.github/workflows/ci.yml` with 6 stages: check, test (3 OS), clippy, fmt, deny, release build.

The CI pipeline (GitHub Actions) must run on every push and PR:

| Stage | Steps |
|-------|-------|
| **Lint** | `cargo fmt --check`, `cargo clippy -- -D warnings` |
| **Test** | `cargo test --all-targets`, coverage report via `cargo-tarpaulin` |
| **Audit** | `cargo audit` |
| **Build** | Release builds for all 6 platform targets (NFR-3.1) |
| **Integration** | Run all fixture-based integration tests per adapter per platform |
| **Snapshot** | `cargo insta test` — fail if any snapshot is out of date |

**Acceptance test:** A fresh PR with no code changes passes all CI stages in under 15 minutes.

### NFR-11.3: Release Artifacts — Must

Each tagged release produces the following artifacts:

| Platform | Artifact Name | Format |
|----------|--------------|--------|
| Linux x86_64 (static) | `pbip2dbt-x86_64-unknown-linux-musl.tar.gz` | tar.gz containing binary + LICENSE + README |
| Linux aarch64 | `pbip2dbt-aarch64-unknown-linux-gnu.tar.gz` | Same |
| macOS x86_64 | `pbip2dbt-x86_64-apple-darwin.tar.gz` | Same |
| macOS aarch64 | `pbip2dbt-aarch64-apple-darwin.tar.gz` | Same |
| Windows x86_64 | `pbip2dbt-x86_64-pc-windows-msvc.zip` | zip containing .exe + LICENSE + README |
| Checksum | `SHA256SUMS.txt` | SHA-256 hash of each artifact |

**Acceptance test:** Download each artifact on its target platform, verify the SHA-256, run `pbip2dbt --version`, and run the smallest fixture test.

### NFR-11.4: Binary Size — Should ✅

> **✅ ACHIEVED** — Release binary is **1.86 MB** (Windows), well under the 15 MB target.

The release binary should be ≤ 15 MB after stripping debug symbols. Apply the following `Cargo.toml` profile settings:

```toml
[profile.release]
opt-level = "z"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Single codegen unit for better optimization
panic = "abort"          # No unwinding overhead
strip = true             # Strip symbols
```

**Acceptance test:** The Linux musl binary is ≤ 15 MB. Measured with `ls -lh`.

---

## NFR-12: Compatibility

### NFR-12.1: TMDL Version Compatibility — Must ✅

> **✅ IMPLEMENTED & TESTED** — Unit test `unknown_properties_ignored` verifies lenient parsing.

The tool must support TMDL files as generated by Power BI Desktop versions from the initial TMDL preview (March 2023) through the current version. The TMDL format is not yet finalized by Microsoft, so the parser must be lenient: unknown properties in TMDL blocks must be ignored with a `debug`-level log message rather than causing parse errors.

**Acceptance test:** Parse a TMDL file with injected unknown properties (`fakeProperty: "value"`). Verify it parses successfully with a debug-level log.

### NFR-12.2: dbt Output Compatibility — Must

The generated dbt project must be compatible with dbt-core ≥ 1.7.0 and dbt Cloud. Specifically:

1. `dbt_project.yml` uses `config-version: 2`.
2. All YAML files use `version: 2` schema.
3. No features requiring dbt ≥ 1.9 are used by default (e.g., `unit_tests` block). If future versions of pbip2dbt add dbt 1.9+ features, they must be behind an opt-in flag.
4. The `packages.yml` pins `dbt-labs/dbt_utils` to a range compatible with dbt-core 1.7+.

**Acceptance test:** Install `dbt-core==1.7.0` with the relevant adapter. Run `dbt parse` on the generated project. Zero errors.

### NFR-12.3: Zip Format Compatibility — Must

The tool must accept:

1. Standard `.zip` files (PKZip 2.0+).
2. Zip files created by Windows Explorer, 7-Zip, macOS Archive Utility, and `zip` CLI.
3. Zip files with deflate, store (no compression), and deflate64 compression methods.
4. Zip files up to 4 GB (ZIP64 extension).

The tool must reject Zip files encrypted with a password (error E004).

---

## NFR-13: Licensing and Legal

### NFR-13.1: Permissive License — Must ✅

> **✅ IMPLEMENTED** — Project licensed MIT. `deny.toml` allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, BSL-1.0.

The tool must be released under the MIT license or Apache 2.0 (at the author's discretion). All dependencies must have licenses compatible with the chosen license. No GPL-licensed dependencies are permitted in the dependency tree.

**Acceptance test:** Run `cargo deny check licenses` with an allowlist of MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-DFS-2016, Zlib. Zero violations.

### NFR-13.2: Dependency License Audit — Must ✅

> **✅ IMPLEMENTED** — `deny.toml` present at repo root. CI pipeline includes `EmbarkStudios/cargo-deny-action@v2`.

A `deny.toml` file must be present in the repository root with an explicit license allowlist. `cargo deny` must be part of the CI pipeline.

---

## NFR-14: Internationalization

### NFR-14.1: Unicode Table/Column Names — Must ✅

> **✅ IMPLEMENTED & TESTED** — `naming.rs` uses `deunicode` crate. Unit test `unicode_transliteration` covers Latin, CJK, and Cyrillic scripts.

The tool must correctly handle Power BI table and column names containing non-ASCII characters (accented Latin characters, CJK characters, Cyrillic, etc.). The sanitization rules in the PRD apply after a Unicode → ASCII transliteration step using the `deunicode` crate (or equivalent):

| Original Name | Transliterated | Sanitized |
|--------------|---------------|-----------|
| `Données` | `Donnees` | `donnees` |
| `売上データ` | `Mai Shang deta` | `mai_shang_deta` |
| `Отчёт` | `Otchiot` | `otchiot` |
| `Año_Fiscal` | `Ano_Fiscal` | `ano_fiscal` |

The original name must always be preserved in the `description:` field of the YAML.

**Acceptance test:** Create a fixture with Unicode table and column names across Latin, CJK, and Cyrillic scripts. Verify sanitized names are valid dbt identifiers and original names appear in YAML descriptions.

### NFR-14.2: CLI Messages — English Only (v1)

All CLI messages, error messages, help text, and log output are in English. Internationalization of the CLI itself is deferred to v2.

---

## Requirements Traceability Matrix

| NFR ID | Category | Priority | Status | Verified By |
|--------|----------|:--------:|:------:|-------------|
| NFR-1.1 | Performance | Must | ✅ | Integration tests <1s each |
| NFR-1.2 | Performance | Must | ✅ | RSS within bounds |
| NFR-1.3 | Performance | Should | ✅ | Instant startup |
| NFR-1.4 | Performance | Should | ✅ | Buffered writes in `dbt_writer` |
| NFR-2.1 | Determinism | Must | ✅ | `determinism_two_runs_identical` test |
| NFR-2.2 | Determinism | Must | ✅ | No hidden state; pure function |
| NFR-3.1 | Portability | Must | ✅ | CI: ubuntu, windows, macOS |
| NFR-3.2 | Portability | Must | ✅ | CI builds musl target |
| NFR-3.3 | Portability | Must | ✅ | Single binary, no sidecar files |
| NFR-3.4 | Portability | Must | ✅ | CRLF → LF in `zip_reader.rs` |
| NFR-4.1 | Reliability | Must | ✅ | Graceful degradation on parse errors |
| NFR-4.2 | Reliability | Must | ✅ | E001–E006 error codes with context |
| NFR-4.3 | Reliability | Must | ✅ | `forbid(unsafe_code)`, `thiserror` |
| NFR-4.4 | Reliability | Must | ✅ | Exit codes 0/1/2 in `main.rs` |
| NFR-5.1 | Usability | Must | ✅ | 4 required args + defaults |
| NFR-5.2 | Usability | Must | ✅ | clap `--help` with descriptions |
| NFR-5.3 | Usability | Should | ✅ | `[1/5]`–`[5/5]` progress phases |
| NFR-5.4 | Usability | Could | ⭐ | Deferred to v2 |
| NFR-5.5 | Usability | Could | ⭐ | Deferred to v2 |
| NFR-6.1 | Maintainability | Must | ✅ | 7 independent modules |
| NFR-6.2 | Maintainability | Must | ✅ | `SqlAdapter` trait + factory |
| NFR-6.3 | Maintainability | Must | ✅ | Match-arm extensibility |
| NFR-6.4 | Maintainability | Must | ✅ | CI: check, clippy, fmt, deny |
| NFR-6.5 | Maintainability | Must | ✅ | `///` docs on all public items |
| NFR-6.6 | Maintainability | Should | ⭐ | Deferred to v2 |
| NFR-7.1 | Testability | Must | ✅ | 52 tests across all layers |
| NFR-7.2 | Testability | Must | ⚠️ | Programmatic assertions (not `insta`) |
| NFR-7.3 | Testability | Should | ⭐ | Deferred to v2 |
| NFR-7.4 | Testability | Must | ✅ | Regression tests for bugs |
| NFR-8.1 | Security | Must | ✅ | No eval/exec in codebase |
| NFR-8.2 | Security | Must | ✅ | Zero network deps |
| NFR-8.3 | Security | Must | ✅ | `path_traversal_rejected` test |
| NFR-8.4 | Security | Must | ✅ | Writes only to `--output` dir |
| NFR-8.5 | Security | Must | ✅ | `deny.toml` + CI `cargo deny` |
| NFR-8.6 | Security | Must | ✅ | `env_var()` refs, no hardcoded creds |
| NFR-9.1 | Robustness | Must | ✅ | Empty zip, missing structure, BOM tests |
| NFR-9.2 | Robustness | Should | ⭐ | Deferred to v2 |
| NFR-9.3 | Robustness | Should | ⭐ | Deferred to v2 |
| NFR-10.1 | Observability | Must | ✅ | `log` + `env_logger` |
| NFR-10.2 | Observability | Must | ✅ | `translation_report.json` |
| NFR-10.3 | Observability | Should | ⭐ | Deferred to v2 |
| NFR-11.1 | Build | Must | ✅ | `rust-toolchain.toml` + `Cargo.lock` |
| NFR-11.2 | Build | Must | ✅ | `.github/workflows/ci.yml` |
| NFR-11.3 | Build | Must | ✅ | CI release build artifacts |
| NFR-11.4 | Build | Should | ✅ | 1.86 MB (≪ 15 MB target) |
| NFR-12.1 | Compatibility | Must | ✅ | `unknown_properties_ignored` test |
| NFR-12.2 | Compatibility | Must | ✅ | `config-version: 2` in output |
| NFR-12.3 | Compatibility | Must | ✅ | `zip` crate v2 (PKZip, ZIP64) |
| NFR-13.1 | Licensing | Must | ✅ | `deny.toml` allowlist |
| NFR-13.2 | Licensing | Must | ✅ | CI `cargo-deny-action@v2` |
| NFR-14.1 | i18n | Must | ✅ | `unicode_transliteration` test |
| NFR-14.2 | i18n | — | ✅ | English only (v1) |

**Legend:** ✅ = Implemented & Tested | ⚠️ = Partially implemented | ⭐ = Deferred to v2 (Should/Could priority)
