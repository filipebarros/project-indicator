# CLI Usage Examples

This document provides comprehensive examples of using project-indicator from the command line.

## Basic Usage

### Current Directory Detection
```bash
# Detect current directory
$ project-indicator
🦀 Rust · 🚀 Rocket

# Same as above (explicit current directory)
$ project-indicator .
🦀 Rust · 🚀 Rocket
```

### Specific Directory Detection
```bash
# Detect a specific directory
$ project-indicator /path/to/my-react-app
󰛦 TypeScript · ⚛️ React

# Multiple directories
$ project-indicator /path/to/rust-project /path/to/js-project
/path/to/rust-project: 🦀 Rust · 🚀 Rocket
/path/to/js-project: 🟨 JavaScript · ⚛️ React
```

## Output Formats

### Default Format (Human-readable)
```bash
$ project-indicator
🦀 Rust · 🚀 Rocket
```

### JSON Format
```bash
$ project-indicator --format json
{
  "language": {
    "name": "Rust",
    "icon": "🦀",
    "color": "#DEA584",
    "files": ["Cargo.toml", "Cargo.lock"],
    "priority": 1
  },
  "frameworks": [
    {
      "name": "Rocket",
      "icon": "🚀",
      "color": "#D22D72",
      "priority": 1,
      "confidence": 0.95
    }
  ],
  "confidence": 0.95,
  "detection_time_ms": 0.063
}
```

### Plain Text Format
```bash
$ project-indicator --format plain
Rust Rocket
```

### Minimal Format
```bash
$ project-indicator --format minimal
🦀🚀
```

### Verbose Format
```bash
$ project-indicator --format verbose
Language: Rust (🦀)
  Files found: Cargo.toml, Cargo.lock, src/main.rs
  Confidence: 95%

Framework: Rocket (🚀)
  Dependencies: rocket, rocket_contrib
  Files found: src/main.rs
  Confidence: 95%
  Priority: 1

Total detection time: 0.063ms
Cache hit: false
```

## Command Line Options

### Help and Version
```bash
# Show help
$ project-indicator --help
A fast project type and framework detector

Usage: project-indicator [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Path(s) to analyze [default: .]

Options:
  -f, --format <FORMAT>    Output format [default: default] [possible values: default, json, plain, minimal, verbose]
  -c, --config <CONFIG>    Path to config file
      --no-cache          Disable caching
      --cache-stats       Show cache statistics
  -h, --help              Print help
  -V, --version           Print version

# Show version
$ project-indicator --version
project-indicator 0.1.0
```

### Configuration Options
```bash
# Use custom config file
$ project-indicator --config /path/to/custom-config.toml

# Disable caching
$ project-indicator --no-cache

# Show cache statistics
$ project-indicator --cache-stats
Cache Statistics:
  Entries: 42
  Hits: 156
  Misses: 23
  Hit rate: 87.2%
  Memory usage: ~2.1KB
```

## Language Detection Examples

### Rust Projects
```bash
# Cargo project
$ cd rust-cli-tool && project-indicator
🦀 Rust

# With Rocket framework
$ cd rocket-api && project-indicator
🦀 Rust · 🚀 Rocket

# With Actix framework
$ cd actix-web-app && project-indicator
🦀 Rust · 🕸️ Actix-Web
```

### JavaScript/TypeScript Projects
```bash
# Plain JavaScript
$ cd vanilla-js && project-indicator
🟨 JavaScript

# React project
$ cd react-app && project-indicator
🟨 JavaScript · ⚛️ React

# TypeScript React
$ cd typescript-react && project-indicator
󰛦 TypeScript · ⚛️ React

# Next.js project
$ cd nextjs-app && project-indicator
󰛦 TypeScript · ▲ Next.js

# Vue.js project
$ cd vue-app && project-indicator
🟨 JavaScript · 🔹 Vue.js
```

