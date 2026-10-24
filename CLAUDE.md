# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**project-indicator** is a high-performance Rust CLI tool for detecting project types and frameworks. It's designed to be integrated into shell prompts; warm invocations are served from a persistent cache in well under a millisecond of binary time.

**Core Goal**: Identify programming language and frameworks in a directory through file pattern matching, dependency analysis, and confidence scoring—fast enough for real-time shell integration.

## Development Commands

### Building & Testing

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run all tests (including property-based tests)
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run only library tests
cargo test --lib

# Run property-based tests
cargo test --test property_tests

# Run symlink handling tests
cargo test --test symlink_handling_tests

# Format code
cargo fmt --all

# Lint with clippy (STRICT: unwrap/expect are DENIED)
cargo clippy --all-targets --all-features -- -D warnings

# Check documentation
cargo doc --no-deps --document-private-items
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench component_benchmark  # pattern matching + file parse caches
cargo bench --bench detection_benchmark  # end-to-end engine.detect scenarios

# Dry run (compile but don't run)
cargo bench --no-run
```

### Running the CLI

```bash
# Basic detection (current directory)
cargo run

# Detect specific directory
cargo run -- /path/to/project

# Different output formats
cargo run -- --format json
cargo run -- --format full

# Debugging (full evidence dump)
cargo run -- --format debug

# Persistent cache management
cargo run -- cache stats
cargo run -- cache clear

