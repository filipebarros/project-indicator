# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Features

- 🔍 **Multi-language Detection**: Supports 18+ programming languages (Rust, JavaScript/TypeScript, Python, Go, Java, PHP, Ruby, and more)
- 🏗️ **Framework Recognition**: Detects 50+ popular frameworks like React, Next.js, Django, Flask, Gin, Spring Boot, Laravel, Rails
- ⚡ **Enterprise Performance**: 12ms detection time, 36μs cache hits (182x improvement), optimized with Arc<T>, parallel processing, and SIMD pattern matching
- 🎨 **Advanced Output**: 5 output formats (Simple, Full, JSON, Compact, Debug) with rich formatting
- 📁 **Intelligent Caching**: File modification-aware caching with early termination, pattern pre-compilation, and confidence score memoization
- 🔧 **Comprehensive CLI**: Full configuration management with validation, cache control, and debugging tools
- 📊 **Project Analytics**: Comprehensive insights with lines of code, dependency analysis, complexity metrics, and multi-format export (JSON, CSV, Markdown, HTML)
- 🧠 **Advanced Detection**: Sophisticated project classification with confidence scoring and hybrid detection modes
- 🔧 **Configuration Templates**: Pre-built templates for different development environments with improved ConfigBuilder pattern
- 🐚 **Shell Integration**: Ready-to-use integration scripts for Bash, Zsh, and Fish with optimized performance
- 🚀 **Shell Ready**: Optimized for shell prompt integration with <25μs average response time
- 🎯 **Root Indicator Integration**: Advanced project root detection with weighted indicators and confidence scoring
- 🔄 **Lazy Framework Detection**: Smart framework detection that only runs when confidence is high enough
- ⚡ **SIMD Optimizations**: Zero-allocation pattern matching with SIMD-like string operations

## Quick Start

### Installation

#### Download from Releases (Recommended)

**Linux (x86_64)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**Linux (ARM64)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-aarch64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**macOS (Intel)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-macos-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**macOS (Apple Silicon)**:
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-macos-aarch64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

