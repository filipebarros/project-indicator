# Project Indicator

🚀 A blazingly fast Rust CLI tool for detecting project types and frameworks in your working directory.

[![Build Status](https://github.com/filipebarros/project-indicator/workflows/CI/badge.svg)](https://github.com/filipebarros/project-indicator/actions)
[![Release](https://img.shields.io/github/v/release/filipebarros/project-indicator)](https://github.com/filipebarros/project-indicator/releases)

Project Indicator is a high-performance replacement for shell-based project detection tools, designed to be integrated into your shell prompt or status line. It quickly identifies what type of project you're working on and displays relevant information with customizable formatting and colors.

## Features

- 🔍 **Multi-language Detection**: Rust, JavaScript/TypeScript, Python, Go, Gleam and more
- 🏗️ **Framework Recognition**: React, Svelte, Axum, Django, Gin, Spring Boot, Laravel, Rails, Hono and more
- 🔒 **Read-only & safe by construction**: never modifies project files, never runs a shell, no `unsafe` code
- ⚡ **Blazing Performance**: 3-5ms typical detection
- 🎨 **Multiple Output Formats**: Simple, Full, JSON, and Debug formats
- 🧠 **Advanced Detection**: Confidence-based scoring with weighted root indicators
- 🐚 **Shell Integration**: Ready-to-use hooks for Bash, Zsh, Fish and PowerShell

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
- `-v, --verbose` - Enable verbose logging (also `RUST_LOG=debug`). Expected conditions such as an unparsable manifest or running in `$HOME` are silent by default and only logged here, so a prompt never shows them

### Subcommands

**Configuration Management:**
```bash
project-indicator config init [--force]
project-indicator config validate
project-indicator config edit
project-indicator config show
```

## Output Examples

`simple` (the default) prints one colored glyph — the framework's if one was
detected, otherwise the language's — so it fits in a prompt. `full` prints
`icon|color` pairs separated by `•`, and `debug` dumps the evidence.

```bash
$ project-indicator --format json
{"indicator":"TypeScript","frameworks":["React"],"icon":"","color":"#61dafb","confidence":0.85,"evidence":["package.json"]}
```

## Performance

Detection results are cached persistently across invocations, so shell
prompt redraws cost almost nothing:

- Cache hit: about 3ms wall time including process spawn
- Fresh detection (first visit or after a change): about 4ms on a typical project
- Worst case (deep nesting, cold): under 50ms

The cache lives under `$XDG_CACHE_HOME/project-indicator/` (default
`~/.cache`). A VCS repository uses its canonical root as the key, so every
subdirectory shares one entry; non-VCS projects keep their project-directory
key. Entries invalidate when manifests, ignore files, watched directories,
your config, or the binary change. Cache-hit cost scales with the number of
watched inputs rather than being constant. Timed-out, partial, and unreadable
scans are never cached.

Repository-wide cold scans use a five-second safety timeout; partial results
are marked truncated and are not persisted. Cold scanning shares one directory
listing per directory across scanning, root-marker lookup, and child
discovery (directories with local ignore files still take the ignore-walker
path), and nested project detection reuses the repository scan instead of
rescanning. Synthetic benchmarks with flat repositories show the expected
scaling: 1,000 candidate directories cost about 40ms cold and 2.5ms warm,
while 10,000 cost about 540ms cold and 31ms warm with 20,001 watched paths
and a 6.6MB cache entry. A repository whose root is itself a project — the
common dotfiles shape — costs about 72ms cold at 1,000 directories and
890ms at 10,000; its warm validation is unchanged at about 31ms, since cache
validation cost is unaffected by this work. On a real dotfiles repository of
about 7,400 directories, a cold scan went from about 3.9s to about 0.5s with
byte-identical output. Repository entries are not dropped based on watch
count; larger repositories simply pay higher cold-scan and validation costs.

```bash
project-indicator cache stats   # entry count and disk usage
project-indicator cache clear   # start fresh
project-indicator --no-cache .  # bypass for one invocation
```

Run `cargo bench` (or `hyperfine project-indicator`) to measure on your machine.

## Configuration

### Quick Start

Write the built-in configuration (every supported language and framework) to a file and edit it:

```bash
project-indicator config init
```

### Configuration File

Create `~/.config/project-indicator/config.toml`:

```toml
# Display settings
[display]
max_frameworks = 5  # 0 hides frameworks

# Detection settings
[detection]
require_vcs_root = false


# Custom indicator (project type). `ecosystems` links it to the framework
# catalog: frameworks sharing an ecosystem are detected for this indicator.
# Ties between indicators follow the order of the [[indicators]] rows.
[[indicators]]
name = "My Language"
files = ["*.mylang", "mylang.config"]
color = "#FF6B6B"
icon = "M"
ecosystems = ["npm"]

# Custom framework — defined once, surfaced by every indicator that shares
# one of its ecosystems
[[frameworks]]
name = "My Framework"
ecosystems = ["npm"]
color = "#4ECDC4"
priority = 1

[frameworks.detection]
dependencies = ["my-framework"]
```

### Repositories

Inside a VCS repository (`.git` — file or directory — `.hg`, `.svn`), detection is repository-wide. The tool finds the nearest VCS root and recursively discovers projects beneath it, including the root project and nested projects. The result is the same whether you run it at the root or deep inside a package:

```
$ cd ~/work/shop              # has .git; contains backend/ (Rust, Axum) and frontend/ (React)
$ project-indicator --format full
<rust>|#dea584•<js>|#f7df1e•<react>|#61dafb   # icon|color components, as for a project
$ project-indicator --format json
{"indicator":null,…,"projects":[{"name":"backend","indicator":"Rust","frameworks":["Axum"],…},
                                {"name":"frontend","indicator":"JavaScript","frameworks":["React"],…}]}
```

Traversal is stable by name. Hidden directories, directory symlinks, and noise directories such as `node_modules` are skipped. Source-only projects are detected when they have no marked project ancestor. Simple and full output show each distinct rendered indicator once; JSON and debug output retain individual project paths. Outside a VCS repository, the existing nearest-project fallback remains, including its four-level upward search and bounded nested-project discovery.

### Configuration Locations

Priority order (first match wins):
1. `$XDG_CONFIG_HOME/project-indicator/config.toml` (defaults to `~/.config`)
2. `$XDG_CONFIG_HOME/project-indicator.toml`
3. `$HOME/.project-indicator.toml`

The current directory is never searched: a cloned repository cannot change
your prompt by shipping a config file.

Parsing is strict: unknown or removed keys make the config fail to parse.
A failing config never breaks your prompt — detection falls back to the
built-in configuration — but customizations are ignored until the file is fixed.
Run `project-indicator config validate` to see the exact error, or
`project-indicator config init --force` to regenerate.


## Shell Integration

Thin prompt hooks for **fish, zsh, bash and PowerShell** live in
[`shell-integration/`](shell-integration/README.md). Each hook just calls `project-indicator` and
prints its output; caching is done by the binary itself.

```bash
shell-integration/install.sh fish   # or zsh | bash
```

PowerShell: dot-source `shell-integration/project-indicator.ps1` from your
`$PROFILE`. See the [integration README](shell-integration/README.md) for
details.

## Supported Languages & Frameworks

23 languages and 44 frameworks in the built-in configuration. A framework is
shown for every language that shares one of its package ecosystems (the
JavaScript-family languages share the npm framework set). Bun is a runtime,
not a language: `bun.lock` and friends are Node markers.

| Language   | Frameworks Detected |
|------------|---------------------|
| C          | — |
| C#         | ASP.NET Core, Xamarin |
| C++        | — |
| Dart       | Flutter |
| Elixir     | Phoenix |
| Gleam      | — |
| Go         | Gin, Echo, Fiber |
| Java       | Spring Boot |
| JavaScript | React, Vue, Angular, Svelte, SolidJS, Hono, Next.js, Nuxt, Astro, NestJS, Express, Electron, React Native, Tauri, Vite |
| Julia      | — |
| Kotlin     | Spring Boot, Ktor, Android |
| Lua        | LÖVE |
| Odin       | — |
| PHP        | Laravel, Symfony, CodeIgniter, Yii |
| Python     | Django, Flask, FastAPI, Tornado, Pyramid, Sanic |
| R          | — |
| Ruby       | Rails, Hanami, Sinatra |
| Rust       | Axum, Actix Web, Rocket, Tauri |
| Scala      | Akka HTTP |
| Swift      | Vapor |
| TypeScript | React, Vue, Angular, Svelte, SolidJS, Hono, Next.js, Nuxt, Astro, NestJS, Express, Electron, React Native, Tauri, Vite |
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
dependencies = ["react"]
```

**Detection methods** (`[frameworks.detection]` holds exactly one of these keys):
- `dependencies` - dependency names checked in the manifests of the
  framework's ecosystems (`npm`, `pypi`, `cargo`, `go`, `packagist`,
  `rubygems`, `maven`, `gradle`, `nuget`, `sbt`, `pub`, `hex`, `luarocks`,
  `swiftpm`)
- `files` - file/directory presence

### Confidence Scoring

Detection uses weighted scoring based on:

- **Pattern importance**: the declared root-indicator weight times its context authority (default 0.5 for plain source globs)
- **File placement**: root files score full weight (1.0); probed source directories (`src/main/java`) score lower — depth weight (1.0/0.7/0.4/…) × directory tier (Source 1.2 / Root 1.0 / Unknown 0.8 / Noise 0.1)

Implementation: `src/types/matched_file.rs`, `src/detection/confidence_scorer.rs`, `src/detection/engine.rs`

## Development

```bash
cargo test                                                  # all suites
cargo clippy --all-targets --all-features -- -D warnings    # lint (unwrap/expect are denied)
cargo bench                                                 # criterion benchmarks
```

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Make your changes with tests
4. Ensure CI passes: `cargo test && cargo clippy`
5. Submit a pull request with appropriate semver label

#### Releases

Merging to `main` triggers an automated release with cross-platform binaries.
Label every PR with one of:
- `major` - Breaking changes
- `minor` - New features
- `patch` - Bug fixes
- `no-release` - No version bump

Labels are the only thing that selects the bump (PR titles are not read). A
merge without a label, or a direct push to `main`, ships a **patch** release
by default; use `no-release` for docs and test-only changes.

## Architecture

See [CLAUDE.md](CLAUDE.md) for the detection pipeline, caching design and module map.

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
- Upward traversal is bounded (at most 4 levels above the start directory)
- Boundary directory checks prevent scanning from system directories (`/`, `/home`, `/root`, `/System`)
- Directory symlinks are never followed (the walker doesn't follow links, so loops can't occur); symlinked files count as files

### File System Access

- Read-only operations - the tool never modifies project files
- Respects file system permissions - gracefully handles unreadable files
- JSON/TOML manifests are read up to 32 MiB through streaming filters, other manifests up to 1 MiB, and one run reads at most 64 MiB in total, so a hostile repository cannot exhaust memory
- Cache files stored in standard user cache directories (`$XDG_CACHE_HOME/project-indicator/results/`, default `~/.cache/...`)

## License

MIT License - see [LICENSE](LICENSE) for details.
