# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Key Features

- ✨ **Rich Output Format**: Detailed table format for comprehensive project information
- 📊 **Result Tracking**: Track detection history, compare snapshots, and analyze project evolution over time
- 🔒 **Enhanced Security**: EDITOR validation to prevent shell injection attacks
- ⚙️ **Configurable Thresholds**: Fine-tune detection with configurable performance thresholds
- 🧪 **Property-Based Testing**: Extensive test suite including rigorous property-based testing with proptest
- 🔗 **Symlink Handling**: Comprehensive edge case handling for Unix symlinks

## Features

- 🔍 **Multi-language Detection**: Supports 19 programming languages (Rust, JavaScript/TypeScript, Python, Go, Java, PHP, Ruby, and more)
- 🏗️ **Framework Recognition**: Detects 51+ popular frameworks like React, Next.js, Django, Flask, Gin, Spring Boot, Laravel, Rails
- ⚡ **Blazing Performance**: 3-5ms typical detection
- 🎨 **Multiple Output Formats**: Simple, Full, JSON, Compact, Debug, and Rich formats
- 🔧 **Comprehensive CLI**: Configuration management, debugging tools, and root indicator analysis
- 🧠 **Advanced Detection**: Confidence-based scoring with weighted root indicators and early termination
- 🔧 **Configuration Templates**: Pre-built templates for different development environments
- 🐚 **Shell Integration**: Ready-to-use integration scripts for Bash, Zsh, and Fish

## Quick Start

### Installation

#### Download from Releases (Recommended)