**Windows**:
1. Download `project-indicator-windows-x86_64.zip` from the [releases page](https://github.com/filipebarros/project-indicator/releases)
2. Extract the ZIP file
3. Add the directory containing `project-indicator.exe` to your PATH

#### Build from Source
**Prerequisites**: Rust 1.80+ (install from [rustup.rs](https://rustup.rs/))

```bash
git clone https://github.com/filipebarros/project-indicator.git
cd project-indicator
cargo install --path .
```

**Verification**:
```bash
# Check version
project-indicator --version

# Test detection
project-indicator

# Run benchmark
project-indicator benchmark
```

### Basic Usage

```bash
# Detect current directory
project-indicator

# Detect specific directory
project-indicator /path/to/project

# JSON output for scripting
project-indicator --format json

# Disable caching for testing
project-indicator --no-cache
```

## CLI

### Usage

```bash
project-indicator [OPTIONS] [PATH]
```

- **PATH**: Directory to analyse. Defaults to current directory.

### Options

- `-f, --format <FORMAT>`: Output format. One of: `simple`, `full`, `json`, `compact`, `debug` (default: `simple`)
- `-c, --config <CONFIG>`: Path to config file
- `--no-cache`: Disable detection cache
- `--cache-stats`: Print cache statistics and exit
- `-h, --help`: Show help
- `-V, --version`: Show version

### Subcommands

**Configuration Management:**
- `config init [--template NAME] [--force] [--path PATH]` — Initialize configuration from template
- `config validate` — Validate the active configuration
- `config edit` — Show the resolved config path and open it with `$EDITOR` if set

**Detection & Performance:**
- `debug [--verbose]` — Run detection and print a detailed debug view
- `benchmark` — Run built-in performance benchmarks

**Cache Management:**
- `cache clear` — Clear the detection cache
- `cache stats` — Show detection cache statistics

**Analytics & Insights:**
- `analytics [--format detailed|summary] [--export FILE] [--export-format json|csv|markdown|html]` — Generate comprehensive project analytics and insights

## Output Examples

```bash
# TypeScript React project
$ project-indicator
󰛦 TypeScript · ⚛️ React

# Rust project
$ project-indicator
🦀 Rust · 🚀 Rocket

# Python Django project
$ project-indicator
🐍 Python · 🎸 Django

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
      "icon": "⚛️",
      "priority": 1
    }
  ],
  "confidence": 0.95
}

# Analytics and insights
$ project-indicator analytics
╭─ Project Analytics ──────────────────────────────────────────────╮
│ 📊 Path: /Users/dev/my-project                                   │
│ 🕒 Generated: 2024-01-15 14:30:22 UTC                           │
├───────────────────────────────────────────────────────────────────┤
│ 📁 File Statistics:                                              │
│   Total Files:       47                                          │
│   Total Lines:    2,340                                          │
│   Total Size:     89.2 KB                                        │
│   Empty Files:        3                                          │
│                                                                   │
│   Top File Types:                                                 │
│        .ts:    847 lines (36.2%)                                 │
│       .tsx:    445 lines (19.0%)                                 │
│      .json:    234 lines (10.0%)                                 │
├───────────────────────────────────────────────────────────────────┤
│ 🎯 Language Breakdown:                                           │
│  1: TypeScript   ████████████████████  85.2%                    │
│  2: JavaScript   ██████                14.8%                    │
├───────────────────────────────────────────────────────────────────┤
│ 📦 Dependencies:                                                  │
│   Total Dependencies:      23                                    │
│   Direct Dependencies:     15                                    │
│   Dev Dependencies:         8                                    │
│                                                                   │
│   Package Managers:                                               │
│     npm      :  23 dependencies                                  │
├───────────────────────────────────────────────────────────────────┤
│ 💡 Project Insights:                                             │
│   Complexity Score: 2.34 (Moderate)                             │
│   Dominant Language: TypeScript (85.2%)                         │
│   Avg Lines/File: 49.8                                          │
│   Characteristics: Web, Medium                                   │
╰───────────────────────────────────────────────────────────────────╯
```

## Performance

Project Indicator is built for speed with enterprise-grade optimizations:

```bash
$ project-indicator benchmark
Performance Benchmark
====================
1. Single Detection (Cold): 12.5ms
2. Single Detection (Warm): 5.7ms (2.2x improvement)  
3. Cache Hit: 36μs (154x improvement)
4. Shell Prompt Simulation: 21μs average (182x improvement)

Performance Recommendations:
✓ Cache performance is excellent (< 100μs)
✓ Detection speed is excellent (< 50ms)
```

**Key Optimizations:**
- **Arc<T> Smart Pointers**: Eliminated unnecessary cloning
- **Parallel Processing**: Multi-threaded file scanning with Rayon
- **Pattern Matching Cache**: Thread-safe caching for wildcard pattern matching
- **SIMD String Operations**: Zero-allocation extension and prefix matching
- **Early Termination**: Multi-tier heuristics stop scanning when confidence is high
- **Memory-Aware Caching**: File modification time tracking with hierarchical cache
- **Pattern Pre-computation**: Pre-filtering extensions and exact patterns
- **Confidence Score Memoization**: Cache language scores to avoid recalculation
- **Lazy Framework Detection**: Skip framework detection when language confidence is low
- **Root Indicator Integration**: Weighted root indicators improve project root detection
- **String Optimization**: COW (Copy-on-Write) strings and pattern caching
- **File System Cache**: Optimized metadata caching with TTL and eviction policies

## Shell Integration

Project Indicator includes ready-to-use shell integration scripts for a seamless development experience:

### Automatic Installation

```bash
# Run the installer to set up shell integration
./shell-integration/install.sh
```

### Manual Setup

**Bash (add to `~/.bashrc`):**
```bash
source "path/to/project-indicator/shell-integration/project-indicator.bash"
```

**Zsh (add to `~/.zshrc`):**
```bash
source "path/to/project-indicator/shell-integration/project-indicator.zsh"
```

**Fish (add to `~/.config/fish/config.fish`):**
```fish
source "path/to/project-indicator/shell-integration/project-indicator.fish"
```

### Configuration Templates

Initialize configuration quickly with pre-built templates using the improved ConfigBuilder pattern:

```bash
# Minimal setup (fastest)
project-indicator config init --template minimal

# Web development focused
project-indicator config init --template web-dev

# Rust development focused
project-indicator config init --template rust-dev

# Python development focused
project-indicator config init --template python-dev

# Enterprise features enabled
project-indicator config init --template enterprise

# All features enabled
project-indicator config init --template full
```

Available templates: `minimal`, `web-dev`, `rust-dev`, `python-dev`, `mobile-dev`, `enterprise`, `data-science`, `full`

**Note**: Configuration templates now use the improved ConfigBuilder pattern for better readability and maintainability. The old `create_config_base` function has been deprecated in favor of the fluent API.

## Configuration

Create a configuration file at `~/.config/project-indicator/config.toml`:

```toml
# Cache settings
[cache]
enabled = true
max_entries = 1000
ttl_seconds = 300

# Output theme
[theme]
name = "default"
separator = " · "

# Custom language detection
[[languages]]
name = "My Language"
files = ["*.mylang", "mylang.config"]
color = "#FF6B6B"
icon = "🎯"
priority = 1

# Custom framework detection
[[frameworks]]
name = "My Framework"
language = "JavaScript"
files = ["my-framework.json"]
dependencies = ["my-framework"]
color = "#4ECDC4"
icon = "🌟"
priority = 1
```

### Configuration file locations

Project Indicator resolves configuration from (first match wins):

1. `$XDG_CONFIG_HOME/project-indicator/config.toml`
2. `$HOME/.config/project-indicator/config.toml`
3. Windows: `%APPDATA%\project-indicator\config.toml`
4. `./project-indicator.toml` (current directory)

### Environment variables

Override select settings via environment variables:

```bash
export PROJECT_INDICATOR_CACHE_ENABLED=false
export PROJECT_INDICATOR_CACHE_TTL=600
export PROJECT_INDICATOR_MAX_ENTRIES=500
export PROJECT_INDICATOR_THEME=minimal
```

## Analytics & Insights

Project Indicator provides comprehensive project analytics beyond basic language detection:

### Usage

```bash
# Generate detailed analytics report
project-indicator analytics

# Generate summary analytics
project-indicator analytics --format summary

# Export analytics to file (auto-detects format from extension)
project-indicator analytics --export report.json
project-indicator analytics --export report.csv
project-indicator analytics --export report.md
project-indicator analytics --export report.html

# Specify export format explicitly
project-indicator analytics --export report.txt --export-format json
```

### Analytics Features

- **📁 File Statistics**: Total files, lines of code, file sizes, empty files
- **🎯 Language Analysis**: Breakdown by programming language with percentages and visual progress bars
- **📦 Dependency Analysis**: Total dependencies, direct vs dev dependencies, package manager detection
- **🔍 Project Insights**: Complexity scoring, dominant language identification, project characteristics
- **📊 File Type Breakdown**: Top file extensions with line counts and percentages
- **📈 Project Characteristics**: Automatic categorization (Web, Mobile, Systems, Data Science, etc.)

### Export Formats

1. **JSON** - Structured data for programmatic processing
2. **CSV** - Tabular data for spreadsheet analysis
3. **Markdown** - Human-readable reports with tables and progress bars
4. **HTML** - Rich web reports with embedded styling and interactivity

### Package Manager Support

Project Indicator analyzes dependencies from multiple package managers:

- **npm/yarn** (package.json) - JavaScript/TypeScript projects
- **Cargo** (Cargo.toml) - Rust projects
- **pip/poetry** (requirements.txt, pyproject.toml) - Python projects
- **Go modules** (go.mod) - Go projects
- **Composer** (composer.json) - PHP projects
- **Bundler** (Gemfile) - Ruby projects

## Shell Integration

Use the provided shell scripts for high-performance, cached prompt integration.

### Fish

1) Install script
```fish
curl -o ~/.config/fish/functions/project-indicator.fish \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.fish
```

2) Use in right prompt
```fish
functions -q fish_right_prompt_project_indicator; and functions -c fish_right_prompt fish_right_prompt_backup
function fish_right_prompt
    fish_right_prompt_project_indicator
end
```

3) Helpers
```fish
# Show info now
project_info
# Clear cache
project_indicator_clear_cache
# TTL/status
project_indicator_config ttl 600
project_indicator_config status
```

