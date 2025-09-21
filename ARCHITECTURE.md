# Project Indicator - Architecture & Implementation Plan

## Overview

A high-performance CLI tool for detecting programming languages and frameworks in projects, designed as a rewrite of the existing `project-detector` binary with enhanced framework detection capabilities.

## Key Design Decisions

### Language Choice: Rust 🦀
- **Performance**: Zero-cost abstractions for maximum speed
- **Concurrency**: Rayon for parallel directory scanning, Tokio for async I/O
- **Safety**: Memory safety without garbage collection overhead
- **Ecosystem**: Rich crates for parsing (serde), CLI (clap), file operations (walkdir)
- **Deployment**: Single binary with static linking

### Core Architecture

#### Multi-Level Detection System
```
Project Detection Flow:
1. Language Detection (file patterns) → Base language identified
2. Framework Detection (within language) → Specific frameworks found
3. Priority Resolution → Best match selected
4. Output Formatting → Icon + color returned
```

#### Module Structure
```
src/
├── main.rs                 # CLI entry point
├── lib.rs                  # Library exports
├── config/
│   ├── mod.rs             # Config module exports
│   ├── parser.rs          # TOML config parsing with serde
│   ├── validator.rs       # Config validation & migration
│   └── migration.rs       # V1 → V2 migration tools
├── detection/
│   ├── mod.rs             # Detection engine exports
│   ├── engine.rs          # Core detection orchestration
│   ├── strategies.rs      # Detection strategy trait & implementations
│   ├── cache.rs           # Result caching for performance
│   └── matchers/
│       ├── mod.rs         # File content matcher exports
│       ├── package_json.rs # Node.js dependency scanning
│       ├── cargo_toml.rs  # Rust dependency scanning
│       ├── go_mod.rs      # Go module scanning
│       └── pyproject.rs   # Python project scanning
├── output/
│   ├── mod.rs             # Output formatting
│   ├── formatters.rs      # Multiple output formats (simple/full/json)
│   └── themes.rs          # Color theme support
└── cli/
    ├── mod.rs             # CLI interface
    ├── commands.rs        # Command implementations
    └── args.rs            # Clap argument parsing
```

## Data Structures