### Python Projects
```bash
# Plain Python
$ cd python-scripts && project-indicator
🐍 Python

# Django project
$ cd django-blog && project-indicator
🐍 Python · 🎸 Django

# Flask application
$ cd flask-api && project-indicator
🐍 Python · 🌶️ Flask

# FastAPI project
$ cd fastapi-service && project-indicator
🐍 Python · ⚡ FastAPI
```

### Go Projects
```bash
# Go module
$ cd go-service && project-indicator
🐹 Go

# Gin framework
$ cd gin-api && project-indicator
🐹 Go · 🍸 Gin

# Echo framework
$ cd echo-server && project-indicator
🐹 Go · 📡 Echo
```

### Other Languages
```bash
# PHP Laravel
$ cd laravel-app && project-indicator
🐘 PHP · 🎨 Laravel

# Ruby on Rails
$ cd rails-blog && project-indicator
💎 Ruby · 🛤️ Rails

# Java Spring Boot
$ cd spring-boot-api && project-indicator
☕ Java · 🍃 Spring Boot

# C# .NET
$ cd dotnet-api && project-indicator
🔷 C# · 🌐 .NET
```

## Advanced Usage

### Multiple Projects Analysis
```bash
# Analyze multiple directories
$ project-indicator ~/code/*
/home/user/code/rust-cli: 🦀 Rust
/home/user/code/react-app: 🟨 JavaScript · ⚛️ React
/home/user/code/python-api: 🐍 Python · 🌶️ Flask
/home/user/code/go-service: 🐹 Go · 🍸 Gin

# JSON output for multiple projects
$ project-indicator --format json ~/code/rust-cli ~/code/react-app
[
  {
    "path": "/home/user/code/rust-cli",
    "language": {
      "name": "Rust",
      "icon": "🦀",
      "color": "#DEA584"
    },
    "frameworks": [],
    "confidence": 0.90
  },
  {
    "path": "/home/user/code/react-app",
    "language": {
      "name": "JavaScript",
      "icon": "🟨",
      "color": "#F7DF1E"
    },
    "frameworks": [
      {
        "name": "React",
        "icon": "⚛️",
        "color": "#61DAFB"
      }
    ],
    "confidence": 0.95
  }
]
```

### Scripting Examples
```bash
# Get just the language name
$ project-indicator --format json | jq -r '.language.name'
Rust

# Get framework name
$ project-indicator --format json | jq -r '.frameworks[0].name // "none"'
Rocket

# Check if it's a specific language
$ [[ "$(project-indicator --format json | jq -r '.language.name')" == "Rust" ]] && echo "It's Rust!"
It's Rust!

# Get confidence score
$ project-indicator --format json | jq -r '.confidence'
0.95

# Conditional execution based on project type
$ if [[ "$(project-indicator --format plain)" =~ "React" ]]; then
    echo "Running React-specific commands..."
    npm start
  fi
```

### Performance Testing
```bash
# Benchmark detection performance
$ project-indicator benchmark
Running benchmark with 1000 iterations...

Results:
  Average time: 63.2μs
  Min time: 41.0μs
  Max time: 2.1ms
  Std deviation: 45.7μs

Cache statistics:
  Hit rate: 94.2%
  Cache size: 156 entries

# Custom benchmark parameters
$ project-indicator benchmark --iterations 5000 --warmup 100
Running benchmark with 5000 iterations (100 warmup)...

# Benchmark without cache
$ project-indicator benchmark --no-cache --iterations 100
Running benchmark without cache (100 iterations)...

Results:
  Average time: 2.3ms
  Cache disabled - showing raw detection performance
```