### Zsh

1) Install script
```zsh
curl -o ~/.project-indicator.zsh \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.zsh
echo 'source ~/.project-indicator.zsh' >> ~/.zshrc
```

2) Use in right prompt
```zsh
export RPS1='$(project_indicator_rprompt)'
```

3) Helpers
```zsh
# Show info now
project_info
# Clear cache
project_indicator_clear_cache
# TTL/status
project_indicator_config ttl 600
project_indicator_config status
```

### Bash

1) Install script
```bash
curl -o ~/.project-indicator.bash \
  https://raw.githubusercontent.com/filipebarros/project-indicator/main/shell-integration/project-indicator.bash
echo 'source ~/.project-indicator.bash' >> ~/.bashrc
```

2) Use in PS1
```bash
export PS1='\u@\h:\w$(project_indicator_prompt)\$ '
```

3) Helpers
```bash
# Show info now
project_info
# Clear cache
project_indicator_clear_cache
# TTL/status
project_indicator_config ttl 600
project_indicator_config status
```

### Advanced Shell Integration

#### Local Examples Setup
For advanced customization, you can use the local example files:
```bash
# Fish Shell
source /path/to/project-indicator/examples/shell-integration/fish.fish

# Zsh
source /path/to/project-indicator/examples/shell-integration/zsh.zsh

# Bash
source /path/to/project-indicator/examples/shell-integration/bash.bash
```

