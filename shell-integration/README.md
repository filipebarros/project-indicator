# Shell Integration

Prompt hooks for fish, zsh, bash, and PowerShell. Each hook is a short script: it calls
`project-indicator` and wraps the output in prompt colors. That's all it
needs to do — **caching lives in the binary**, which keeps a persistent,
mtime-invalidated result cache under `$XDG_CACHE_HOME/project-indicator/`
(default `~/.cache`); total prompt cost is dominated by process spawn.

## Install

```bash
./install.sh          # detects your shell from $SHELL
./install.sh fish     # or pick one explicitly
```

Or manually:

- **fish** — copy `project-indicator.fish` to
  `~/.config/fish/functions/fish_right_prompt.fish`, or call
  `project_indicator_prompt` from your existing right prompt.
- **zsh** — `source /path/to/project-indicator.zsh` in `~/.zshrc`
  (appends to `RPROMPT`).
- **bash** — `source /path/to/project-indicator.bash` in `~/.bashrc`
  (appends to `PS1`).
- **PowerShell** — dot-source `project-indicator.ps1` from your `$PROFILE`
  (wraps your existing `prompt` function).

## Cache management

```bash
project-indicator cache stats   # entry count and disk usage
project-indicator cache clear   # start fresh
project-indicator --no-cache    # bypass for one invocation
```

The cache invalidates automatically when the project directory, its
manifests, your config file, or the binary version change, and also when a
subdirectory the scan listed changes.

## Testing

```bash
cargo build --release
./test-integrations.sh
```
