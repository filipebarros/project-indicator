# Output Format Samples

This document shows examples of project-indicator output in all supported formats for different project types.

## Format Comparison

### Rust + Rocket Project

#### Default Format
```
🦀 Rust · 🚀 Rocket
```

#### JSON Format
```json
{
  "language": {
    "name": "Rust",
    "icon": "🦀",
    "color": "#DEA584",
    "files": ["Cargo.toml", "src/main.rs"],
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

#### Plain Format
```
Rust Rocket
```

#### Minimal Format
```
🦀🚀
```

#### Verbose Format
```
Language: Rust (🦀)
  Files found: Cargo.toml, src/main.rs
  Dependencies detected: rocket, serde
  Confidence: 95%

Framework: Rocket (🚀)
  Dependencies: rocket, rocket_contrib
  Files found: src/main.rs
  Confidence: 95%
  Priority: 1

Total detection time: 0.063ms
Cache hit: false
```

## Language-Specific Examples

### TypeScript + React + Next.js

#### Default Format
```
󰛦 TypeScript · ▲ Next.js
```

#### JSON Format
```json
{
  "language": {
    "name": "TypeScript",
    "icon": "󰛦",
    "color": "#3178C6",
    "files": ["tsconfig.json", "package.json"],
    "priority": 1
  },
  "frameworks": [
    {
      "name": "Next.js",
      "icon": "▲",
      "color": "#000000",
      "priority": 1,
      "confidence": 0.92
    },
    {
      "name": "React",
      "icon": "⚛️",
      "color": "#61DAFB",
      "priority": 1,
      "confidence": 0.98
    }
  ],
  "confidence": 0.95,
  "detection_time_ms": 0.124
}
```

#### Verbose Format
```
Language: TypeScript (󰛦)
  Files found: tsconfig.json, package.json, src/index.ts
  Confidence: 98%

Frameworks detected:
  1. Next.js (▲) - Priority 1, Confidence 92%
     Dependencies: next, @next/bundle-analyzer
     Files: next.config.js, pages/

  2. React (⚛️) - Priority 1, Confidence 98%
     Dependencies: react, react-dom, @types/react
     Files: src/components/, public/index.html

Selected: Next.js (higher specificity)
Total detection time: 0.124ms
```

### Python + Django

#### Default Format
```
🐍 Python · 🎸 Django
```

#### JSON Format
```json
{
  "language": {
    "name": "Python",
    "icon": "🐍",
    "color": "#3776AB",
    "files": ["requirements.txt", "manage.py"],
    "priority": 1
  },
  "frameworks": [
    {
      "name": "Django",
      "icon": "🎸",
      "color": "#092E20",
      "priority": 1,
      "confidence": 0.89
    }
  ],
  "confidence": 0.89,
  "detection_time_ms": 0.087
}
```

#### Verbose Format
```
Language: Python (🐍)
  Files found: requirements.txt, manage.py, settings.py
  Confidence: 91%

Framework: Django (🎸)
  Dependencies: django, djangorestframework
  Files found: manage.py, settings.py, urls.py
  Confidence: 89%
  Priority: 1

Total detection time: 0.087ms
Cache hit: true
```

### Go + Gin

#### Default Format
```
🐹 Go · 🍸 Gin
```

#### JSON Format
```json
{
  "language": {
    "name": "Go",
    "icon": "🐹",
    "color": "#00ADD8",
    "files": ["go.mod", "main.go"],
    "priority": 1
  },
  "frameworks": [
    {
      "name": "Gin",
      "icon": "🍸",
      "color": "#00ADD8",
      "priority": 1,
      "confidence": 0.94
    }
  ],
  "confidence": 0.94,
  "detection_time_ms": 0.045
}
```

## Edge Cases and Special Scenarios

### No Framework Detected

#### Default Format
```
🐍 Python
```

#### JSON Format
```json
{
  "language": {
    "name": "Python",
    "icon": "🐍",
    "color": "#3776AB",
    "files": ["*.py"],
    "priority": 1
  },
  "frameworks": [],
  "confidence": 0.78,
  "detection_time_ms": 0.032
}
```

#### Verbose Format
```
Language: Python (🐍)
  Files found: script.py, utils.py
  Confidence: 78%

No frameworks detected
Total detection time: 0.032ms
Cache hit: false
```

### No Project Detected

#### Default Format
```
(no output)
```

#### JSON Format
```json
{
  "language": null,
  "frameworks": [],
  "confidence": 0.0,
  "detection_time_ms": 0.018
}
```

#### Verbose Format
```
No project type detected
Files scanned: 5
Detection time: 0.018ms
```

### Multiple Frameworks (Priority Resolution)

#### Default Format
```
🟨 JavaScript · ⚛️ React
```

#### JSON Format (Showing All Frameworks)
```json
{
  "language": {
    "name": "JavaScript",
    "icon": "🟨",
    "color": "#F7DF1E",
    "files": ["package.json"],
    "priority": 2
  },
  "frameworks": [
    {
      "name": "React",
      "icon": "⚛️",
      "color": "#61DAFB",
      "priority": 1,
      "confidence": 0.95
    },
    {
      "name": "Express",
      "icon": "🚂",
      "color": "#000000",
      "priority": 2,
      "confidence": 0.88
    }
  ],
  "confidence": 0.92,
  "detection_time_ms": 0.156
}
```

#### Verbose Format
```
Language: JavaScript (🟨)
  Files found: package.json, src/index.js
  Confidence: 85%