#### Performance Optimizations
- **Caching for better responsiveness** - Directory change detection with cached results
- **Minimal external calls** - Optimized to reduce shell startup time
- **Conditional loading** - Only loads if project-indicator is available

#### Testing Your Integration
Test your shell integration:
```bash
# All shells support
test_project_indicator_integration
```

#### Framework Support
The shell integrations work with popular shell frameworks:
- **Starship** integration
- **Oh-My-Zsh** compatibility
- **Powerlevel10k** segments
- **Powerline** support

#### Customization Examples

**Custom Colors:**
```bash
# Modify color codes in integration files
# \e[32m = green, \e[34m = blue, \e[33m = yellow
```

**Custom Format:**
```bash
my_project_prompt() {
    local info=$(project-indicator --format json 2>/dev/null)
    # Parse and format as needed
}
```

**Conditional Display:**
```bash
# Only show in specific directories
project_prompt_conditional() {
    [[ "$PWD" == "$HOME/code"* ]] && project_prompt
}
```

#### Performance Tips
1. **Use cached versions** - Cached prompt functions are recommended for daily use
2. **Avoid frequent calls** - Project info is cached per directory change
3. **JSON parsing** - Only use JSON output when you need structured data
4. **Error handling** - All examples include proper error handling

## Supported Languages & Frameworks

| Language   | Icon | Frameworks Detected |
|------------|------|-------------------|
| Rust       | 🦀   | Rocket, Actix, Axum, Tauri |
| JavaScript | 🟨   | React, Next.js, Vue, Nuxt, Svelte |
| TypeScript | 󰛦    | React, Next.js, Vue, Angular |
| Python     | 🐍   | Django, Flask, FastAPI, Poetry |
| Go         | 🐹   | Gin, Echo, Fiber, Buffalo |
| Java       | ☕   | Spring Boot, Maven, Gradle |
| PHP        | 🐘   | Laravel, Symfony, CodeIgniter |
| Ruby       | 💎   | Rails, Sinatra, Jekyll |
| C#         | 🔷   | .NET, ASP.NET |
| Swift      | 🍎   | SwiftUI, Vapor |

## Recent Improvements & Missing Features

### Performance Enhancements (v0.2.0+)

The latest version includes significant performance improvements and code quality enhancements:

#### 🚀 **Performance Optimizations**
- **Modular Architecture**: Refactored engine.rs from 2,035 lines into 5 focused modules (async_engine, batch_detector, confidence_scorer, file_scanner, language_resolver)
- **Pattern Pre-filtering**: Reduces unnecessary file scanning by pre-computing extension filters
- **SIMD Pattern Matching**: Zero-allocation string operations for extension and prefix matching
- **Confidence Score Memoization**: Caches language scores to avoid redundant calculations
- **Lazy Framework Detection**: Skips framework detection when language confidence is below threshold
- **Multi-tier Early Termination**: Ultra-high, high, and medium confidence termination heuristics
- **Thread-safe Pattern Cache**: Mutex-protected caching for wildcard pattern matching
- **String Optimization**: COW strings and pattern caching to reduce allocations
- **Async Detection Engine**: Parallel processing capabilities for large codebases
- **Batch Detection**: Optimized for processing multiple projects simultaneously

#### 🎯 **Root Indicator Integration**
- **Weighted Root Indicators**: Root indicators now contribute to confidence scoring
- **Framework-specific Indicators**: Proper separation of language vs framework root indicators
- **Mandatory File Detection**: Only truly mandatory files are used as root indicators
- **Confidence-based Detection**: Root indicators improve project root detection accuracy

