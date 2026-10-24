# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Key Features

- 🔒 **Read-only & safe by construction**: never modifies project files, never runs a shell, no `unsafe` code
- 🧪 **Property-Based Testing**: Extensive test suite including rigorous property-based testing with proptest
- 🔗 **Symlink Handling**: Comprehensive edge case handling for Unix symlinks

## Features

- 🔍 **Multi-language Detection**: Supports 26 languages and toolchains (Rust, JavaScript/TypeScript, Python, Go, Deno, Bun, Gleam, Terraform, Nix, and more)
- 🏗️ **Framework Recognition**: Detects 40 popular frameworks like React, Svelte, Axum, Django, Gin, Spring Boot, Laravel, Rails, Fresh
- ⚡ **Blazing Performance**: 3-5ms typical detection
- 🎨 **Multiple Output Formats**: Simple, Full, JSON, and Debug formats
- 🔧 **CLI**: Configuration and cache management, plus a full-evidence debug format
- 🧠 **Advanced Detection**: Confidence-based scoring with weighted root indicators
- 🔧 **Configuration Templates**: Pre-built templates for different development environments
- 🐚 **Shell Integration**: Ready-to-use hooks for Bash, Zsh, Fish and PowerShell (plus Starship and tmux snippets)

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

**Prerequisites**: Rust 1.88+ (install from [rustup.rs](https://rustup.rs/))

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
project-indicator --format debug
```

## CLI Reference

### Usage

```bash
project-indicator [OPTIONS] [PATH]
```

**Arguments:**
- `PATH` - Directory to analyze (defaults to current directory)

**Options:**
- `--format <FORMAT>` - Output format: simple (default), full, json, debug
- `--no-cache` - Skip the persistent result cache for this invocation
- `--max-depth <N>` - Maximum scan depth (default: 1)
- `-v, --verbose` - Enable verbose logging

### Subcommands

**Configuration Management:**
```bash
project-indicator config init [--template NAME] [--force] [--path PATH]
project-indicator config validate
project-indicator config edit
project-indicator config show
```

**Debugging:**
```bash
project-indicator --format debug   # full evidence and confidence breakdown
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

Run `cargo bench` (or `hyperfine project-indicator`) to measure on your machine.

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

# Detection settings
[detection]
max_upward_traversal = 10
require_vcs_root = false
max_depth = 1


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
1. `./project-indicator.toml` (current directory)
2. `./.project-indicator.toml` (current directory)
3. `$XDG_CONFIG_HOME/project-indicator/config.toml` (defaults to `~/.config`)
4. `$XDG_CONFIG_HOME/project-indicator.toml`
5. `$HOME/.project-indicator.toml`

A config file in the current directory shadows the global one — if your
global edits seem ignored, check for a local file first.

Parsing is strict: unknown or removed keys make the config fail to parse.
A failing config never breaks your prompt — detection falls back to the
built-in template — but customizations are ignored until the file is fixed.
Run `project-indicator config validate` to see the exact error, or
`project-indicator config init --force` to regenerate.


## Shell Integration

Thin prompt hooks for **fish, zsh, bash and PowerShell** live in
[`shell-integration/`](shell-integration/README.md) (plus Starship and tmux
snippets). Each hook just calls `project-indicator` and prints its output;
caching is done by the binary itself.

```bash
shell-integration/install.sh fish   # or zsh | bash
```

PowerShell: dot-source `shell-integration/project-indicator.ps1` from your
`$PROFILE`. See the [integration README](shell-integration/README.md) for
details.

## Supported Languages & Frameworks

26 languages and 40 frameworks in the built-in `full` template. A framework is
shown for every language that shares one of its package ecosystems (the
JavaScript-family languages share the npm framework set).

| Language   | Frameworks Detected |
|------------|---------------------|
| Bun        | React, Vue, Angular, Svelte, SolidJS, Hono, Next.js, Astro, NestJS, Vite |
| C#         | ASP.NET Core, Xamarin |
| C++        | — |
| Dart       | Flutter |
| Deno       | Fresh, Oak, Hono, plus the npm set (React, Vue, Svelte, …) |
| Elixir     | Phoenix |
| Gleam      | — |
| Go         | Gin, Echo, Fiber |
| Java       | Spring Boot |
| JavaScript | React, Vue, Angular, Svelte, SolidJS, Hono, Next.js, Astro, NestJS, Vite |
| Julia      | — |
| Kotlin     | Spring Boot, Ktor, Android |
| Lua        | LÖVE |
| Nix        | — |
| Odin       | — |
| PHP        | Laravel, Symfony, CodeIgniter, Yii |
| Python     | Django, Flask, FastAPI, Tornado, Pyramid, Sanic |
| R          | — |
| Ruby       | Rails |
| Rust       | Axum, Actix Web, Rocket, Tauri |
| Scala      | Akka HTTP |
| Swift      | Vapor |
| Terraform  | — |
| TypeScript | React, Vue, Angular, Svelte, SolidJS, Hono, Next.js, Astro, NestJS, Vite |
| V          | — |
| Zig        | — |

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
  `swiftpm`, `deno`)
- `FileExists` - file/directory presence

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
- **Pattern matching**: `patterns::matches_pattern`, a plain function (no cache — a memo was benchmarked and removed)
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

`$EDITOR` may include arguments (`EDITOR="code --wait"`). The command is
spawned directly, never through a shell, so shell metacharacters have no
effect — but the binary it names is run as-is, so:

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
- Upward traversal is bounded (`max_upward_traversal`: default 10, built-in templates use 2–5)
- Boundary directory checks prevent scanning from system directories (`/`, `/home`, `/root`, `/System`)
- Directory symlinks are never followed (the walker doesn't follow links, so loops can't occur); symlinked files count as files

### File System Access

- Read-only operations - the tool never modifies project files
- Respects file system permissions - gracefully handles unreadable files
- Files over 1MB are read but not kept in the per-run parse cache
- Cache files stored in standard user cache directories (`$XDG_CACHE_HOME/project-indicator/results/`, default `~/.cache/...`)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by shell-based project detection tools
- Built with ❤️ in Rust
- Performance optimized for daily developer use
- Community contributions welcome
