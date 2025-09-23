# Bash integration for project-indicator
# Add this to your ~/.bashrc

# Function to get project info
get_project_info() {
    project-indicator 2>/dev/null
}

# Function to get project language only
get_project_language() {
    local project_info=$(project-indicator --format json 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo "$project_info" | jq -r '.language.name // empty'
    fi
}

# Function to get project framework only
get_project_framework() {
    local project_info=$(project-indicator --format json 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo "$project_info" | jq -r '.frameworks[0].name // empty'
    fi
}

# Basic project prompt function
project_prompt() {
    local project_info=$(project-indicator 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo -e "\e[90m[\e[0m${project_info}\e[90m]\e[0m"
    fi
}

# Detailed project prompt function
project_prompt_detailed() {
    local project_info=$(project-indicator --format json 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        local language=$(echo "$project_info" | jq -r '.language.name // empty')
        local framework=$(echo "$project_info" | jq -r '.frameworks[0].name // empty')

        if [[ -n "$language" ]]; then
            local output="\e[34m$language\e[0m"
            if [[ -n "$framework" ]]; then
                output="$output\e[90m · \e[0m\e[32m$framework\e[0m"
            fi
            echo -e "\e[90m[\e[0m${output}\e[90m]\e[0m"
        fi
    fi
}

# Git branch function (if not already defined)
git_branch() {
    local branch=$(git branch --show-current 2>/dev/null)
    if [[ -n "$branch" ]]; then
        echo -e "\e[35m($branch)\e[0m"
    fi
}

# Simple PS1 with project info
PS1_WITH_PROJECT='\[\e[32m\]\u\[\e[0m\]@\[\e[34m\]\h\[\e[0m\]:\[\e[33m\]\w\[\e[0m\]$(project_prompt)$(git_branch) \[\e[32m\]❯\[\e[0m\] '

# Detailed PS1 with project info
PS1_DETAILED='\[\e[32m\]\u\[\e[0m\]@\[\e[34m\]\h\[\e[0m\]:\[\e[33m\]\w\[\e[0m\]$(project_prompt_detailed)$(git_branch) \[\e[32m\]❯\[\e[0m\] '

# Performance optimized version with caching
__project_indicator_cache_dir=""
__project_indicator_cache_result=""

__get_cached_project_info() {
    local current_dir="$PWD"
    if [[ "$current_dir" != "$__project_indicator_cache_dir" ]]; then
        __project_indicator_cache_dir="$current_dir"
        __project_indicator_cache_result=$(project-indicator 2>/dev/null)
    fi
    echo "$__project_indicator_cache_result"
}

# Cached project prompt for better performance
project_prompt_cached() {
    local project_info=$(__get_cached_project_info)
    if [[ -n "$project_info" ]]; then
        echo -e "\e[90m[\e[0m${project_info}\e[90m]\e[0m"
    fi
}

# Cached PS1
PS1_CACHED='\[\e[32m\]\u\[\e[0m\]@\[\e[34m\]\h\[\e[0m\]:\[\e[33m\]\w\[\e[0m\]$(project_prompt_cached)$(git_branch) \[\e[32m\]❯\[\e[0m\] '

# Starship-style prompt setup
setup_starship_project_indicator() {
    cat << 'EOF'
Add this to your ~/.config/starship.toml:

[custom.project_indicator]
command = "project-indicator"
when = true
format = "[$output]($style) "
style = "bold blue"
EOF
}

# For powerline/airline users - JSON output for parsing
project_powerline_segment() {
    local project_info=$(project-indicator --format json 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        # Format for powerline: {"contents": "text", "highlight_groups": ["group"]}
        echo "$project_info" | jq -r '"project:" + (.language.name // "unknown") + if .frameworks[0].name then ":" + .frameworks[0].name else "" end'
    fi
}

# Simple alias for quick access
alias pinfo='project-indicator'
alias plang='get_project_language'
alias pframework='get_project_framework'

# Conditional loading - only enable if project-indicator is available
if command -v project-indicator >/dev/null 2>&1; then
    # Uncomment the PS1 you want to use
    # PS1="$PS1_WITH_PROJECT"
    # PS1="$PS1_DETAILED"
    # PS1="$PS1_CACHED"  # Recommended for performance

    # Or use PROMPT_COMMAND for more flexibility
    # PROMPT_COMMAND='PS1="\[\e[32m\]\u\[\e[0m\]@\[\e[34m\]\h\[\e[0m\]:\[\e[33m\]\w\[\e[0m\]$(project_prompt_cached) \[\e[32m\]❯\[\e[0m\] "'
fi

# Integration with existing prompt frameworks
# For bash-git-prompt users
if [[ -n "$GIT_PROMPT_THEME" ]]; then
    GIT_PROMPT_START_USER='\[\e[32m\]\u\[\e[0m\]@\[\e[34m\]\h\[\e[0m\]:\[\e[33m\]\w\[\e[0m\]$(project_prompt_cached)'
fi

# For liquidprompt users
if [[ -n "$LP_ENABLE_PROXY" ]]; then
    LP_PS1_POSTFIX='$(project_prompt_cached)'
fi

# Function to test the integration
test_project_indicator_integration() {
    echo "Testing project-indicator integration..."

    if command -v project-indicator >/dev/null 2>&1; then
        echo "✓ project-indicator is available"

        local test_output=$(project-indicator 2>/dev/null)
        if [[ $? -eq 0 ]]; then
            echo "✓ project-indicator works: $test_output"
        else
            echo "✗ project-indicator failed to run"
        fi

        if command -v jq >/dev/null 2>&1; then
            echo "✓ jq is available for JSON parsing"
        else
            echo "⚠ jq not found - JSON features will not work"
        fi
    else
        echo "✗ project-indicator not found in PATH"
    fi
}

# Uncomment to test on shell startup
# test_project_indicator_integration