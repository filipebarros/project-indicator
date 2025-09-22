# Shell Integration Examples

This directory contains shell integration examples for project-indicator across different shells.

## Quick Setup

### Fish Shell
```bash
# Add to ~/.config/fish/config.fish
source /path/to/project-indicator/examples/shell-integration/fish.fish
```

### Zsh
```bash
# Add to ~/.zshrc
source /path/to/project-indicator/examples/shell-integration/zsh.zsh
```

### Bash
```bash
# Add to ~/.bashrc
source /path/to/project-indicator/examples/shell-integration/bash.bash
```

## Features Included

### Basic Integration
- Right prompt with project information
- Functions to get project details
- Conditional loading (only if project-indicator is available)

### Performance Optimizations
- Caching for better responsiveness
- Directory change detection
- Minimal external calls

### Customization Options
- Multiple prompt styles (basic, detailed, minimal)
- Color themes
- Icon support
- JSON parsing for advanced use cases

### Framework Support
- Starship integration
- Oh-My-Zsh compatibility
- Powerlevel10k segments
- Powerline support

## Testing Your Integration

Each integration file includes a test function to verify everything works:

```bash
# Fish
test_project_indicator_integration

# Zsh/Bash
test_project_indicator_integration
```

## Performance Tips

1. **Use cached versions** - The cached prompt functions are recommended for daily use
2. **Avoid frequent calls** - Project info is cached per directory change
3. **JSON parsing** - Only use JSON output when you need structured data
4. **Error handling** - All examples include proper error handling with 2>/dev/null

## Customization Examples

### Custom Colors
```bash
# Modify the color codes in any integration file
# \e[32m = green, \e[34m = blue, \e[33m = yellow, etc.
```

### Custom Format
```bash
# Create your own format function
my_project_prompt() {
    local info=$(project-indicator --format json 2>/dev/null)
    # Parse and format as needed
}
```

### Conditional Display
```bash
# Only show in specific directories
project_prompt_conditional() {
    # Only show in ~/code directory
    [[ "$PWD" == "$HOME/code"* ]] && project_prompt
}
```