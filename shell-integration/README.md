# Shell Integration for Project Indicator

High-performance shell prompt integration with intelligent caching for Fish, Bash, and Zsh shells.

## Features

- **🚀 High Performance**: Intelligent 2-tier caching (memory + disk) with sub-millisecond response times
- **🔄 Auto-Invalidation**: Smart cache invalidation when project files change
- **⚡ Non-Blocking**: Async support for Zsh, prevents prompt lag
- **🛡️ Robust Error Handling**: Graceful fallbacks when binary is missing or fails
- **🎨 Customizable**: Easy prompt integration with color support
- **📊 Monitoring**: Built-in cache statistics and configuration tools

## Quick Start

### 1. Install Project Indicator

```bash
# Via Cargo (Rust package manager)
cargo install project-indicator

# Or download from releases
# https://github.com/filipebarros/project-indicator/releases
```

### 2. Install Shell Integration

Choose your shell and add the appropriate line to your shell's configuration file:

#### Fish Shell
```fish
# Add to ~/.config/fish/config.fish
source /path/to/project-indicator/shell-integration/project-indicator.fish

# Use in your prompt
function fish_right_prompt
    fish_right_prompt_project_indicator
end
```

#### Bash
```bash
# Add to ~/.bashrc or ~/.bash_profile
source /path/to/project-indicator/shell-integration/project-indicator.bash

# Use in PS1
export PS1='\u@\h:\w$(project_indicator_ps1)\$ '
```

#### Zsh
```zsh
# Add to ~/.zshrc
source /path/to/project-indicator/shell-integration/project-indicator.zsh

# Use as right prompt
export RPS1='$(project_indicator_rprompt)'

# Or integrate with existing prompt
export PROMPT='%n@%m:%~$(project_indicator_prompt)%# '
```

## Advanced Configuration

### Cache Settings

Control cache behavior for optimal performance:

```bash
# Set cache TTL (time-to-live) to 10 minutes
project_indicator_config ttl 600

# View current status and cache statistics
project_indicator_config status

# Clear all cached data
project_indicator_clear_cache
```

### Performance Optimization

The shell integration includes several performance optimizations:

1. **Memory Cache**: 30-second in-memory cache for the current directory
2. **Disk Cache**: 5-minute persistent cache with automatic invalidation
3. **Smart Invalidation**: Monitors project files (package.json, Cargo.toml, etc.)
4. **Timeout Protection**: 10-second timeout prevents hanging prompts

### Custom Formatting

#### Fish Shell Customization
```fish
# Custom formatting function
function my_project_prompt
    set -l info (project_info)
    if test -n "$info"
        set_color blue
        echo -n "[$info]"
        set_color normal
    end
end
```

#### Bash Customization
```bash
# Custom formatting
my_project_prompt() {
    local info=$(project_info)
    if [[ -n "$info" ]]; then
        echo -n " \[\033[94m\][$info]\[\033[0m\]"
    fi
}

# Use in PS1
PS1='\u@\h:\w$(my_project_prompt)\$ '
```

#### Zsh Customization
```zsh
# Custom formatting with colors
my_project_prompt() {
    local info=$(project_info)
    if [[ -n "$info" ]]; then
        print -n " %F{blue}[${info}]%f"
    fi
}

# Oh-My-Zsh theme integration
ZSH_THEME_PROJECT_INDICATOR_PREFIX=" %F{blue}["
ZSH_THEME_PROJECT_INDICATOR_SUFFIX="]%f"
```

## Integration Examples

### Starship Prompt

Add to your `starship.toml`:

```toml
[custom.project_indicator]
command = "project-indicator"
when = true
format = "[$output]($style) "
style = "bright-blue"
```

### Oh-My-Zsh Theme