### Debug and Troubleshooting
```bash
# Enable debug logging
$ RUST_LOG=debug project-indicator
DEBUG project_indicator::detection::engine > Starting detection for path: /current/dir
DEBUG project_indicator::detection::cache > Cache miss for path: /current/dir
DEBUG project_indicator::detection::engine > Found language files: ["Cargo.toml", "src/main.rs"]
DEBUG project_indicator::detection::matchers::cargo_toml > Parsing Cargo.toml dependencies
DEBUG project_indicator::detection::engine > Detected frameworks: ["Rocket"]
🦀 Rust · 🚀 Rocket

# Test specific directory with verbose output
$ project-indicator --format verbose /path/to/complex/project
Language: TypeScript (󰛦)
  Files found: tsconfig.json, package.json, src/index.ts
  Confidence: 98%
  Detection time: 0.124ms

Framework: Next.js (▲)
  Dependencies: next, react, react-dom
  Files found: next.config.js, pages/index.tsx
  Confidence: 96%
  Priority: 1
  Detection time: 0.087ms

Total detection time: 0.211ms
Cache hit: false
Parallel processing: enabled
Files scanned: 23
```

## Integration Examples

### Shell Prompt Integration
```bash
# Fish shell function
function project_prompt
    set -l info (project-indicator 2>/dev/null)
    test -n "$info"; and echo "[$info]"
end

# Bash function
project_prompt() {
    local info=$(project-indicator 2>/dev/null)
    [[ -n "$info" ]] && echo "[$info]"
}

# Zsh function
project_prompt() {
    local info=$(project-indicator 2>/dev/null)
    [[ -n "$info" ]] && echo "[$info]"
}
```

### CI/CD Integration
```bash
# GitHub Actions workflow
- name: Detect project type
  run: |
    PROJECT_INFO=$(project-indicator --format json)
    PROJECT_LANG=$(echo "$PROJECT_INFO" | jq -r '.language.name')
    echo "PROJECT_LANGUAGE=$PROJECT_LANG" >> $GITHUB_ENV

# Conditional steps based on project type
- name: Run Rust tests
  if: env.PROJECT_LANGUAGE == 'Rust'
  run: cargo test

- name: Run Node.js tests
  if: env.PROJECT_LANGUAGE == 'JavaScript' || env.PROJECT_LANGUAGE == 'TypeScript'
  run: npm test
```

### Editor Integration
```bash
# Vim statusline
set statusline+=%{system('project-indicator --format plain')}

# VS Code task
{
  "label": "Show Project Type",
  "type": "shell",
  "command": "project-indicator --format verbose",
  "group": "build"
}

# Emacs function
(defun show-project-type ()
  "Show current project type in minibuffer"
  (interactive)
  (message (shell-command-to-string "project-indicator")))
```

## Exit Codes

```bash
# Success: project detected
$ project-indicator; echo $?
🦀 Rust · 🚀 Rocket
0

# No project detected (not an error)
$ cd /tmp && project-indicator; echo $?

0

# Error: invalid path
$ project-indicator /nonexistent/path; echo $?
Error: No such file or directory (os error 2)
1

# Error: invalid config
$ project-indicator --config /invalid/config.toml; echo $?
Error: Failed to read config file
1
```

## Output Samples by Project Type

### Complex Multi-Framework Projects
```bash
# Full-stack JavaScript project
$ cd fullstack-app && project-indicator --format verbose
Language: TypeScript (󰛦)
  Files found: tsconfig.json, package.json
  Confidence: 95%

Frameworks detected:
  1. Next.js (▲) - Priority 1, Confidence 92%
     Dependencies: next, @next/bundle-analyzer
     Files: next.config.js, pages/

  2. React (⚛️) - Priority 1, Confidence 98%
     Dependencies: react, react-dom, @types/react
     Files: src/components/, public/index.html

Selected: Next.js (higher specificity)
Detection time: 0.234ms
```

### Monorepo Detection
```bash
# Monorepo with multiple projects
$ cd monorepo && project-indicator packages/*
packages/frontend: 󰛦 TypeScript · ⚛️ React
packages/backend: 🦀 Rust · 🚀 Rocket
packages/mobile: 🎯 Dart · 📱 Flutter
packages/shared: 󰛦 TypeScript
```

This comprehensive CLI usage guide covers all aspects of using project-indicator effectively in various scenarios.