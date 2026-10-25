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
cargo run -- config init
```

## Architecture Overview

### Core Detection Pipeline

The detection flow follows this path:

1. **DetectionEngine** (`src/detection/engine.rs`) - Orchestrator
   - Creates and coordinates all specialized components
   - Wires the per-run caches and components together
   - A VCS-tracked folder (`.git` — file or directory — `.hg` or `.svn`)
     that is no project and has none above it (no scan root, no indicator,
     not a boundary directory) is a *container*: `DetectionEngine::detect`
     runs `detect_one` on up to `MAX_NESTED_EXAMINED` (24) subdirectories by
     name across two levels (`apps/*`; hidden dirs, symlinks and noise dirs
     like node_modules skipped; a found project is not looked into) and
     keeps up to `MAX_NESTED_SHOWN` (4) projects in
     `DetectionResult::nested`. Always on. Plain folders without a VCS
     marker are never scanned inside. Every examined directory and each
     subproject's read files / watched dirs are merged into the container's
     evidence (absolute paths), so the persistent cache stays correct

2. **RootIndicatorEngine** - Project root discovery
   - Walks upward from the start path (the start directory plus up to
     `MAX_UPWARD_TRAVERSAL` (4) levels above it) looking for qualifying root
     markers (`Cargo.toml`, `Gemfile`, …; `.git` is never itself a root
     candidate — it only gates `require_vcs_root`) to pick the directory to
     scan. Marker certainty = declared weight × context class; content
     checks disambiguate shared files (`package.json` TS vs JS via a
     `typescript`/`ts-node` dependency, `pyproject.toml` needs a
     project/poetry/build-system table, `DESCRIPTION` needs a `Package:`
     line). When upward discovery finds nothing (including a
     `require_vcs_root` rejection), the start folder's own markers still
     identify it
   - Refuses to scan from boundary directories (home, system dirs)
   - Wildcard markers (`*.csproj`) need a directory listing, so they are not
     looked for in directories with more than 500 entries (literal markers
     are still found by `stat`)

3. **ScanningEngine** (`src/detection/scanner/`) - File discovery
   - A single sequential, name-sorted walk of the scan root's immediate
     entries (deterministic; no parallel path; the walk never descends below
     the root — only root files and probed source paths can identify
     anything). Finding a project from inside it uses no depth:
     root discovery walks UP looking for marker files by name
   - Exact patterns that are relative paths (`src/main/java`,
     `src/main/kotlin`, `src/main/scala`) are probed with one `stat` each, so
     a shared build tool (Gradle/Maven) is classified by its source language
     even though the walk never reaches those files; such a
     directory on its own (a sources-only checkout) also makes its language
     a candidate
   - Root exact-name manifests are seeded first and don't count against the
     15-match cap; if the cap is hit, a second pass tops up matches sorted
     past the cap (`App.csproj` after many earlier-sorted files). Only the
     scan root's own ignore files apply
   - Exact-name root manifests (and the oversized-root check) are literal
     lookups that deliberately ignore .gitignore, so an ignored `go.work`
     or `Cargo.toml` still counts; the walk and the top-up honour it
   - Oversized roots (>500 entries) only get the root-level check; an
     adaptive timeout cuts long walks short (such results are not cached)
   - Uses `PatternProcessor` for efficient pattern matching

4. **Decision** (`engine.rs` + `IndicatorResolver`) — three arms:
   - **Uncontested**: exactly one language has a qualifying marker and no
     other language has root source files → that language wins directly;
     confidence = the marker's certainty. An empty scan does not discard a
     found marker
   - **Contested** (`arbitrate_contested`): ordered rules — an app-framework
     dependency (priority <= 2, only between contenders whose strongest
     non-build marker is within 0.1 of the leader's), then strongest
     non-BuildSystem marker certainty, then root source files (a file that
     is itself a marker name, e.g. `build.gradle.kts`, never counts as
     source), then strongest BuildSystem marker, then config order
   - **Markerless** (`IndicatorResolver`): only root source files (`*.ext`
     at the root, or a probed `src/main/*` directory) make a candidate; tool
     configs and version pins alone identify nothing. Confidence is the scan
     ratio. A confirmed framework dependency floors every path at 0.75

5. **ConfidenceScorer** (stateless) - the ratio score used by the markerless
   path and by a sources-only arbitration winner
   - Per-pattern importance (weight × context of the declaring root
     indicator; 0.5 neutral otherwise), best matched file per pattern
   - File weight = depth weight (1.0/0.7/0.4/…) × directory tier
     (Source 1.2 / Root 1.0 / Unknown 0.8 / Noise 0.1); root files score
     1.0, probed `src/main/*` paths below it — this is how the tiers shape
     the markerless ratio
   - `alternative_group` members count once (max, not sum), so lockfile
     format choices don't inflate the denominator

6. **Framework detection** (`src/detection/framework_detector.rs`) - Framework identification
   - Candidates scoped by ecosystem intersection with the winning indicator
   - Ecosystem-specific matchers (`src/detection/matchers/ecosystems/`)
   - Dependency analysis from package.json, Cargo.toml, etc.

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
  root discovery consulted (bounded by `MAX_UPWARD_TRAVERSAL`),
  the watched directories (`watched_dirs`: the nearest existing parent of
  each probed source dir, plus the container scan's entries; a result
  watching more than 256 is not stored), and the `.gitignore`/`.ignore`/`.git/info/exclude`
  files (and the `.git/info` directory, never `.git` itself: every git command
  touches its mtime) that shaped the walk, every file read through the parse cache — matchers
  and root-discovery candidates, rejected ones included (all file reads go
  through it, which is what keeps this list complete); lockfiles no
  indicator pattern lists, the nearest existing parent of every
  `FileExists` probe path, and — with `require_vcs_root` — the ancestors up
  to the traversal bound; any mismatch is a miss. A result is not stored when a probed path is a dangling
  symlink (its target appearing moves no watched timestamp). A result is not stored if
  anything it depends on changed after detection started (or within 20 ms
  before it: file timestamps can come from a coarser clock; a whole-second
  timestamp betrays a 1–2 s filesystem (FAT/exFAT, ext3, HFS+, some NFS), so
  one under 4 s old is not trusted either)
- **Failure policy**: every error degrades silently to a fresh detection
  (`log::debug` only) — the cache must never break a prompt
- **Bypass**: `--no-cache`. (`--format debug` served from a cache hit shows the original
  run's `total_scan_time_ms` and `files_matched`; pass `--no-cache` for
  fresh diagnostics)
- **Housekeeping**: entries are 0600, written via a temp file + rename; a
  hit older than 24 h touches the entry's mtime; GC after a store removes
  temp files older than 10 minutes and, only above 512 entries, entries
  older than 30 days and then the oldest beyond 512. The entry also matches
  on the canonical home and the active config path, so a new higher-priority
  config file is a miss

The remaining caches live and die within a single detection run:

#### 1. FileSystemCache (Per-detection run)

- **Location**: `src/performance.rs`
- **Purpose**: Memoize file existence checks so root discovery and
  the framework detector don't repeat `stat` calls

#### 2. ParsedFileCache (Per-detection run)

- **Location**: `src/detection/caches/parsed_file.rs`
- **Purpose**: Cache parsed JSON/TOML content so manifests like `package.json`
  are parsed at most once per run

(Pattern matching itself is NOT cached: a DashMap memo was benchmarked and
removed — a cache hit cost ~6× more than computing the match, and removal cut
end-to-end detection ~70%. Don't reintroduce it without new numbers.)

### Configuration System

**One built-in configuration** (`built_in_config()`): one data row per language (`src/config/templates/languages.rs`) plus a
single ecosystem-keyed framework catalog (`src/config/templates/frameworks.rs`)

- Each indicator row defines file patterns, root indicators, and the
  ecosystems it participates in
- Frameworks are defined ONCE in the catalog, tagged with their ecosystems;
  an indicator surfaces every framework whose ecosystems intersect its own
- Users can customize via `~/.config/project-indicator/config.toml`
  (`[display]`, `[detection]`, `[[indicators]]`, `[[frameworks]]`; the schema is
  enforced structurally, there is no version field)
- Parsing is strict (`deny_unknown_fields`): removed or misspelled keys fail
  to parse instead of being silently ignored. At prompt time a failing config
  soft-falls-back to the built-in configuration (never breaks the shell);
  `config validate` surfaces the exact error and `config init --force`
  regenerates. `[frameworks.detection]` holds exactly one key naming the
  method (`dependencies = [...]` or `files = [...]`)

**Key Configuration Types**:

- `DetectionConfig`: VCS-root requirement
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
- **Memory limits**: JSON/TOML manifests are read up to 32 MiB and parsed
  through streaming filters (JSON keeps only the keys matchers read, TOML
  only the tables they read, both under a size budget), so parsing memory does
  not grow with the file. Line-oriented manifests (requirements, go.mod, poms)
  are held as text and capped at 1 MiB. Size is checked before any bytes are
  read, and one run reads at most 64 MiB in total. Text is kept for the run, so memory is
  bounded by that budget, not by the parsed size. A refused file is remembered
  and stays an input of the cached result; a failed read is remembered too and
  keeps the result out of the cache
- **Untrusted repositories**: a cloned repo chooses what sits at manifest and
  ignore-file paths. Manifests are read only if they are regular files (a
  FIFO blocks forever, a symlink to `/dev/zero` never ends — on unix the
  check is made on an already-open non-blocking handle, so a FIFO swapped in
  after a stat cannot block either) and only up to
  the per-class limits above; the scan root's `.gitignore`/`.ignore`/`.git/info/exclude` are
  honoured only if they are plain files under 1 MiB. A result computed
  without a manifest that failed to read is never cached. Icons and colors
  from a config, and file names in the debug dump, have control characters
  stripped before they reach the terminal. (The walk never descends below
  the scan root, so no nested ignore file is ever read.)

### 5. Testing Requirements

- Property-based tests for invariants (use `proptest`)
- Edge case coverage (symlinks, large projects, empty directories)
- All new features require tests

## Adding New Indicator or Framework Support

**Indicators** (project types — languages, runtimes, toolchains):

1. Add one row to `language_indicators()` in `src/config/templates/languages.rs`
   (row order is the detection tie-breaker — earlier wins, insert deliberately):

```rust
// name, files (sources and tool configs — filename-shaped root patterns are
// derived into the scan set by the engine), color, nerd-font hex, ecosystems, roots
indicator("MyLang", &["*.ml"], "#COLOR", "xxxx",
    &[Npm], &[root("mylang.toml", 0.95, LanguageRoot)]),
```

**Frameworks** (defined once, ecosystem-keyed):

1. Add one `dep(...)`/`file_fw(...)` row to `framework_catalog()` in
   `src/config/templates/frameworks.rs`:

```rust
dep(
    "MyFramework",
    &[Npm],            // scoping: surfaced by indicators sharing an ecosystem
    &["my-framework"],
    None,              // nerd-font code point in hex, e.g. Some("e7ba")
    Some("#COLOR"),
    PRIORITY,
), // file_fw(...) detects by file presence instead
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

1. Add a `check_*_ecosystem` fn to `src/detection/matchers/ecosystems/mod.rs`. Most are
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
  spawn (~3.3ms median measured; a bare process spawn is ~2.4ms on the same
  machine, so the lookup itself is microseconds)
- Cold detection: < 10ms wall including spawn (~3.8ms median measured;
  in-process detection is ~0.2ms for a small project, per the criterion
  benchmarks, and grows ~2µs per non-matching file in the scan root)
- Worst case (deep nesting): < 50ms

## Project Structure Notes

- `src/cli.rs`: CLI argument parsing (uses clap)
- `src/main.rs`: Entry point, command routing
- `src/lib.rs`: Library interface for external use
- `src/constants.rs`: Centralized manifest/lockfile name constants
- `src/patterns.rs`: Pattern matching utilities
- `src/commands/`: CLI command implementations (detect, config, cache)

## Deliberately Not Built

Decided against, with reasons — don't re-propose without new evidence:

- **Config-only framework detection** (`next.config.js`, `angular.json` alone):
  a framework config without its dependency in a manifest is a broken or
  manifest-less folder; the dependency path covers real projects
- **`setup.py`/`setup.cfg` parsing, `requirements/*.txt` layouts, `-r`
  includes**: arbitrary code / unbounded I/O for a shrinking tail; modern
  Python declares deps in `pyproject.toml` or a root `requirements.txt`
- **Direct-vs-transitive disambiguation in `Cargo.lock`**: impossible from
  the lockfile; it only runs for manifests that declare nothing
- **Prefix-blocking boundary directories**: a project genuinely under
  `/Applications` or a `/System` subpath should still detect
- **Detecting below a VCS-less root under `require_vcs_root`**: would hollow
  out the gate the option exists for

## Release Process

Automated via GitHub Actions:

- The PR's semver label picks the bump: `major`, `minor`, `patch`, or
  `no-release`. Titles are not read; an unlabeled merge or a direct push to
  `main` defaults to a patch release
- Merge to main triggers auto-release with cross-platform binaries
- Binaries built for: Linux x86_64/ARM64, macOS Intel/Apple Silicon, Windows

## Security Considerations

- **Editor**: `config edit` spawns `$EDITOR` directly (never via a shell), splitting arguments
- **Path traversal protection**: bounded upward traversal (`MAX_UPWARD_TRAVERSAL`), boundary checks
- **No file modifications**: Tool is read-only
- **File size**: per-class read limits and a per-run budget (see Memory limits)
- **Symlink safety**: directory symlinks are never followed (no loops possible); symlinked files count as files
