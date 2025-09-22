# Installation and Configuration Guide

This guide covers all installation methods and configuration options for project-indicator.

## Installation Methods

### 1. Download Pre-built Binaries (Recommended)

Download the latest release for your platform:

#### Linux (x86_64)
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

#### Linux (ARM64)
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-linux-aarch64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

#### macOS (Intel)
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-macos-x86_64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

#### macOS (Apple Silicon)
```bash
curl -L https://github.com/filipebarros/project-indicator/releases/latest/download/project-indicator-macos-aarch64.tar.gz | tar xz
sudo mv project-indicator /usr/local/bin/
chmod +x /usr/local/bin/project-indicator
```

#### Windows
1. Download `project-indicator-windows-x86_64.zip` from the [releases page](https://github.com/filipebarros/project-indicator/releases)
2. Extract the ZIP file
3. Add the directory containing `project-indicator.exe` to your PATH

### 2. Build from Source

#### Prerequisites
- Rust 1.80+ (install from [rustup.rs](https://rustup.rs/))
- Git

#### Steps
```bash
# Clone the repository
git clone https://github.com/filipebarros/project-indicator.git
cd project-indicator

# Build and install
cargo install --path .

# Or build without installing
cargo build --release
# Binary will be at target/release/project-indicator
```

#### Development Build
```bash
# Debug build for development
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check code quality
cargo clippy
cargo fmt
```

## Verification

Test that the installation works:

```bash
# Check version
project-indicator --version

# Test detection
project-indicator

# Test JSON output
project-indicator --format json

# Run benchmark
project-indicator benchmark --iterations 100
```

## Configuration

### Configuration File Location

Project-indicator looks for configuration files in the following order:

1. `$XDG_CONFIG_HOME/project-indicator/config.toml` (Linux)
2. `$HOME/.config/project-indicator/config.toml` (Linux/macOS)
3. `%APPDATA%\project-indicator\config.toml` (Windows)
4. `./project-indicator.toml` (current directory)

### Creating Configuration Directory

```bash
# Linux/macOS
mkdir -p ~/.config/project-indicator

# Windows (PowerShell)
New-Item -ItemType Directory -Force -Path "$env:APPDATA\project-indicator"
```

### Configuration Options Reference

#### Cache Section
- `enabled`: Enable/disable caching (default: true)
- `max_entries`: Maximum cache entries (default: 1000)
- `ttl_seconds`: Cache time-to-live in seconds (default: 300)

#### Theme Section
- `name`: Theme name - "default", "minimal", "colorful" (default: "default")
- `separator`: Text between language and framework (default: " · ")
- `show_icons`: Display icons (default: true)
- `show_colors`: Enable colored output (default: true)

#### Performance Section
- `parallel_processing`: Enable parallel file processing (default: true)
- `max_depth`: Maximum directory scan depth (default: 3)

#### Output Section
- `default_format`: Default output format (default: "default")
- `include_confidence`: Show confidence scores (default: false)

#### Languages Array
Each language definition requires:
- `name`: Language name
- `files`: Array of file patterns to match
- `color`: Hex color code
- `icon`: Unicode icon or emoji
- `priority`: Priority (1 = highest)

#### Frameworks Array
Each framework definition requires:
- `name`: Framework name
- `language`: Associated language name
- `detection_type`: Detection method (see below)
- `dependencies`: Array of dependency names to match
- `files`: Array of file patterns (optional)
- `color`: Hex color code
- `icon`: Unicode icon or emoji
- `priority`: Priority (1 = highest)

#### Detection Types
- `PackageJson`: npm/yarn dependencies (JavaScript/TypeScript)
- `CargoToml`: Cargo dependencies (Rust)
- `GoMod`: Go module dependencies
- `PyProjectToml`: Python project dependencies
- `GemSpec`: Ruby gem dependencies
- `ComposerJson`: PHP Composer dependencies
- `FileExists`: File/directory existence check

### Environment Variables

Override configuration with environment variables:

```bash
export PROJECT_INDICATOR_CACHE_ENABLED=false
export PROJECT_INDICATOR_CACHE_TTL=600
export PROJECT_INDICATOR_MAX_ENTRIES=500
export PROJECT_INDICATOR_THEME=minimal
```

### Shell Integration Setup

See the [shell integration examples](examples/shell-integration/) for detailed setup instructions for your specific shell.

## Troubleshooting

### Common Issues

#### "Command not found"
Ensure the binary is in your PATH:
```bash
echo $PATH
which project-indicator
```

#### Permission denied
Make sure the binary is executable:
```bash
chmod +x /usr/local/bin/project-indicator
```

#### Config file not found
Verify the config file location:
```bash
# Check if file exists
ls -la ~/.config/project-indicator/config.toml

# Create directory if needed
mkdir -p ~/.config/project-indicator
```

#### Slow performance
Enable caching and check your configuration:
```bash
# Check current performance
project-indicator benchmark

# Verify cache is enabled
project-indicator --help | grep cache
```

### Debug Mode

Run with verbose output to debug issues:
```bash
# Enable debug logging
RUST_LOG=debug project-indicator

# Test specific directory
project-indicator /path/to/project --no-cache
```

### Performance Testing

Benchmark your installation:
```bash
# Run performance tests
project-indicator benchmark --iterations 1000

# Test without cache
project-indicator benchmark --iterations 100 --no-cache

# Profile specific directory
time project-indicator /path/to/large/project
```

## Uninstalling

### Binary Installation
```bash
sudo rm /usr/local/bin/project-indicator
rm -rf ~/.config/project-indicator
```

### Cargo Installation
```bash
cargo uninstall project-indicator
```

### Source Installation
```bash
# Remove from ~/.cargo/bin/ if installed with cargo install --path
rm ~/.cargo/bin/project-indicator
```

## Getting Help

- Check the [README](README.md) for usage examples
- View [shell integration examples](examples/shell-integration/)
- Open an issue on [GitHub](https://github.com/filipebarros/project-indicator/issues)
- Run `project-indicator --help` for CLI options
