# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**project-indicator** is a high-performance Rust CLI tool for detecting project types and frameworks. It's designed to be integrated into shell prompts, achieving ~3µs detection times through aggressive caching and optimization.

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
cargo bench --bench pattern_matching_benchmark
cargo bench --bench parsed_file_cache_benchmark
cargo bench --bench filesystem_cache_benchmark
cargo bench --bench framework_detection_benchmark

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
cargo run -- --format debug
cargo run -- --format rich

# Performance modes
cargo run -- --mode fast     # Early termination
cargo run -- --mode thorough # Full scan (default)

# Debugging
cargo run -- debug --verbose
cargo run -- benchmark

# Configuration
cargo run -- config validate
cargo run -- config show
cargo run -- config init --template rust-dev

# Root indicator analysis
cargo run -- root-indicators conflicts --detailed
cargo run -- root-indicators list
cargo run -- root-indicators validate
```

## Architecture Overview

### Core Detection Pipeline

The detection flow follows this path:

1. **DetectionEngine** (`src/detection/engine.rs`) - Orchestrator
   - Creates and coordinates all specialized components
   - Shares a single `Arc<PatternMatcher>` across components for cache efficiency

2. **RootIndicatorEngine** - Fast path detection
   - Checks for high-confidence root files (`.git`, `Cargo.toml`, etc.)
   - Early returns if confidence threshold met

3. **ScanningEngine** (`src/detection/scanner/`) - File discovery
   - Traverses directory with configurable depth
   - Uses `PatternProcessor` for efficient pattern matching
   - Adaptive performance (different strategies for small/large projects)

4. **LanguageResolver** - Conflict resolution
   - Handles multi-language projects (e.g., TypeScript + Rust)
   - Uses priority and file count for resolution

5. **ConfidenceScorer** - Score calculation
   - Depth-based scoring (root files weighted highest)
   - Directory type multipliers (src/ weighted higher than node_modules/)
   - Pattern importance factors (package.json > *.js)
   - Root indicator bonus

6. **FrameworkDetector** - Framework identification
   - Ecosystem-specific matchers (`src/detection/matchers/ecosystems/`)
   - Dependency analysis from package.json, Cargo.toml, etc.

7. **OutputFormatter** (`src/output/`) - Result rendering
   - Multiple formats: simple, full, json, compact, debug, rich
   - Renderer pattern for extensibility

### Caching Architecture

All caches live and die within a single detection run (one CLI invocation);
there is no cross-invocation persistence.

#### 1. FileSystemCacheManager (Per-detection run)

- **Location**: `src/detection/caches/file_system.rs` + `src/performance.rs`
- **Purpose**: Memoize file existence checks and metadata (size, modified time)
  so the fast path, scanner, and framework detector don't repeat `stat` calls

#### 2. ParsedFileCache (Per-detection run)

- **Location**: `src/detection/caches/parsed_file.rs`
- **Purpose**: Cache parsed JSON/TOML content so manifests like `package.json`
  are parsed at most once per run

#### 3. PatternMatcher Cache (Shared)

- **Location**: `src/detection/pattern_matching.rs`
- **Shared**: Single instance via `Arc<PatternMatcher>`
- **Purpose**: Memoize glob pattern matching results
- **Thread-safe**: Uses DashMap for concurrent access

### Configuration System

**Template-based**: 19 language-specific templates (`src/config/templates/`)

- Each language module (e.g., `rust.rs`, `python.rs`) defines patterns and frameworks
- Templates combine languages (e.g., `web-dev` = JS + TS + frameworks)
- Users can customize via `~/.config/project-indicator/config.toml`

**Key Configuration Types**:

- `DetectionConfig`: Scan depth, thresholds, root indicators
- `DisplayConfig`: Output formatting preferences
- `CacheConfig`: TTL, max entries, eviction settings

### Type System

All types in `src/types/` organized by domain:

- `config.rs`: Configuration structures
- `detection.rs`: Detection results and evidence
- `framework.rs`: Framework detection types
- `indicators.rs`: Language and root indicators
- `matched_file.rs`: File matching results

## Critical Implementation Rules

### 1. NO UNWRAP/EXPECT

- **Clippy lints enforce**: `unwrap_used = "deny"`, `expect_used = "deny"`
- Use `?` operator, `.ok_or()`, or `match` statements
- All tests must also avoid unwrap/expect

### 2. Shared PatternMatcher Pattern

When adding new components that need pattern matching:

```rust
// In DetectionEngine::new() or similar
let shared_pattern_matcher = Arc::new(PatternMatcher::new());