Frameworks detected:
  1. React (⚛️) - Priority 1, Confidence 95%
     Dependencies: react, react-dom, react-scripts
     Files: src/App.js, public/index.html

  2. Express (🚂) - Priority 2, Confidence 88%
     Dependencies: express, cors, helmet
     Files: server.js

Selected: React (higher priority)
Total detection time: 0.156ms
```

## Multi-Project Output

### Multiple Directories

#### Default Format
```
/home/user/projects/rust-cli: 🦀 Rust
/home/user/projects/react-app: 🟨 JavaScript · ⚛️ React
/home/user/projects/python-api: 🐍 Python · 🌶️ Flask
/home/user/projects/empty-dir:
```

#### JSON Format
```json
[
  {
    "path": "/home/user/projects/rust-cli",
    "language": {
      "name": "Rust",
      "icon": "🦀",
      "color": "#DEA584"
    },
    "frameworks": [],
    "confidence": 0.90,
    "detection_time_ms": 0.045
  },
  {
    "path": "/home/user/projects/react-app",
    "language": {
      "name": "JavaScript",
      "icon": "🟨",
      "color": "#F7DF1E"
    },
    "frameworks": [
      {
        "name": "React",
        "icon": "⚛️",
        "color": "#61DAFB",
        "priority": 1,
        "confidence": 0.95
      }
    ],
    "confidence": 0.93,
    "detection_time_ms": 0.078
  },
  {
    "path": "/home/user/projects/python-api",
    "language": {
      "name": "Python",
      "icon": "🐍",
      "color": "#3776AB"
    },
    "frameworks": [
      {
        "name": "Flask",
        "icon": "🌶️",
        "color": "#000000",
        "priority": 1,
        "confidence": 0.87
      }
    ],
    "confidence": 0.89,
    "detection_time_ms": 0.092
  },
  {
    "path": "/home/user/projects/empty-dir",
    "language": null,
    "frameworks": [],
    "confidence": 0.0,
    "detection_time_ms": 0.012
  }
]
```

## Cache Statistics Output

```bash
$ project-indicator --cache-stats
Cache Statistics:
  Enabled: true
  Entries: 42
  Hits: 156
  Misses: 23
  Hit rate: 87.2%
  Memory usage: ~2.1KB
  Invalidations: 3
  Oldest entry: 245s ago
  Newest entry: 2s ago
```

## Benchmark Output

```bash
$ project-indicator benchmark --iterations 1000
Running benchmark with 1000 iterations...

┌─────────────────┬─────────────────┐
│ Metric          │ Value           │
├─────────────────┼─────────────────┤
│ Iterations      │ 1000            │
│ Average time    │ 63.2μs          │
│ Min time        │ 41.0μs          │
│ Max time        │ 2.1ms           │
│ Std deviation   │ 45.7μs          │
│ 95th percentile │ 156.3μs         │
│ 99th percentile │ 387.9μs         │
└─────────────────┴─────────────────┘

Cache Performance:
  Hit rate: 94.2%
  Average hit time: 12.3μs
  Average miss time: 2.1ms
  Cache size: 156 entries

Performance Grade: A+ (Excellent)
Recommendation: Cache is working optimally
```

## Error Output Examples

### Invalid Path
```bash
$ project-indicator /nonexistent/path
Error: No such file or directory (os error 2)
Path: /nonexistent/path
```

### Invalid Config
```bash
$ project-indicator --config invalid.toml
Error: Failed to parse config file
File: invalid.toml
Line: 5, Column: 12
Reason: Invalid TOML syntax
```

### Permission Denied
```bash
$ project-indicator /root/restricted
Error: Permission denied (os error 13)
Path: /root/restricted
Suggestion: Check file permissions or run with appropriate privileges
```

## Integration-Specific Formats

### Shell Prompt Format (Minimal)
```
🦀🚀  # Just icons for space efficiency
```

### Status Bar Format (Custom)
```
[🦀 Rust]  # Brackets for visual separation
```

### CI/CD Format (Machine Readable)
```
LANG=Rust FRAMEWORK=Rocket CONFIDENCE=95
```

## Theme Variations

### Default Theme
```
🦀 Rust · 🚀 Rocket
```

### Minimal Theme (No Icons)
```
Rust · Rocket
```

### Colorful Theme (Terminal Colors)
```
🦀 \e[33mRust\e[0m · 🚀 \e[31mRocket\e[0m
```

### ASCII Theme (No Unicode)
```
[Rust] [Rocket]
```

This comprehensive output sample guide demonstrates how project-indicator formats information across all supported output modes and project types.