# Configuration
cargo run -- config validate
cargo run -- config show
cargo run -- config init --template rust-dev
```

## Architecture Overview

### Core Detection Pipeline

The detection flow follows this path:

1. **DetectionEngine** (`src/detection/engine.rs`) - Orchestrator
   - Creates and coordinates all specialized components
   - Wires the per-run caches and components together

2. **RootIndicatorEngine** - Project root discovery
   - Walks upward from the start path (the start directory plus up to
     `max_upward_traversal` levels above it) looking for high-confidence root
     files (`.git`, `Cargo.toml`, etc.) to pick the directory to scan
   - Refuses to scan from boundary directories (home, system dirs)
   - Wildcard markers (`*.csproj`) need a directory listing, so they are not
     looked for in directories with more than 500 entries (literal markers
     are still found by `stat`)

3. **ScanningEngine** (`src/detection/scanner/`) - File discovery
   - Traverses directory with configurable depth: a single sequential,
     name-sorted walk (deterministic; no parallel path)
   - Exact patterns that are relative paths (`src/main/java`,
     `src/main/kotlin`, `src/main/scala`) are probed with one `stat` each, so
     a shared build tool (Gradle/Maven) is classified by its source language
     even though the default depth never reaches those files
   - Root exact-name manifests are seeded first and don't count against the
     15-match cap; if the cap is hit, a root-only pass tops up root-level
     matches (`App.csproj` after a busy `Controllers/`). Only ignore files
     inside the scan root apply, and `max_depth = 0` scans nothing
   - Exact-name root manifests (and the oversized-root check) are literal
     lookups that deliberately ignore .gitignore, so an ignored `go.work`
     or `Cargo.toml` still counts; the walk and the top-up honour it
   - Oversized roots (>500 entries) only get the root-level check; an
     adaptive timeout cuts long walks short (such results are not cached)
   - Uses `PatternProcessor` for efficient pattern matching

4. **IndicatorResolver** - Conflict resolution
   - Handles multi-indicator projects (e.g., TypeScript + Rust)
   - Candidates scoring >= 0.5 compete on score (priority only breaks
     ties); below that the declared priority decides (lower number wins)

5. **ConfidenceScorer** - Score calculation
   - Depth-based scoring (root files weighted highest)
   - Directory type multipliers (src/ weighted higher than node_modules/)
   - Pattern importance factors (package.json > *.js)
   - Root indicator bonus

6. **Framework detection** (`src/detection/framework_detector.rs`) - Framework identification
   - Candidates scoped by ecosystem intersection with the winning indicator
   - Ecosystem-specific matchers (`src/detection/matchers/ecosystems/`)
   - Dependency analysis from package.json, Cargo.toml, deno.json, etc.

7. **OutputFormatter** (`src/output/`) - Result rendering
   - Multiple formats: simple, full, json, debug
   - One render function per format

### Caching Architecture

#### 0. PersistentCache (Cross-invocation)

- **Location**: `src/cache.rs`
- **Purpose**: One JSON entry per project directory under
  `$XDG_CACHE_HOME/project-indicator/results/`; a warm prompt hit skips
  config parsing and engine construction entirely
- **Invalidation**: evidence-keyed mtimes — the entry stores mtimes of the
  directory, active config file, and every file that influenced the result,
  plus the binary build (package version + executable size/mtime, so a
  rebuild without a version bump misses), the directories above it that
  root discovery consulted (bounded by the configured `max_upward_traversal`),
  the subdirectories the scan listed (`max_depth > 1`, `watched_dirs`; a scan
  that listed more than 256 is not stored), the nearest existing parent of
  each probed source dir, and the `.gitignore`/`.ignore`/`.git/info/exclude`
  files that shaped the walk, every file a matcher read (lockfiles no
  indicator pattern lists), the nearest existing parent of every
  `FileExists` probe path, and — with `require_vcs_root` — the ancestors up
  to the traversal bound; any mismatch is a miss. A result is not stored if
  anything it depends on changed after detection started (or within 20 ms
  before it: file timestamps can come from a coarser clock)
- **Failure policy**: every error degrades silently to a fresh detection
  (`log::debug` only) — the cache must never break a prompt
- **Bypass**: `--no-cache`, or automatically when a `--max-depth` override
  is passed

The remaining caches live and die within a single detection run:

#### 1. FileSystemCache (Per-detection run)

- **Location**: `src/performance.rs`
- **Purpose**: Memoize file existence / is-file checks so root discovery and
  the framework detector don't repeat `stat` calls

#### 2. ParsedFileCache (Per-detection run)

- **Location**: `src/detection/caches/parsed_file.rs`
- **Purpose**: Cache parsed JSON/TOML content so manifests like `package.json`
  are parsed at most once per run

(Pattern matching itself is NOT cached: a DashMap memo was benchmarked and
removed — a cache hit cost ~6× more than computing the match, and removal cut
end-to-end detection ~70%. Don't reintroduce it without new numbers.)

### Configuration System

**Template-based**: one indicator template per language (`src/config/templates/`) plus a
single ecosystem-keyed framework catalog (`src/config/templates/frameworks.rs`)

- Each indicator module (e.g., `rust.rs`, `deno.rs`) defines file patterns,
  root indicators, and the ecosystems it participates in
- Frameworks are defined ONCE in the catalog, tagged with their ecosystems;
  an indicator surfaces every framework whose ecosystems intersect its own
- Users can customize via `~/.config/project-indicator/config.toml`
  (schema v3: `[[indicators]]` + `[[frameworks]]`)
- Parsing is strict (`deny_unknown_fields`): removed or misspelled keys fail
  to parse instead of being silently ignored. At prompt time a failing config
  soft-falls-back to the built-in template (never breaks the shell);
  `config validate` surfaces the exact error and `config init --force`
  regenerates. Exception: keys under `[frameworks.detection]` — serde cannot
  deny unknowns inside an internally-tagged enum

**Key Configuration Types**:

- `DetectionConfig`: Scan depth, upward traversal, VCS-root requirement
- `DisplayConfig`: Output formatting preferences

### Type System

All types in `src/types/` organized by domain:

- `config.rs`: Configuration structures
- `detection.rs`: Detection results and evidence
- `framework.rs`: Framework detection types
- `indicators.rs`: Indicator and root indicator definitions
- `matched_file.rs`: File matching results

## Critical Implementation Rules

### 1. NO UNWRAP/EXPECT

- **Clippy lints enforce**: `unwrap_used = "deny"`, `expect_used = "deny"`
- Use `?` operator, `.ok_or()`, or `match` statements
- All tests must also avoid unwrap/expect

### 2. Pattern Matching

Pattern matching is a free function: `crate::patterns::matches_pattern`.
There is no matcher object and no cache (a memo was benchmarked out — see
Caching Architecture); components call the function directly.

### 3. Error Handling

- Use `anyhow::Result<T>` for functions that can fail
- No custom error types: `anyhow::bail!` / `anyhow!` carry the message; don't
  add an error-derive dependency
- Always provide context: `.context("Failed to parse config")?`

### 4. Performance Considerations

- **Cache key design**: Use `PathBuf` with `Borrow<Path>` trait to avoid allocations
- **Bound the scan**: The match cap and adaptive timeout keep worst-case scans cheap
- **Memory limits**: files over 1MB are read but not kept in the per-run parse cache

### 5. Testing Requirements

- Property-based tests for invariants (use `proptest`)
- Edge case coverage (symlinks, large projects, empty directories)
- All new features require tests

## Adding New Indicator or Framework Support

**Indicators** (project types — languages, runtimes, toolchains):

1. Create template file: `src/config/templates/<name>.rs`:

```rust
pub fn create_mylang_indicator() -> Indicator {
    Indicator::with_root_indicators(
        "MyLang".to_string(),
        vec!["*.ml".to_string(), "mylang.toml".to_string()],
        "#COLOR".to_string(),
        nerd_icon("xxxx"),
        PRIORITY, // lower wins; infra toolchains use 12+ so app code outranks them
        vec![Ecosystem::Npm], // ecosystems this project type participates in
        vec![root_indicator("mylang.toml", 0.95, IndicatorContext::LanguageRoot)],
    )
}
```

2. Register in `src/config/templates/mod.rs` (module, import, full template list).

**Frameworks** (defined once, ecosystem-keyed):

1. Add to the catalog — either `src/config/templates/frameworks.rs` directly or a
   per-language `*_frameworks()` fn that the catalog extends:

```rust
dependency_framework(
    "MyFramework",
    &[Ecosystem::Npm], // scoping: surfaced by indicators sharing an ecosystem
    &["my-framework"],
    None,              // nerd-font code point in hex, e.g. Some("e7ba")
    Some("#COLOR"),
    PRIORITY,
) // file_framework(...) detects by file presence; with_roots(...) adds root indicators
```

2. New package ecosystem? Add an `Ecosystem` variant, a matcher in
   `src/detection/matchers/ecosystems/`, and a `check_ecosystem` dispatch arm.
3. Add a fixture under `tests/fixtures/<name>/` and a row in
   `tests/fixture_detection_tests.rs` (enforced — the suite fails on fixtures
   without expectations).

## Adding New Output Format

1. Add a render function in `src/output/render.rs`:

```rust
pub fn render_mine(result: &DetectionResult) -> String {
    // Implementation
}
```

2. Add an `OutputFormat` variant and match arm: `src/output/formatters.rs`
3. Update CLI help and documentation

## Adding New Detection Strategy

For ecosystem-specific dependency matching:

1. Create matcher: `src/detection/matchers/ecosystems/<ecosystem>.rs`. Most are
   one `first_hit(...)` call over `(file, check_fn)` probes (`helpers.rs`);
   manifests that declare dependencies are authoritative, so a lockfile probe
   only runs when the manifest declares none
2. Read files through `ParsedFileCache` (parsed at most once per run)
3. Add to the `check_ecosystem` dispatch: `src/detection/matchers/dependency_matcher.rs`

## Performance Optimization Guidelines

### When optimizing

1. **Benchmark first**: Use `cargo bench` to establish baseline
2. **Profile**: Run with `cargo bench --bench detection_benchmark`
3. **Cache aggressively**: But measure cache overhead vs benefit
4. **Plain std maps**: per-run caches are `RefCell<HashMap>` (a detection run is single-threaded and the caches are built inside `detect`, so they are `!Sync`; keep each borrow to a single lookup/insert, never across I/O or another cache call)
5. **Document complexity**: Add Big-O notation comments for algorithms

### Performance targets

- Shell prompt scenario (warm cache hit): < 10ms wall including process
  spawn (~5ms measured; in-process cache lookup itself is microseconds)
- Cold detection: < 10ms wall including spawn (~5ms measured; in-process
  detection is ~0.4ms for a cold engine, ~0.23ms warm, per the criterion
  benchmarks — the pattern-memo removal cut it ~70%)
- Worst case (deep nesting): < 50ms

## Project Structure Notes

- `src/cli.rs`: CLI argument parsing (uses clap)
- `src/main.rs`: Entry point, command routing
- `src/lib.rs`: Library interface for external use
- `src/constants.rs`: Centralized file/extension constants
- `src/patterns.rs`: Pattern matching utilities
- `src/commands/`: CLI command implementations (detect, config, cache)

## Release Process

Automated via GitHub Actions:

- The PR's semver label picks the bump: `major`, `minor`, `patch`, or
  `no-release`. Titles are not read; an unlabeled merge or a direct push to
  `main` defaults to a patch release
- Merge to main triggers auto-release with cross-platform binaries
- Binaries built for: Linux x86_64/ARM64, macOS Intel/Apple Silicon, Windows

## Security Considerations

- **Editor**: `config edit` spawns `$EDITOR` directly (never via a shell), splitting arguments
- **Path traversal protection**: bounded upward traversal (`max_upward_traversal`), boundary checks
- **No file modifications**: Tool is read-only
- **File size**: files over 1MB are read but not kept in the per-run parse cache
- **Symlink safety**: directory symlinks are never followed (no loops possible); symlinked files count as files