#### 🔧 **Code Quality Improvements**
- **Template System Refactoring**: Modular template system with language-specific files and shared components
- **Builder Pattern**: Replaced functions with too many parameters with fluent ConfigBuilder API
- **Iterator Optimization**: Replaced `Iterator::last()` with `next_back()` for better performance
- **Comprehensive Testing**: All 343 tests passing with full coverage
- **Memory Safety**: Improved error handling and resource management
- **Documentation**: Enhanced inline documentation and examples

#### 📊 **Detection Accuracy**
- **Improved Language Detection**: Better separation of language and framework indicators
- **Enhanced Framework Detection**: More accurate framework identification
- **Smart Early Termination**: Stops scanning when confidence is sufficiently high
- **Root Discovery**: Better project root detection with weighted indicators

### 🚧 **Missing Features & Known Limitations**

#### **Not Yet Implemented**
- **Rich Output Module**: Advanced terminal formatting capabilities are partial
  - Missing: Rich terminal rendering implementation
  - Missing: Interactive output modes
  - Missing: Advanced color and styling features
- **Configuration Validator**: Incomplete validation for complex configurations
  - Missing: Cross-language framework validation
  - Missing: Template dependency validation
  - Missing: Runtime configuration reload
- **Analytics Export**: Limited export format support
  - Missing: Advanced HTML export with charts
  - Missing: PDF export capabilities
  - Missing: Custom template support for exports

#### **Performance & Scalability**
- **Large Monorepo Support**: Performance degrades with very large repositories (>100k files)
- **Memory Usage**: High memory consumption when processing large codebases simultaneously
- **Incremental Updates**: No support for incremental cache updates on file changes
- **Distributed Caching**: No support for shared caches across team members

#### **Integration & Ecosystem**
- **IDE Integration**: No VSCode extension or IDE plugins available
- **CI/CD Integration**: Limited GitHub Actions integration examples
- **Docker Support**: Missing official Docker images and Kubernetes manifests
- **Language Server**: No LSP implementation for real-time project analysis

#### **Developer Experience**
- **Debugging Tools**: Limited debugging and profiling tools for configuration issues
- **Migration Tools**: No tools for migrating from other project detection tools
- **Plugin System**: No plugin architecture for custom detection logic
- **Hot Reload**: Configuration changes require tool restart

#### **Platform Support**
- **Windows WSL**: Partial Windows Subsystem for Linux support
- **ARM Optimization**: Suboptimal performance on ARM architectures
- **Network Drives**: Poor performance on network-mounted filesystems

### 🎯 **Roadmap Priorities**

1. **Complete ML Module**: Implement missing ML classification features
2. **Performance Optimization**: Address large repository performance issues
3. **Rich Output**: Complete terminal formatting and interactive features
4. **IDE Integration**: Develop VSCode extension
5. **Plugin Architecture**: Design extensible plugin system

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

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes with tests
4. Ensure CI passes: `cargo test && cargo clippy`
5. Submit a pull request with appropriate semver label

#### SemVer Labels

PRs must include one of these labels:
- `major` - Breaking changes
- `minor` - New features
- `patch` - Bug fixes
- `no-release` - No version bump needed

Or use title prefixes: `feat:`, `fix:`, `breaking:`, `docs:`

### Release Process

Releases are fully automated:

1. **Development** → Create PR with appropriate semver label
2. **Review** → PR validation ensures semver compliance
3. **Merge** → Auto-release workflow creates tag and GitHub release
4. **Build** → Cross-platform binaries built and uploaded

## Configuration Reference

### Language Detection

Languages are detected by file patterns and specific files:

```toml
[[languages]]
name = "Rust"
files = ["Cargo.toml", "Cargo.lock", "*.rs"]
color = "#DEA584"
icon = "🦀"
priority = 1
```

### Framework Detection

Frameworks are detected through multiple methods:

```toml
[[frameworks]]
name = "React"
language = "JavaScript"
detection_type = "PackageJson"  # PackageJson, CargoToml, GoMod, PyProjectToml, GemSpec, ComposerJson
dependencies = ["react"]
files = ["src/App.jsx", "public/index.html"]
color = "#61DAFB"
icon = "⚛️"
priority = 1
```

### Detection Types