```zsh
# In your theme file
function project_indicator_info() {
    local info=$(project_info)
    if [[ -n "$info" ]]; then
        echo "%{$fg[blue]%}[$info]%{$reset_color%}"
    fi
}

# Update your PROMPT
PROMPT='${ret_status} %{$fg[cyan]%}%c%{$reset_color%} $(project_indicator_info)$(git_prompt_info)%{$fg[cyan]%}❯%{$reset_color%} '
```

### Fish Oh-My-Fish Theme

```fish
# In your theme's fish_prompt.fish
function fish_prompt
    # ... existing prompt code ...

    set -l project_info (project_info)
    if test -n "$project_info"
        set_color blue
        echo -n "[$project_info] "
        set_color normal
    end

    # ... rest of prompt ...
end
```

## API Reference

### Public Functions

#### `project_info [directory]`
Get project information for the specified directory (or current directory).

```bash
# Get info for current directory
project_info

# Get info for specific directory
project_info /path/to/project
```

#### `project_indicator_clear_cache`
Clear all cached project information.

```bash
project_indicator_clear_cache
```

#### `project_indicator_config <command> [value]`
Configure the shell integration.

```bash
# View status
project_indicator_config status

# Set cache TTL to 10 minutes
project_indicator_config ttl 600

# View current TTL
project_indicator_config ttl
```

### Shell-Specific Functions

#### Fish Shell
- `fish_right_prompt_project_indicator` - Ready-to-use right prompt
- `__project_indicator_format` - Format project info with colors

#### Bash
- `project_indicator_ps1` - Ready-to-use PS1 integration
- `__project_indicator_format` - Format project info with colors

#### Zsh
- `project_indicator_rprompt` - Ready-to-use right prompt
- `project_indicator_prompt` - General prompt function
- `__project_indicator_format` - Format project info with colors

## Performance Benchmarks

Typical performance characteristics:

- **Cache Hit**: < 1ms (sub-millisecond response)
- **Cache Miss**: 5-20ms (depending on project size)
- **Memory Usage**: < 1MB for cache data
- **Disk Usage**: < 100KB per 1000 cached directories

## Troubleshooting

### Common Issues

#### Binary Not Found
```bash
Warning: project-indicator binary not found in PATH
```

**Solution**: Install project-indicator or add it to your PATH:
```bash
cargo install project-indicator
# or
export PATH="$PATH:/path/to/project-indicator"
```

#### Slow Prompt Response
If prompts feel slow, check cache status:
```bash
project_indicator_config status
```

**Solutions**:
- Increase cache TTL: `project_indicator_config ttl 900`
- Clear and rebuild cache: `project_indicator_clear_cache`
- Check for large project directories

#### Cache Issues
If project changes aren't reflected:
```bash
# Clear cache and restart shell
project_indicator_clear_cache
exec $SHELL
```

### Debug Mode

Enable debug output to troubleshoot issues:

```bash
# Fish
set -x PROJECT_INDICATOR_DEBUG 1

# Bash/Zsh
export PROJECT_INDICATOR_DEBUG=1
```

## File Locations

### Cache Directories
- **Fish**: `~/.cache/project-indicator-fish/`
- **Bash**: `~/.cache/project-indicator-bash/`
- **Zsh**: `~/.cache/project-indicator-zsh/`

### Integration Scripts
- **Fish**: `shell-integration/project-indicator.fish`
- **Bash**: `shell-integration/project-indicator.bash`
- **Zsh**: `shell-integration/project-indicator.zsh`

## Contributing

Contributions to improve shell integration are welcome! Areas for improvement:

- Additional shell support (PowerShell, etc.)
- Performance optimizations
- Better async support
- Enhanced error handling

## License

MIT License - see the main project for details.

## Related

- [Project Indicator Main Repository](https://github.com/filipebarros/project-indicator)
- [Starship Prompt](https://starship.rs/)
- [Oh-My-Zsh](https://ohmyz.sh/)
- [Oh-My-Fish](https://github.com/oh-my-fish/oh-my-fish)