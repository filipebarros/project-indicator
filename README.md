# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Key Features

- ✨ **Rich Output Format**: Detailed table format for comprehensive project information
- 🔒 **Enhanced Security**: EDITOR validation to prevent shell injection attacks
- ⚙️ **Configurable Thresholds**: Fine-tune detection with configurable performance thresholds
- 🧪 **Property-Based Testing**: Extensive test suite including rigorous property-based testing with proptest
- 🔗 **Symlink Handling**: Comprehensive edge case handling for Unix symlinks

## Features

- 🔍 **Multi-language Detection**: Supports 23 languages and toolchains (Rust, JavaScript/TypeScript, Python, Go, Deno, Bun, Terraform, Nix, and more)
- 🏗️ **Framework Recognition**: Detects 37 popular frameworks like React, Svelte, Axum, Django, Gin, Spring Boot, Laravel, Rails
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
project-indicator root-indicators list [--indicator NAME] [--framework NAME] [--conflicts-only]
project-indicator root-indicators validate [--strict] [--suggest]
project-indicator root-indicators stats
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
{"indicator":"TypeScript","frameworks":["React"],"icon":"","color":"#61dafb","confidence":0.85,"evidence":["package.json"]}

# Rich format (detailed table view)
$ project-indicator --format rich
╭────────────────────────────────────╮
│ Project Detection Results         │
├────────────────────────────────────┤
│ Project:     TypeScript           │
│ Framework:   React                │
│ Confidence:  95%                  │
╰────────────────────────────────────╯
```

## Performance

Detection results are cached persistently across invocations, so shell
prompt redraws cost almost nothing:

- Warm (cached) invocation: ~0.2ms of work — total wall time is dominated
  by OS process spawn
- Fresh detection (first visit or after a change): typically 3-5ms
- Worst case (deep nesting, cold): ~20-30ms

The cache lives under `$XDG_CACHE_HOME/project-indicator/` (default
`~/.cache`) and invalidates automatically when the project root, the
manifests that produced the result, your config file, or the binary version
change. Changes deep in subdirectories are only picked up when something at
the project root changes — use `--no-cache` or `cache clear` if you hit
that edge.

```bash
project-indicator cache stats   # entry count and disk usage
project-indicator cache clear   # start fresh
project-indicator --no-cache .  # bypass for one invocation
```

Run `project-indicator benchmark` to measure on your machine.

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
version = "3.0"

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

# Custom indicator (project type). `ecosystems` links it to the framework
# catalog: frameworks sharing an ecosystem are detected for this indicator.
[[indicators]]
name = "My Language"
files = ["*.mylang", "mylang.config"]
color = "#FF6B6B"
icon = "M"
priority = 1
ecosystems = ["npm"]

# Custom framework — defined once, surfaced by every indicator that shares
# one of its ecosystems
[[frameworks]]
name = "My Framework"
ecosystems = ["npm"]
color = "#4ECDC4"
priority = 1

[frameworks.detection]
type = "Dependencies"
dependencies = ["my-framework"]
```

### Configuration Locations

Priority order (first match wins):
1. `$XDG_CONFIG_HOME/project-indicator/config.toml`
2. `$HOME/.config/project-indicator/config.toml`
3. Windows: `%APPDATA%\project-indicator\config.toml`
4. `./project-indicator.toml` (current directory)


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
ecosystems = ["npm"]
icon = "⚛️"
color = "#61DAFB"
priority = 1

[frameworks.detection]
type = "Dependencies"
dependencies = ["react"]
```

**Detection Types:**
- `Dependencies` - dependency names checked in the manifests of the
  framework's ecosystems (`npm`, `pypi`, `cargo`, `go`, `packagist`,
  `rubygems`, `maven`, `gradle`, `nuget`, `sbt`, `pub`, `hex`, `luarocks`,
  `swiftpm`)
- `FileExists` - file/directory presence
- `ConfigFile` - config file contents

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

**Test Coverage**: extensive suite across multiple categories:
- Unit tests: Core functionality and edge cases
- Integration tests: End-to-end CLI behavior, persistent cache, fixture snapshots
- Property-based tests: Invariant validation with proptest
- Symlink handling tests: Unix symlink edge cases
- Concurrent stress tests: Thread safety verification

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
├── cache.rs            # Persistent cross-invocation result cache
├── commands/           # CLI command implementations
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
- **ParsedFileCache**: Per-run manifest parse memoization (JSON/TOML)
- **FileSystemCache**: Per-run metadata memoization
- **PersistentCache**: Cross-invocation result cache with mtime invalidation
- **ConfidenceScorer**: Calculates detection confidence with root indicators
- **FrameworkDetector**: Ecosystem-aware framework detection

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