// Share with components
let component = Component::new(shared_pattern_matcher.clone());
```

This ensures cache hits across the entire pipeline.

### 3. Error Handling

- Use `anyhow::Result<T>` for functions that can fail
- Use `thiserror` for custom error types
- Always provide context: `.context("Failed to parse config")?`

### 4. Performance Considerations

- **Cache key design**: Use `PathBuf` with `Borrow<Path>` trait to avoid allocations
- **Early termination**: Check confidence scores; stop scanning when threshold met
- **Memory limits**: File size limits prevent memory exhaustion (1MB per file)

### 5. Testing Requirements

- Property-based tests for invariants (use `proptest`)
- Concurrent stress tests for cache implementations
- Edge case coverage (symlinks, large projects, empty directories)
- All new features require tests

## Adding New Language Support

1. Create template file: `src/config/templates/<language>.rs`
2. Define language patterns and frameworks:

```rust
pub fn create_language_language(builder: &mut ConfigBuilder) -> ProjectIndicator {
    builder.add_language(
        "Language",
        vec!["*.ext", "manifest.json"],
        "#COLOR",
        "ICON",
        PRIORITY,
        vec![builder.create_framework(
            "Framework",
            DetectionType::FileExists {
                files: vec!["framework.config"],
            },
            "ICON",
            "#COLOR",
            PRIORITY,
        )],
    )
}
```

3. Register in `src/config/templates/mod.rs`
4. Add ecosystem matcher if needed: `src/detection/matchers/ecosystems/<language>.rs`
5. Update tests and documentation

## Adding New Output Format

1. Create renderer: `src/output/render.rs`

```rust
pub struct MyRenderer;
impl Render for MyRenderer {
    fn render(&self, result: &DetectionResult, config: &DisplayConfig) -> String {
        // Implementation
    }
}
```

2. Add to `OutputFormat` enum: `src/output/formatters.rs`
3. Update CLI help and documentation

## Adding New Detection Strategy

For ecosystem-specific dependency matching:

1. Create matcher: `src/detection/matchers/ecosystems/<ecosystem>.rs`
2. Implement parsing logic (use `ParsedFileCache` for performance)
3. Add to `DependencyMatcher` dispatch: `src/detection/matchers/dependency_matcher.rs`
4. Add confidence calculation helpers to `src/detection/matchers/common.rs`

## Performance Optimization Guidelines

### When optimizing

1. **Benchmark first**: Use `cargo bench` to establish baseline
2. **Profile**: Run with `cargo bench --bench profiling_benchmark`
3. **Cache aggressively**: But measure cache overhead vs benefit
4. **Prefer Arc over Clone**: For shared state across threads
5. **Use DashMap**: For concurrent HashMap needs
6. **Document complexity**: Add Big-O notation comments for algorithms

### Performance targets

- Shell prompt scenario: < 10µs (currently ~3µs)
- Typical detection: < 10ms (currently 3-5ms)
- Worst case (deep nesting): < 50ms (currently 20-30ms)

## Project Structure Notes

- `src/cli.rs`: CLI argument parsing (uses clap)
- `src/main.rs`: Entry point, command routing
- `src/lib.rs`: Library interface for external use
- `src/constants.rs`: Centralized file/extension constants
- `src/patterns.rs`: Pattern matching utilities
- `src/commands/`: CLI command implementations (detect, config, debug, benchmark, root-indicators)

## Release Process

Automated via GitHub Actions:

- PRs require semver label: `major`, `minor`, `patch`, or `no-release`
- Or use title prefix: `feat:`, `fix:`, `breaking:`, `docs:`
- Merge to main triggers auto-release with cross-platform binaries
- Binaries built for: Linux x86_64/ARM64, macOS Intel/Apple Silicon, Windows

## Security Considerations

- **Editor validation**: `config edit` validates `$EDITOR` environment variable
- **Path traversal protection**: Max 10 level upward traversal, boundary checks
- **No file modifications**: Tool is read-only
- **File size limits**: 1MB per file prevents memory exhaustion
- **Symlink safety**: Loop detection for circular symlinks