**Linux (x86_64)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**macOS (Apple Silicon)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-macos-aarch64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**Other platforms**: See [releases page](https://github.com/filipebarros/project-indicator/releases) for Linux ARM64, macOS Intel, and Windows builds.

#### Build from Source

**Prerequisites**: Rust 1.80+ (install from [rustup.rs](https://rustup.rs/))

```bash
git clone https://github.com/filipebarros/project-indicator.git
cd project-indicator
cargo install --path .
```

**Verification**:
```bash
project-indicator --version
project-indicator
```

### Basic Usage

```bash
# Detect current directory
project-indicator

# Detect specific directory
project-indicator /path/to/project

# JSON output for scripting
project-indicator --format json

# Different output formats
project-indicator --format full
project-indicator --format compact
project-indicator --format debug
project-indicator --format rich
```

## CLI Reference

### Usage

```bash
project-indicator [OPTIONS] [PATH]
```

**Arguments:**
- `PATH` - Directory to analyze (defaults to current directory)

**Options:**
- `--format <FORMAT>` - Output format: simple (default), full, json, compact, debug, rich
- `--max-depth <N>` - Maximum scan depth (default: 3)
- `--mode <MODE>` - Detection mode: thorough (default) or fast
- `-v, --verbose` - Enable verbose logging

### Subcommands

**Configuration Management:**
```bash
project-indicator config init [--template NAME] [--force] [--path PATH]
project-indicator config validate
project-indicator config edit
project-indicator config show
```

**Debugging & Performance:**
```bash
project-indicator debug [--verbose]
project-indicator benchmark
```

**Root Indicator Analysis:**
```bash
project-indicator root-indicators conflicts [--detailed] [--compare-legacy] [--show-strategies]
project-indicator root-indicators list [--language NAME] [--framework NAME] [--conflicts-only]
project-indicator root-indicators validate [--strict] [--suggest]
project-indicator root-indicators stats
```

**Result Tracking:**
```bash
project-indicator history [PATH] [-n LIMIT] [--changes-only]
project-indicator diff <FROM> [TO]
project-indicator stats [PATH] [--since TIME_RANGE]
```

## Output Examples

```bash
# TypeScript React project
$ project-indicator
󰛦 TypeScript ·  React

# Rust project
$ project-indicator
 Rust · Rocket

# Python Django project
$ project-indicator
 Python ·  Django

# JSON format
$ project-indicator --format json
{
  "language": {
    "name": "TypeScript",
    "icon": "󰛦",
    "color": "#3178C6"
  },
  "frameworks": [
    {
      "name": "React",
      "icon": "",
      "priority": 1
    }
  ],
  "confidence": 0.95
}

# Rich format (detailed table view)
$ project-indicator --format rich
╭────────────────────────────────────╮
│ Project Detection Results         │
├────────────────────────────────────┤
│ Language:    TypeScript           │
│ Framework:   React                │
│ Confidence:  95%                  │
╰────────────────────────────────────╯
```

## Performance

Detection is fast enough for real-time shell prompt integration:

- Typical detection: 3-5ms
- Best case (strong root indicators): ~1ms
- Worst case (deep nesting): ~20-30ms

Every invocation performs a fresh detection; within a run, file metadata,
parsed manifests, and pattern-match results are memoized to avoid repeated
work. Run `project-indicator benchmark` to measure on your machine.

## Configuration

### Quick Start with Templates

Initialize configuration with pre-built templates:

```bash
# Minimal setup (fastest)
project-indicator config init --template minimal

# Full features
project-indicator config init --template full

# Language-specific
project-indicator config init --template rust-dev
project-indicator config init --template web-dev
```

Available templates: `minimal`, `full`, `rust-dev`, `web-dev`, `python-dev`, `mobile-dev`, `data-science`, `enterprise`

### Configuration File

Create `~/.config/project-indicator/config.toml`:

```toml
# Metadata
[meta]
version = "2.0"

# Display settings
[display]
show_frameworks = true
max_frameworks = 2
framework_separator = "+"

# Detection settings
[detection]
max_upward_traversal = 10
require_vcs_root = false
confidence_threshold = 0.3
max_depth = 1
# Configurable performance thresholds
max_matches_per_pattern = 15    # Stop after N matches per pattern
small_project_threshold = 50    # Project size threshold for fast path
extreme_size_threshold = 500    # Large project threshold

# Detection mode: "fast" or "thorough"
[detection.mode]
mode = "thorough"

# Root indicators (optional - no defaults)
[[detection.root_indicators]]
pattern = ".git"
weight = 1.0
context = "VersionControl"

[[detection.root_indicators]]
pattern = "Cargo.toml"
weight = 0.9
context = "BuildSystem"

# Custom language
[[languages]]
name = "My Language"
files = ["*.mylang", "mylang.config"]
color = "#FF6B6B"
icon = "󰛦"
priority = 1

# Custom framework
[[frameworks]]
name = "My Framework"
detection = { type = "NodeEcosystem", dependencies = ["my-framework"] }
icon = ""
color = "#4ECDC4"
priority = 1
files = ["my-framework.json"]
```

### Configuration Locations

Priority order (first match wins):
1. `$XDG_CONFIG_HOME/project-indicator/config.toml`
2. `$HOME/.config/project-indicator/config.toml`
3. Windows: `%APPDATA%\project-indicator\config.toml`
4. `./project-indicator.toml` (current directory)

### Environment Variables

Override settings via environment:
```bash
export PROJECT_INDICATOR_CACHE_ENABLED=false
export PROJECT_INDICATOR_CACHE_TTL=600
export PROJECT_INDICATOR_MAX_ENTRIES=500
```

## Shell Integration

Project Indicator includes optimized shell integration scripts:

### Fish

```fish
# Install
curl -o ~/.config/fish/functions/project-indicator.fish \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.fish

# Add to right prompt (config.fish)
function fish_right_prompt
    fish_right_prompt_project_indicator
end

# Helpers
project_info                          # Show project info
project_indicator_clear_cache         # Clear cache
project_indicator_config ttl 600      # Set TTL
project_indicator_config status       # Show status
```

### Zsh

```zsh
# Install
curl -o ~/.project-indicator.zsh \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.zsh
echo 'source ~/.project-indicator.zsh' >> ~/.zshrc

# Add to right prompt
export RPS1='$(project_indicator_rprompt)'

# Helpers (same as Fish)
```

### Bash

```bash
# Install
curl -o ~/.project-indicator.bash \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.bash
echo 'source ~/.project-indicator.bash' >> ~/.bashrc

# Add to PS1
export PS1='\u@\h:\w$(project_indicator_prompt)\$ '

# Helpers (same as Fish)
```

## Supported Languages & Frameworks

| Language   | Icon | Frameworks Detected |
|------------|------|---------------------|
| Rust       |    | Rocket, Actix, Axum, Tauri |
| JavaScript |    | React, Next.js, Vue, Nuxt, Svelte, SvelteKit |
| TypeScript | 󰛦   | React, Next.js, Vue, Angular, NestJS |
| Python     |    | Django, Flask, FastAPI, Poetry |
| Go         |    | Gin, Echo, Fiber, Buffalo |
| Java       |    | Spring Boot, Maven, Gradle |
| PHP        |    | Laravel, Symfony, CodeIgniter |
| Ruby       |    | Rails, Sinatra, Jekyll |
| C#         |    | .NET, ASP.NET |
| Swift      |    | SwiftUI, Vapor |
| Kotlin     |    | Ktor, Spring |
| Scala      |    | Play, Akka |
| Elixir     |    | Phoenix |
| Dart       |    | Flutter |
| C++        |    | CMake, Conan |
| Zig        |    | Build system support |
| Lua        |    | LuaRocks, Love2D |
| Julia      |    | Package ecosystem |
| R          |    | Shiny, RMarkdown |

## Advanced Features

### Framework Detection

Frameworks are detected through multiple methods:

```toml
[[frameworks]]
name = "React"
detection = { type = "NodeEcosystem", dependencies = ["react"] }
files = ["src/App.jsx"]
icon = "⚛️"
color = "#61DAFB"
priority = 1
```

**Detection Types:**
- `NodeEcosystem` - npm/yarn dependencies (JavaScript/TypeScript)
- `PythonEcosystem` - pip/poetry dependencies
- `RustEcosystem` - Cargo dependencies
- `GoEcosystem` - Go module dependencies
- `PHPEcosystem` - Composer packages
- `RubyEcosystem` - Gemfile gems
- `JavaEcosystem` - Maven/Gradle dependencies
- `FileExists` - File/directory presence
- `ConfigFile` - Config file contents

### Root Indicator System

Root indicators improve project root detection with weighted scoring:

```toml
[[detection.root_indicators]]
pattern = ".git"              # Version control
weight = 1.0
context = "VersionControl"

[[detection.root_indicators]]
pattern = "Cargo.toml"        # Rust projects
weight = 0.9
context = "BuildSystem"

[[detection.root_indicators]]
pattern = "package.json"      # Node.js projects
weight = 0.9
context = "PackageManifest"
```

**Context Types:**
- `VersionControl` - .git, .hg, .svn
- `BuildSystem` - Cargo.toml, CMakeLists.txt, Makefile
- `PackageManifest` - package.json, pyproject.toml, go.mod
- `ProjectStructure` - src/, lib/, workspace markers
- `Configuration` - Config files and settings

**Note**: Root indicators must be explicitly defined in your config - there are no built-in defaults.

### Confidence Scoring

Detection uses weighted scoring based on:

- **Depth tiers**: Root (1.0) → Depth 1 (0.7) → Depth 2 (0.4) → Depth 3+ (0.1-0.05)
- **Directory multipliers**:
  - Source dirs (`src/`, `lib/`): 1.2
  - Config dirs (`.github/`, `config/`): 1.1
  - Test dirs: 0.2
  - Build output: 0.1
  - Dependencies: 0.05
- **Pattern importance**:
  - Core configs (package.json, Cargo.toml): 2.0
  - Build files (Makefile, tsconfig.json): 1.5
  - Source patterns (*.rs, *.ts): 1.0
- **Root indicator bonus**: Weighted contribution from root indicators

**Early termination heuristics:**
- Ultra-high confidence (≥2.0): Single important file at root
- High confidence (≥1.5): Multiple important files
- Medium confidence (≥1.0): Several moderate files
- Fallback: Stop after 15 files

Implementation: `src/types/matched_file.rs`, `src/detection/confidence_scorer.rs`, `src/detection/engine.rs`

### Result Tracking

Track detection results over time for debugging, performance analysis, and project evolution monitoring.

**Enable Tracking:**

Add to `~/.config/project-indicator/config.toml`:

```toml
[tracking]
enabled = true
# storage_path = "/custom/path"  # Optional custom location
```

**Features:**
- 📊 Records every detection with full context and evidence
- 🔍 Change detection (language changes, framework additions/removals)
- ⚡ Performance tracking (duration, cache hit rates)
- 📈 Statistics aggregation (median/min/max durations, language frequencies)
- 🕒 Timeline analysis (first seen, last seen)
- ⚠️ Zero overhead when disabled (no I/O operations)

**Storage:**
- macOS/Linux: `~/.cache/project-indicator/snapshots/YYYY-MM-DD.jsonl`
- Windows: `%APPDATA%\Local\project-indicator\snapshots\YYYY-MM-DD.jsonl`
- Format: JSON Lines (one JSON object per line)

**View Detection History:**

```bash
# Recent detections for current directory
project-indicator history

# History for specific path
project-indicator history ~/my-project

# Show more results
project-indicator history -n 20

# Only show detections with changes
project-indicator history --changes-only
```

**Compare Snapshots:**

```bash
# Compare by snapshot IDs
project-indicator diff abc-123 def-456

# Compare latest two snapshots for a path
project-indicator diff ~/my-project

# Shows changes: language, frameworks, confidence, cache status, performance
```

**View Statistics:**

```bash
# Stats for current directory
project-indicator stats

# Stats for specific path
project-indicator stats ~/my-project

# Shows:
# - Total detections, cache hit rate
# - Performance metrics (median/min/max duration)
# - Language frequency distribution
# - Timeline (first/last seen)
```

**Snapshot Data:**

Each detection snapshot includes:
- Unique snapshot ID and timestamp
- Detected language and frameworks with confidence scores
- Sample matched files and root indicators
- Cache performance (detection from cache, hit/miss stats)
- Duration in microseconds, files scanned

**Change Detection:**

The system automatically detects:
- Language changes (e.g., JavaScript → TypeScript)
- Framework additions/removals
- Confidence score changes
- Cache status changes (cached ↔ fresh)
- Performance variations

**Use Cases:**
- **Debugging**: Understand why a project was detected differently
- **Performance Analysis**: Track detection speed over time
- **Cache Effectiveness**: Measure cache hit rates
- **Project Evolution**: See how projects change languages/frameworks
- **Testing**: Validate detection consistency

**Example Output:**

```bash
$ project-indicator history -n 3

History for: /Users/name/my-project

Time                 Language        Frameworks                     Duration   Source
────────────────────────────────────────────────────────────────────────────────────
2025-01-21 14:23:45  TypeScript      React, Next.js                 3.2ms      fresh
2025-01-21 14:20:12  TypeScript      React, Next.js                 0.8ms      cached
2025-01-21 13:15:30  JavaScript      React                          4.1ms      fresh

Total: 3 detections shown

$ project-indicator stats

📊 Statistics for: /Users/name/my-project
════════════════════════════════════════════════════════════

Detection Summary:
  Total detections:   42
  Cached detections:  38 (90.5%)
  Fresh detections:   4

Performance:
  Median duration:    1.2ms
  Min duration:       0.5ms
  Max duration:       8.3ms

Languages Detected:
  TypeScript      38 (90.5%)
  JavaScript      4 (9.5%)

Timeline:
  First seen: 2025-01-15 09:30:00
  Last seen:  2025-01-21 14:23:45
```

**Performance:**

The tracking system is highly optimized with minimal overhead:
- **~1.25µs overhead** per detection (total snapshot creation and recording)
- **Non-blocking I/O**: Background thread handles all file writes
- **Path cache**: 250x faster on cache hits (52ns vs 13µs for canonicalization)
- **Pre-serialized buffers**: 28% faster than direct serialization
- **Arc<str> for names**: 5-10% reduction in allocation overhead

Technical optimizations:
- Thread-safe path canonicalization cache using DashMap
- Pre-allocated 2KB serialization buffers
- Shared string references (Arc<str>) for language/framework names
- Channel-based background writer with batch flushing
- Daily file rotation with kept-open file handles

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench
```

### Testing

```bash
# All tests
cargo test

# Specific test
cargo test test_name

# With output
cargo test -- --nocapture

# Library tests only
cargo test --lib

# Property-based tests only
cargo test --test property_tests
```

**Test Coverage**: 368 tests passing across multiple categories:
- Unit tests: Core functionality and edge cases
- Integration tests: End-to-end CLI behavior
- Property-based tests: Invariant validation with proptest (6 tests, 100+ scenarios each)
- Symlink handling tests: Unix symlink edge cases
- Concurrent stress tests: Thread safety verification
- Cache behavior tests: Unified cache, upgrades, and eviction
- Tracking E2E tests: Complete workflow validation, change detection, performance tracking

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes with tests
4. Ensure CI passes: `cargo test && cargo clippy`
5. Submit a pull request with appropriate semver label

#### Semver Labels

PRs must include one of:
- `major` - Breaking changes
- `minor` - New features
- `patch` - Bug fixes
- `no-release` - No version bump

Or use title prefixes: `feat:`, `fix:`, `breaking:`, `docs:`

### Release Process

Releases are automated:
1. Create PR with semver label
2. Merge triggers auto-release
3. Cross-platform binaries built automatically
4. GitHub release created with artifacts

## Architecture

### Module Structure

```
src/
├── cli.rs              # CLI argument parsing
├── main.rs             # Entry point
├── lib.rs              # Library interface
├── config.rs           # Configuration types
├── performance.rs      # FileSystemCache
├── detection/
│   ├── engine.rs       # Main detection engine
│   ├── confidence_scorer.rs
│   ├── framework_detector.rs
│   ├── pattern_matching.rs
│   ├── root_indicators.rs
│   ├── caches/         # Cache implementations
│   └── scanner/        # File scanning
├── tracking/
│   ├── types.rs        # Snapshot data structures
│   ├── storage.rs      # JSON Lines persistence
│   ├── comparison.rs   # Diff and change detection
│   ├── formatting.rs   # Colored terminal output
│   └── utils.rs        # Shared utilities
├── commands/
│   ├── history.rs      # History command
│   ├── diff.rs         # Diff command
│   └── stats.rs        # Stats command
├── output/             # Output formatting
├── types/              # Core type definitions
└── config/
    ├── parser.rs       # Config parsing
    ├── validator.rs    # Config validation
    └── templates/      # Config templates
```

### Key Components

- **DetectionEngine**: Orchestrates detection workflow
- **PatternMatcher**: Thread-safe pattern matching with nested DashMap cache
- **ParsedFileCache**: Unified cache with progressive enhancement (None → RawContent → ParsedJson/ParsedToml)
  - Generic ParsedType trait for extensibility
  - Smart upgrades from raw to parsed on demand
  - O(1) statistics via atomic counters
- **FileSystemCache**: Metadata caching with TTL and eviction
- **ConfidenceScorer**: Calculates detection confidence with root indicators
- **FrameworkDetector**: Parallel framework detection
- **ResultTracker**: Optional detection history tracking with JSON Lines storage
  - Zero overhead when disabled
  - Snapshot comparison and change detection
  - Performance statistics aggregation

## Exit Codes

```
0  Success (project detected or no project found)
1  Error (invalid path, config error, other failures)
```

## Security Considerations

### Editor Configuration

When using `project-indicator config edit`, the tool respects your `$EDITOR` environment variable to open the configuration file. **Ensure this variable points to a trusted executable.**

**Recommended editors:**
- `vim`, `nvim` - Vi/Neovim
- `emacs` - GNU Emacs
- `nano` - Nano editor
- `code` - Visual Studio Code
- `subl` - Sublime Text

**Security notes:**
- The tool validates against shell injection attempts (`;`, `&`, `|` characters)
- Unknown editors will trigger a warning but are still allowed
- Always verify your `$EDITOR` variable: `echo $EDITOR`
- Avoid setting `$EDITOR` to shell scripts or untrusted binaries

**Example:**
```bash
# Check your current editor
echo $EDITOR

# Set a safe editor (add to ~/.bashrc or ~/.zshrc)
export EDITOR=vim
```

### Path Traversal Protection

The tool includes built-in safeguards against path traversal attacks:
- Maximum upward traversal limited to 10 directory levels
- Boundary directory checks prevent scanning from system directories (`/`, `/home`, `/root`, `/System`)
- Symbolic links are followed safely with loop detection

### File System Access

- Read-only operations - the tool never modifies project files
- Respects file system permissions - gracefully handles unreadable files
- File size limits prevent memory exhaustion (1MB cache limit per file)
- Cache files stored in standard user cache directories (`~/.cache/project-indicator-*`)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by shell-based project detection tools
- Built with ❤️ in Rust
- Performance optimized for daily developer use
- Community contributions welcome