- `PackageJson` - npm/yarn dependencies (JavaScript/TypeScript)
- `CargoToml` - Cargo dependencies (Rust)
- `GoMod` - Go module dependencies
- `PyProjectToml` - Python project dependencies
- `GemSpec` - Ruby gem dependencies
- `ComposerJson` - PHP Composer dependencies
- `FileExists` - File/directory existence

### Project Root Detection

Configure how project roots are detected when working in subdirectories. Root indicators now contribute to confidence scoring and improve detection accuracy:

```toml
[detection]
# Maximum number of directories to traverse upward when looking for project root
max_upward_traversal = 10

# Require a version control system root (e.g., .git) to be considered a project root
require_vcs_root = false

# Minimum confidence threshold (0.0 - 1.0) to accept a path as project root
confidence_threshold = 0.3

# Root indicators for project root detection
# These now contribute to confidence scoring and improve detection accuracy
[[detection.root_indicators]]
pattern = ".git"             # Version control
weight = 1.0

[[detection.root_indicators]]
pattern = "Cargo.toml"       # Rust projects
weight = 0.9

[[detection.root_indicators]]
pattern = "package.json"     # Node.js projects
weight = 0.9

[[detection.root_indicators]]
pattern = "workspace.json"   # Nx/monorepo workspace file
weight = 0.8
```

**Note**: Root discovery only works if you explicitly define `root_indicators` in your configuration. There are no built-in defaults - you must specify which files/directories should be considered project root indicators.

**Improvements in v0.2.0+**:
- Root indicators now contribute to confidence scoring
- Framework-specific indicators are properly separated from language indicators
- Only truly mandatory files are used as root indicators
- Weighted indicators improve project root detection accuracy

### Output Formats

- `simple` — Human-readable summary (default)
- `full` — Language plus frameworks with details
- `json` — Structured JSON for scripting
- `compact` — Minimal, space-saving output (good for prompts)
- `debug` — Detailed diagnostic output

## Depth weighting

Project Indicator scores evidence files using depth and directory context to improve precision and stop early when confidence is high. The scoring system has been enhanced in v0.2.0+ with root indicator integration and improved early termination.

- **Depth tiers** (from `MatchedFile::calculate_weight`):
  - Depth 0 (root): 1.0
  - Depth 1: 0.7
  - Depth 2: 0.4
  - Depth 3: 0.1
  - Depth ≥4: 0.05

- **Directory multipliers** (from `DirectoryType::weight`):
  - Root: 1.0
  - Source (`src/`, `lib/`, `app/`): 1.2
  - Config (`.github/`, `config/`, `.config/`, etc.): 1.1
  - Documentation (`docs/`): 0.6
  - Examples (`examples/`): 0.3
  - Test (`test/`, `spec/`, `__tests__`, `fixtures/`): 0.2
  - Build (`dist/`, `build/`, `target/`): 0.1
  - Dependencies (`node_modules/`, `vendor/`): 0.05
  - Unknown: 0.8

- **Pattern importance** (from `MatchedFile::get_pattern_importance`):
  - Core configs (`package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod`, `pom.xml`, `build.gradle`): 2.0
  - Build/config files (`Makefile`, `tsconfig.json`, `vite.config.js`, etc.): 1.5
  - Source patterns like `*.rs`, `*.ts`, `*.py`: 1.0

- **Root indicator bonus** (new in v0.2.0+):
  - Root indicators contribute additional weight to confidence scoring
  - Framework-specific indicators are properly weighted
  - Mandatory files have higher impact on detection accuracy

- **Language confidence** combines the above by selecting the best-weighted file per pattern, multiplying by the pattern importance, adding root indicator bonuses, and normalising by the maximum possible for that language. This feeds conflict resolution and multi-tier early-termination logic.

- **Early termination heuristics** (enhanced in v0.2.0+):
  - Ultra-high confidence (≥2.0): Single important file at root depth
  - High confidence (≥1.5): Multiple important files or single file with high importance
  - Medium confidence (≥1.0): Several files with moderate importance
  - Fallback: Stop after 15 files to prevent excessive scanning

Implementation references: `src/types.rs` (`MatchedFile`, `DirectoryType`), `src/detection/engine.rs` (language scoring and early termination), and `src/detection/confidence_scorer.rs` (confidence calculation with root indicators).

## Exit codes

```text
0  Success (project detected or no project found)
1  Error (invalid path, invalid config, other failures)
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by shell-based project detection tools
- Built with ❤️ in Rust
- Performance optimized for daily developer use
