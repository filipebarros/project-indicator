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
- `--cache-ttl <SECONDS>` - Serve recent entries without revalidating watched paths; config and binary changes still invalidate them
- `-v, --verbose` - Enable verbose logging (also `RUST_LOG=debug`). Expected conditions such as an unparsable manifest or running in `$HOME` are silent by default and only logged here, so a prompt never shows them

### Subcommands

**Configuration Management:**
```bash
project-indicator config init [--force]
project-indicator config validate
project-indicator config edit
project-indicator config show
```

**Cache Management:**
```bash
project-indicator cache stats
project-indicator cache clear
```

## Output Examples

`simple` (the default) prints one colored glyph per detected project — the
framework's if one was detected, otherwise the language's — so it fits in a
prompt. `full` prints `icon|color` pairs separated by `•`, and `debug` dumps
the evidence.

```bash
$ project-indicator --format json
{"indicator":"TypeScript","frameworks":["React"],"icon":"","color":"#61dafb","confidence":0.85,"evidence":["package.json"]}
```

## Repository Detection

Inside a VCS repository (`.git` — file or directory — `.hg`, `.svn`), detection
is repository-wide. The tool finds the nearest VCS root and discovers the root
project and nested projects beneath it. Running from a subdirectory returns the
same result as running from the repository root:

```console
$ cd ~/work/shop              # backend/ (Rust, Axum) and frontend/ (React)
$ project-indicator --format full
<rust>|#dea584•<js>|#f7df1e•<react>|#61dafb
$ project-indicator --format json
{"indicator":null,…,"projects":[{"name":"backend","indicator":"Rust","frameworks":["Axum"],…},
                                {"name":"frontend","indicator":"JavaScript","frameworks":["React"],…}]}
```

Repository traversal uses the `ignore` crate's `WalkBuilder`. It skips hidden
directories, directory symlinks, noise directories such as `node_modules`, and
directories excluded by `.gitignore`, `.ignore`, or `.git/info/exclude`, with
nested rules and negations honoured. Ignore files must be small regular files;
if an unsafe special or oversized file is found, traversal restarts with ignore
filters disabled rather than risking a blocking read.

Exact-name root manifests still count when gitignored, preserving detection for
projects such as an ignored `Cargo.toml` or `go.work`. Source-only projects are
detected when they have no marked project ancestor. Results are sorted into
stable breadth-first order. Simple and full output deduplicate rendered
indicators; JSON and debug output retain individual project paths.

Outside a VCS repository, detection uses the nearest-project fallback, with a
four-level upward search and bounded nested-project discovery.

## Performance

Detection results are cached persistently across invocations. A typical project
costs about 3ms for a cache hit and 4ms for fresh detection, including process
startup. Repository scans scale with the number of visited directories and
watched inputs; cold repository scans have a five-second safety timeout.

The cache lives under `$XDG_CACHE_HOME/project-indicator/results/` (default
`~/.cache/project-indicator/results/`). A VCS repository uses its canonical root
as the key, so every subdirectory shares one entry; non-VCS detection uses the
canonical invocation directory. Entries invalidate when manifests, ignore
files, watched directories, the active config, or the binary change. Timed-out,
partial, and unreadable scans are never cached.

By default every watched path is revalidated on a cache hit. For large
repositories, `--cache-ttl <SECONDS>` skips that evidence validation while the
entry is younger than the TTL, trading bounded result staleness for lower
latency. Active-config, binary, repository-path, and home checks still apply;
TTL hits do not refresh the entry or extend its lifetime.

Current median measurements on the development machine:

| Scenario | Cold detection | Warm cache | Cache entry |
|----------|---------------:|-----------:|------------:|
| Flat synthetic repository, 10,000 candidate directories | 1.03s | 32.5ms | 6.6MB |
| Root-project synthetic repository, 10,000 candidate directories | 1.35s | 30.8ms | 6.6MB |
| Real dotfiles repository, 89 visited directories | 14.7ms | 3.5ms | 29KB |

These figures are examples, not guarantees; filesystem, repository contents,
and hardware all affect them. Run `cargo bench` or `hyperfine project-indicator`
to measure your own setup.

```bash
project-indicator cache stats   # entry count and disk usage
project-indicator cache clear   # start fresh
project-indicator --no-cache .  # bypass for one invocation
```

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
1  Runtime error (invalid path, config error, or another failure)
2  CLI usage error (invalid arguments or options)
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

The tool includes built-in safeguards against unsafe traversal:
- Non-VCS upward discovery is bounded to 4 levels; VCS-root discovery climbs
  until it finds a repository or reaches a boundary
- Boundary checks prevent scanning from home and system directories such as
  `/`, `/home`, `/root`, and `/System`
- Directory symlinks are never followed, preventing loops; symlinked files
  count as files
- Ignore files must be small regular files, preventing FIFOs and similar
  special files from blocking a scan

### File System Access

- Read-only operations - the tool never modifies project files
- Respects file system permissions - gracefully handles unreadable files
- JSON/TOML manifests are read up to 32 MiB through streaming filters, other manifests up to 1 MiB, and one run reads at most 64 MiB in total, so a hostile repository cannot exhaust memory
- Cache files stored in standard user cache directories (`$XDG_CACHE_HOME/project-indicator/results/`, default `~/.cache/...`)

## License

MIT License - see [LICENSE](LICENSE) for details.
