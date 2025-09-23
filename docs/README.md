# Project-Indicator Documentation

Welcome to the comprehensive documentation for project-indicator, a blazingly fast Rust CLI tool for detecting project types and frameworks.

## 📚 Documentation Index

### Getting Started
- **[Main README](../README.md)** - Project overview, quick start, and basic usage
- **[Installation Guide](../INSTALL.md)** - Detailed installation and configuration instructions

### Usage Guides
- **[CLI Usage Examples](../examples/CLI_USAGE.md)** - Comprehensive command-line usage examples
- **[Output Format Samples](../examples/OUTPUT_SAMPLES.md)** - Examples of all output formats

### Integration
- **[Shell Integration](../examples/shell-integration/)** - Ready-to-use shell integrations
  - [Fish Shell](../examples/shell-integration/fish.fish)
  - [Zsh](../examples/shell-integration/zsh.zsh)
  - [Bash](../examples/shell-integration/bash.bash)
  - [Integration README](../examples/shell-integration/README.md)

### Configuration
- **[Example Config](../config/examples/project-indicator.toml)** - Complete configuration example
- **[Configuration Reference](../INSTALL.md#configuration-options-reference)** - All configuration options explained

### Development
- **[Pull Request Template](../.github/PULL_REQUEST_TEMPLATE.md)** - SemVer compliance guide for contributors
- **[GitHub Workflows](../.github/workflows/)** - Automated CI/CD setup

## 🚀 Quick Navigation

### I want to...

**Install project-indicator**
→ [Installation Guide](../INSTALL.md#installation-methods)

**Set up shell integration**
→ [Shell Integration Examples](../examples/shell-integration/)

**See all CLI options**
→ [CLI Usage Examples](../examples/CLI_USAGE.md#command-line-options)

**Configure custom languages/frameworks**
→ [Configuration Reference](../INSTALL.md#configuration-options-reference)

**Understand output formats**
→ [Output Format Samples](../examples/OUTPUT_SAMPLES.md)

**Contribute to the project**
→ [Pull Request Template](../.github/PULL_REQUEST_TEMPLATE.md)

**Report issues or get help**
→ [GitHub Issues](https://github.com/filipebarros/project-indicator/issues)

## 📖 Documentation Structure

```
project-indicator/
├── README.md                          # Main project overview
├── INSTALL.md                         # Installation & configuration
├── docs/
│   └── README.md                      # This file - documentation index
├── examples/
│   ├── CLI_USAGE.md                   # Command-line usage examples
│   ├── OUTPUT_SAMPLES.md              # Output format examples
│   └── shell-integration/             # Shell integration files
│       ├── README.md                  # Integration guide
│       ├── fish.fish                  # Fish shell integration
│       ├── zsh.zsh                    # Zsh integration
│       └── bash.bash                  # Bash integration
├── config/
│   └── examples/
│       └── project-indicator.toml     # Example configuration
└── .github/
    ├── PULL_REQUEST_TEMPLATE.md       # PR template with SemVer guide
    └── workflows/                     # Automated CI/CD workflows
```

## 🎯 Key Features Covered

### Performance
- **Caching System**: File modification-aware caching for consistent sub-100μs performance
- **Parallel Processing**: Multi-core file scanning for large projects
- **Benchmarking**: Built-in performance testing and statistics

### Language Support
- **10+ Languages**: Rust, JavaScript/TypeScript, Python, Go, Java, PHP, Ruby, C#, Swift
- **Framework Detection**: 20+ frameworks including React, Django, Rails, Spring Boot
- **Custom Detection**: Configurable rules for proprietary frameworks

### Output Flexibility
- **5 Output Formats**: Default, JSON, Plain, Minimal, Verbose
- **Shell Integration**: Ready-to-use prompts for Fish, Zsh, Bash
- **Theming**: Customizable colors, icons, and separators

### Developer Experience
- **Zero Configuration**: Works out of the box with sensible defaults
- **Extensible**: TOML configuration for custom languages and frameworks
- **CI/CD Ready**: JSON output perfect for automation
- **Error Handling**: Graceful failures with helpful error messages

## 🔧 Configuration Quick Reference

### Basic Setup
```toml
[cache]
enabled = true
max_entries = 1000
ttl_seconds = 300

[theme]
separator = " · "
show_icons = true
```

### Custom Language
```toml
[[languages]]
name = "My Language"
files = ["*.mylang"]
icon = "🎯"
priority = 1
```

### Custom Framework
```toml
[[frameworks]]
name = "My Framework"
language = "JavaScript"
detection_type = "PackageJson"
dependencies = ["my-framework"]
icon = "🌟"
priority = 1
```

## 🛠️ Common Use Cases

### Shell Prompt Integration
Add real-time project detection to your shell prompt for instant context about your current working directory.

### CI/CD Pipeline Integration
Use JSON output to conditionally run different build steps based on detected project type.

### Editor/IDE Integration
Display current project information in your editor's status line or use for context-aware tooling.

### Project Organization
Quickly identify project types when browsing through multiple repositories.

### Development Workflow
Automatically switch development tools and configurations based on detected project type.

## 🔍 Troubleshooting

Common issues and solutions are covered in:
- [Installation Guide - Troubleshooting](../INSTALL.md#troubleshooting)
- [CLI Usage - Debug Mode](../examples/CLI_USAGE.md#debug-and-troubleshooting)

## 🤝 Contributing

We welcome contributions! Please see:
- [Pull Request Template](../.github/PULL_REQUEST_TEMPLATE.md) for SemVer compliance
- [GitHub Issues](https://github.com/filipebarros/project-indicator/issues) for bug reports and feature requests

## 📄 License

Project-indicator is released under the MIT License. See [LICENSE](../LICENSE) for details.

---

*This documentation covers project-indicator version 0.1.0 and later. For the latest updates, visit our [GitHub repository](https://github.com/filipebarros/project-indicator).*