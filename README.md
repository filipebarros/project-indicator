# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Features

- 🔍 **Multi-language Detection**: Supports 10+ programming languages (Rust, JavaScript/TypeScript, Python, Go, Java, PHP, Ruby, and more)
- 🏗️ **Framework Recognition**: Detects popular frameworks like React, Next.js, Django, Flask, Gin, Spring Boot, Laravel, Rails
- ⚡ **Blazingly Fast**: Average detection time under 100μs with intelligent caching
- 🎨 **Customizable Output**: Multiple output formats (JSON, plain text, icons) with color themes
- 📁 **Smart Caching**: File modification-aware caching system for consistent performance
- 🔧 **Configurable**: Flexible TOML configuration with custom detection rules
- 🚀 **Shell Integration**: Easy integration with Fish, Zsh, Bash prompts

## Quick Start

### Installation

#### Download from Releases
```bash
# Download the latest release for your platform
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
```

#### Build from Source
```bash
git clone https://github.com/filipebarros/project-indicator.git
cd project-indicator
cargo install --path .
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
```

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

## Shell Integration

### Fish Shell

Add to your `~/.config/fish/config.fish`:

```fish
function fish_right_prompt
    set -l project_info (project-indicator 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        echo -n (set_color brblack)"["(set_color normal)$project_info(set_color brblack)"]"(set_color normal)
    end
end
```

### Zsh

Add to your `~/.zshrc`:

```zsh
# Right prompt with project indicator
RPS1='$(project_info=$(project-indicator 2>/dev/null); [[ -n "$project_info" ]] && echo "%F{8}[%f${project_info}%F{8}]%f")'
```

### Bash

Add to your `~/.bashrc`:

```bash
# Function to get project info
project_prompt() {
    local project_info=$(project-indicator 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo -e "\e[90m[\e[0m${project_info}\e[90m]\e[0m"
    fi
}

# Add to PS1
PS1='\u@\h:\w $(project_prompt)\$ '
```

## Performance

Project Indicator is designed for speed:

- **Cold start**: ~2.7ms (first run)
- **Cached**: ~63μs (subsequent runs)
- **Parallel processing**: Utilizes multiple CPU cores for large projects
- **Smart caching**: File modification tracking prevents stale results

Run benchmarks yourself:

```bash
project-indicator benchmark --iterations 1000
```

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

### Output Formats

- `default` - Human-readable with icons and colors
- `json` - Structured JSON for parsing
- `plain` - Plain text without formatting
- `minimal` - Just the essential info
- `verbose` - Detailed detection information

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Inspired by shell-based project detection tools
- Built with ❤️ in Rust
- Performance optimized for daily developer use