### Core Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndicator {
    pub name: String,
    pub files: Vec<String>,          // File patterns for language detection
    pub color: String,               // Hex color for display
    pub icon: String,                // Nerd Font icon
    pub priority: u8,                // Language detection priority
    pub frameworks: Vec<FrameworkDetector>, // Framework detection rules
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkDetector {
    pub name: String,
    pub detection_type: DetectionType,
    pub icon: Option<String>,        // Override language icon
    pub color: Option<String>,       // Override language color
    pub priority: u8,                // Framework priority within language
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DetectionType {
    PackageJson { dependencies: Vec<String> },
    CargoToml { dependencies: Vec<String> },
    GoMod { modules: Vec<String> },
    PyProjectToml { tools: Vec<String> },
    GemSpec { gems: Vec<String> },
    ComposerJson { packages: Vec<String> },
    FileExists { files: Vec<String> },
    ConfigFile { file: String, keys: Vec<String> },
}
```

### Detection Result
```rust
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub language: Option<ProjectIndicator>,
    pub frameworks: Vec<FrameworkMatch>,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct FrameworkMatch {
    pub framework: FrameworkDetector,
    pub confidence: f32,
    pub evidence: Vec<String>,       // Files that triggered detection
}
```

## Framework Detection Strategy

### Supported Ecosystems (25+ Frameworks)

#### JavaScript/TypeScript
- **React**: `package.json` → dependencies: ["react"]
- **Next.js**: `package.json` → dependencies: ["next"] + config files
- **Vue**: `package.json` → dependencies: ["vue", "@vue/core"]
- **Angular**: `package.json` → dependencies: ["@angular/core"]
- **Svelte**: `package.json` → dependencies: ["svelte"]
- **Vite**: `package.json` → dependencies: ["vite"] + `vite.config.*`

#### Python
- **Django**: `pyproject.toml` → tools: ["Django"] + `manage.py`
- **Flask**: `pyproject.toml` → tools: ["Flask"]
- **FastAPI**: `pyproject.toml` → tools: ["fastapi"]
- **Poetry**: `pyproject.toml` → `[tool.poetry]` section

#### Ruby
- **Rails**: `Gemfile` → gems: ["rails"] + `config/application.rb`
- **Sinatra**: `Gemfile` → gems: ["sinatra"]

#### Go
- **Gin**: `go.mod` → modules: ["github.com/gin-gonic/gin"]
- **Echo**: `go.mod` → modules: ["github.com/labstack/echo"]

#### Rust
- **Rocket**: `Cargo.toml` → dependencies: ["rocket"]
- **Actix**: `Cargo.toml` → dependencies: ["actix-web"]

### Detection Algorithm
```rust
pub fn detect_frameworks(
    path: &Path,
    language: &ProjectIndicator
) -> Vec<FrameworkMatch> {
    let mut matches = Vec::new();

    for framework in &language.frameworks {
        match &framework.detection_type {
            DetectionType::PackageJson { dependencies } => {
                if let Some(package_json) = parse_package_json(path) {
                    for dep in dependencies {
                        if package_json.has_dependency(dep) {
                            matches.push(FrameworkMatch::new(framework, 0.9));
                        }
                    }
                }
            }
            // ... other detection types
        }
    }

    // Sort by priority and confidence
    matches.sort_by(|a, b| {
        a.framework.priority.cmp(&b.framework.priority)
            .then(b.confidence.partial_cmp(&a.confidence).unwrap())
    });

    matches
}
```

## Performance Optimizations

### Caching Strategy
```rust
#[derive(Debug)]
pub struct DetectionCache {
    results: HashMap<PathBuf, CacheEntry>,
    ttl: Duration,
}

#[derive(Debug)]
struct CacheEntry {
    result: DetectionResult,
    timestamp: Instant,
    file_mtimes: HashMap<PathBuf, SystemTime>, // Track dependency file changes
}
```

- Cache results by directory path + key file modification times
- Invalidate when `package.json`, `Cargo.toml`, etc. change
- Configurable TTL (default: 5 minutes)
- Memory-only cache for simplicity

### Parallel Processing
```rust
use rayon::prelude::*;

// Parallel directory scanning
let files: Vec<PathBuf> = WalkDir::new(path)
    .into_iter()
    .par_bridge()
    .filter_map(Result::ok)
    .map(|entry| entry.into_path())
    .collect();

// Parallel framework detection
let frameworks: Vec<_> = candidates
    .par_iter()
    .flat_map(|lang| detect_frameworks(path, lang))
    .collect();
```

### File I/O Optimization
- Use `walkdir` for efficient directory traversal
- Read only necessary files (no full directory scans)
- Lazy loading of framework detectors
- Minimal memory allocations

## Configuration Format

### Enhanced TOML Schema
```toml
[meta]
version = "2.0"
cache_ttl = 300  # seconds

[display]
show_frameworks = true
max_frameworks = 2
framework_separator = "+"

[[languages]]
name = "TypeScript"
files = ["package.json", "tsconfig.json"]
color = "#3178C6"
icon = "󰛦"
priority = 1

  [[languages.frameworks]]
  name = "React"
  detection = { type = "PackageJson", dependencies = ["react"] }
  icon = "⚛️"
  color = "#61DAFB"
  priority = 1

  [[languages.frameworks]]
  name = "Next.js"
  detection = { type = "PackageJson", dependencies = ["next"] }
  files = ["next.config.js", "next.config.ts"]  # Additional file checks
  icon = "▲"
  color = "#000000"
  priority = 1
```

## CLI Interface

### Enhanced Commands
```bash
# Detection (primary use case)
project-indicator                    # Current directory, simple format
project-indicator --format full     # Icon + framework info
project-indicator --format json     # JSON output for scripting
project-indicator /path/to/project   # Specific directory

# Framework-specific
project-indicator --frameworks-only # Show only frameworks
project-indicator --language rust   # Force specific language

# Configuration management
project-indicator config validate   # Validate configuration
project-indicator config migrate    # Migrate from v1
project-indicator config edit       # Edit configuration

# Development tools
project-indicator benchmark         # Performance testing
project-indicator debug            # Verbose detection info
project-indicator cache clear      # Clear detection cache
```

### Output Formats
```bash
# Simple (default) - for shell prompts
⚛️

# Full - with color codes
⚛️|#61DAFB

# JSON - for scripting
{
  "language": "TypeScript",
  "frameworks": ["React"],
  "icon": "⚛️",
  "color": "#61DAFB"
}

# Compact - for status bars
React+TS
```

## Implementation Timeline

### Phase 1: Core Infrastructure (Week 1-2)
- [ ] Project scaffolding with Cargo workspace
- [ ] Core data structures and traits
- [ ] TOML configuration parsing with serde
- [ ] Basic language detection (file patterns)
- [ ] CLI interface with clap

### Phase 2: Framework Detection (Week 3-4)
- [ ] Package.json dependency scanning
- [ ] Cargo.toml, go.mod, pyproject.toml parsers
- [ ] Framework detection engine
- [ ] Priority resolution and conflict handling

### Phase 3: Performance & Polish (Week 5-6)
- [ ] Result caching implementation
- [ ] Parallel processing with rayon
- [ ] Performance benchmarking and optimization
- [ ] Comprehensive test suite

### Phase 4: Migration & Documentation (Week 7)
- [ ] V1 configuration migration tools
- [ ] Documentation and usage examples
- [ ] Fish shell integration testing
- [ ] Release preparation

## Dependencies

### Core Dependencies
```toml
[dependencies]
# CLI and config
clap = { version = "4.0", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# File operations and performance
walkdir = "2.0"
rayon = "1.7"
ignore = "0.4"  # Git-aware file traversal

# JSON parsing for package.json
serde_json = "1.0"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Caching
dashmap = "5.0"  # Concurrent HashMap for cache
```

### Development Dependencies
```toml
[dev-dependencies]
criterion = "0.5"      # Benchmarking
tempfile = "3.0"       # Test fixtures
pretty_assertions = "1.0"
```

## Migration from V1

### Backward Compatibility
- V2 can read V1 TOML configurations
- Automatic migration with `project-indicator config migrate`
- Graceful fallback to V1 behavior if V2 features fail
- Maintain same CLI interface for basic usage

### Migration Strategy
1. **Detection**: Automatically detect V1 vs V2 config format
2. **Convert**: Migrate V1 config to V2 format with framework stubs
3. **Enhance**: User manually adds framework detection rules
4. **Validate**: Ensure migrated config works correctly

This architecture provides a solid foundation for building a high-performance, extensible project detection tool that significantly enhances the existing functionality while maintaining compatibility and ease of use.