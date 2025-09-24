#!/bin/bash

# Project Indicator Bash Shell Integration
# High-performance shell prompt integration with intelligent caching

# Global variables for caching (compatible with Bash 3.2+)
__PROJECT_INDICATOR_CACHE_DIR="${HOME}/.cache/project-indicator-bash"
__PROJECT_INDICATOR_CACHE_TTL=300  # 5 minutes in seconds
__PROJECT_INDICATOR_LAST_PWD=""
__PROJECT_INDICATOR_LAST_RESULT=""
__PROJECT_INDICATOR_LAST_TIME=0

# Initialize cache directory
__project_indicator_init_cache() {
    [[ -d "$__PROJECT_INDICATOR_CACHE_DIR" ]] || mkdir -p "$__PROJECT_INDICATOR_CACHE_DIR" 2>/dev/null
}

# Get cache file path for a directory
__project_indicator_cache_file() {
    local pwd_hash="$1"
    echo "$__PROJECT_INDICATOR_CACHE_DIR/${pwd_hash}.cache"
}

# Generate hash for directory path
__project_indicator_hash_pwd() {
    local path="$1"
    local clean_path length hash

    # Replace problematic characters and create a unique hash
    clean_path=$(printf '%s' "$path" | sed 's/[/.]/_/g')
    length=${#path}

    # Use md5/shasum if available for better hashing, fallback to simple method
    if command -v md5sum >/dev/null 2>&1; then
        hash=$(printf '%s' "$path" | md5sum | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        hash=$(printf '%s' "$path" | shasum | awk '{print $1}')
    else
        # Fallback to simple hash with length
        hash="${clean_path}_${length}"
    fi

    # Limit hash length using parameter expansion (more portable)
    echo "${hash:0:32}"
}

# Get file modification time (cross-platform)
__project_indicator_get_mtime() {
    local file="$1"
    [[ -f "$file" ]] || { echo 0; return 1; }

    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS
        stat -f %m "$file" 2>/dev/null || echo 0
    else
        # Linux and others
        stat -c %Y "$file" 2>/dev/null || echo 0
    fi
}

# Check if cache is valid based on directory modification time
__project_indicator_cache_valid() {
    local cache_file="$1" directory="$2"
    [[ -f "$cache_file" ]] || return 1

    local cache_time current_time dir_time file_time
    cache_time=$(__project_indicator_get_mtime "$cache_file")
    current_time=$(date +%s)

    # Check TTL
    (( current_time - cache_time > __PROJECT_INDICATOR_CACHE_TTL )) && return 1

    # Check if directory has been modified since cache
    dir_time=$(__project_indicator_get_mtime "$directory")
    (( dir_time > cache_time )) && return 1

    # Check if project files have been modified (quick check for common files)
    local project_files=("package.json" "Cargo.toml" "pyproject.toml" "go.mod" "composer.json")
    for file in "${project_files[@]}"; do
        if [[ -f "$directory/$file" ]]; then
            file_time=$(__project_indicator_get_mtime "$directory/$file")
            (( file_time > cache_time )) && return 1
        fi
    done

    return 0
}

# Execute project-indicator and cache result
__project_indicator_execute() {
    local directory="$1"
    local result=""

    # Try to execute project-indicator
    if command -v project-indicator >/dev/null 2>&1; then
        result=$(project-indicator --format full "$directory" 2>/dev/null)
        local exit_code=$?

        # Only cache successful results
        if (( exit_code == 0 )) && [[ -n "$result" ]]; then
            local pwd_hash cache_file
            pwd_hash=$(__project_indicator_hash_pwd "$directory")
            cache_file=$(__project_indicator_cache_file "$pwd_hash")

            # Write to cache atomically
            echo "$result" > "${cache_file}.tmp" 2>/dev/null && \
            mv "${cache_file}.tmp" "$cache_file" 2>/dev/null
        fi
    fi

    echo "$result"
}

# Main project indicator function with caching
__project_indicator_get() {
    local directory="${1:-$PWD}"
    local current_time pwd_hash cache_file cached_result

    # Initialize cache
    __project_indicator_init_cache

    current_time=$(date +%s)

    # Memory cache check (for same directory)
    if [[ "$directory" == "$__PROJECT_INDICATOR_LAST_PWD" ]] && \
       (( current_time - __PROJECT_INDICATOR_LAST_TIME < 30 )); then
        echo "$__PROJECT_INDICATOR_LAST_RESULT"
        return
    fi

    # Disk cache check
    pwd_hash=$(__project_indicator_hash_pwd "$directory")
    cache_file=$(__project_indicator_cache_file "$pwd_hash")

    if __project_indicator_cache_valid "$cache_file" "$directory"; then
        cached_result=$(<"$cache_file" 2>/dev/null)
        if [[ -n "$cached_result" ]]; then
            # Update memory cache
            __PROJECT_INDICATOR_LAST_PWD="$directory"
            __PROJECT_INDICATOR_LAST_RESULT="$cached_result"
            __PROJECT_INDICATOR_LAST_TIME="$current_time"

            echo "$cached_result"
            return
        fi
    fi

    # Execute and cache
    local result
    result=$(__project_indicator_execute "$directory")

    # Update memory cache
    __PROJECT_INDICATOR_LAST_PWD="$directory"
    __PROJECT_INDICATOR_LAST_RESULT="$result"
    __PROJECT_INDICATOR_LAST_TIME="$current_time"

    echo "$result"
}

# Public function for getting project info
project_info() {
    __project_indicator_get "$1"
}

# Clear project indicator cache
project_indicator_clear_cache() {
    if [[ -d "$__PROJECT_INDICATOR_CACHE_DIR" ]]; then
        rm -f "$__PROJECT_INDICATOR_CACHE_DIR"/*.cache 2>/dev/null
        echo "Project indicator cache cleared"
    fi

    # Clear memory cache
    __PROJECT_INDICATOR_LAST_PWD=""
    __PROJECT_INDICATOR_LAST_RESULT=""
    __PROJECT_INDICATOR_LAST_TIME=0
}

# Configuration function
project_indicator_config() {
    case "${1:-}" in
        "ttl")
            if [[ -n "${2:-}" ]]; then
                __PROJECT_INDICATOR_CACHE_TTL="$2"
                echo "Cache TTL set to $2 seconds"
            else
                echo "Current TTL: $__PROJECT_INDICATOR_CACHE_TTL seconds"
            fi
            ;;
        "status")
            local cache_count=0
            echo "Project Indicator Bash Integration Status:"
            echo "Cache directory: $__PROJECT_INDICATOR_CACHE_DIR"
            echo "Cache TTL: $__PROJECT_INDICATOR_CACHE_TTL seconds"
            echo "Last directory: $__PROJECT_INDICATOR_LAST_PWD"
            if command -v project-indicator >/dev/null 2>&1; then
                echo "Binary available: yes"
            else
                echo "Binary available: no"
            fi
            if [[ -d "$__PROJECT_INDICATOR_CACHE_DIR" ]]; then
                cache_count=$(find "$__PROJECT_INDICATOR_CACHE_DIR" -name "*.cache" 2>/dev/null | wc -l)
            fi
            echo "Cached directories: $cache_count"
            ;;
        *)
            echo "Usage: project_indicator_config [ttl SECONDS|status]"
            echo ""
            echo "Commands:"
            echo "  ttl SECONDS  Set cache TTL (time-to-live) in seconds"
            echo "  status       Show current configuration and status"
            ;;
    esac
}

# Convert hex color to RGB values for ANSI escape sequence
__project_indicator_hex_to_rgb() {
    local hex="$1"
    # Remove # if present
    hex="${hex#\#}"

    # Extract RGB components
    local r=$((16#${hex:0:2}))
    local g=$((16#${hex:2:2}))
    local b=$((16#${hex:4:2}))

    echo "$r;$g;$b"
}

# Prompt integration functions - handles new format with individual component colors
__project_indicator_format() {
    local info="$1"
    if [[ -n "$info" ]]; then
        # Parse new format: icon1|color1•icon2|color2•icon3|color3
        local output=" \[\033[90m\][\[\033[0m\]"

        # Use parameter expansion to split on bullet character
        local remaining="$info"
        while [[ -n "$remaining" ]]; do
            local component=""
            if [[ "$remaining" == *"•"* ]]; then
                component="${remaining%%•*}"
                remaining="${remaining#*•}"
            else
                component="$remaining"
                remaining=""
            fi

            if [[ "$component" == *"|"* ]]; then
                # Split icon and color
                local icon="${component%|*}"
                local color="${component#*|}"

                if [[ -n "$icon" && -n "$color" ]]; then
                    # Convert hex color to RGB for true color support
                    local rgb
                    rgb=$(__project_indicator_hex_to_rgb "$color")

                    # Use true color ANSI escape sequence
                    output+="\[\033[38;2;${rgb}m\]${icon}\[\033[0m\]"
                fi
            else
                # Fallback for malformed components
                output+="$component"
            fi
        done

        output+="\[\033[90m\]]\[\033[0m\]"
        printf '%s' "$output"
    fi
}

# Ready-to-use right prompt function for PS1
__project_indicator_ps1() {
    local info
    info=$(__project_indicator_get)
    __project_indicator_format "$info"
}

# Example PS1 integration (uncomment to use)
# export PS1='\u@\h:\w$(__project_indicator_ps1)\$ '

# Alternative: function to get formatted project info for custom prompts
project_indicator_prompt() {
    local info
    info=$(__project_indicator_get)
    __project_indicator_format "$info"
}

# PROMPT_COMMAND integration for dynamic prompts
__project_indicator_prompt_command() {
    # Clear memory cache if directory changed (for performance)
    if [[ "$PWD" != "$__PROJECT_INDICATOR_LAST_PWD" ]]; then
        __PROJECT_INDICATOR_LAST_TIME=0
    fi
}

# Auto-setup PROMPT_COMMAND if not already set
if [[ "$PROMPT_COMMAND" != *"__project_indicator_prompt_command"* ]]; then
    if [[ -n "$PROMPT_COMMAND" ]]; then
        PROMPT_COMMAND="$PROMPT_COMMAND; __project_indicator_prompt_command"
    else
        PROMPT_COMMAND="__project_indicator_prompt_command"
    fi
fi

# Initialization
__project_indicator_init_cache

# Check if project-indicator is available and show warning if not
if ! command -v project-indicator >/dev/null 2>&1; then
    echo "Warning: project-indicator binary not found in PATH" >&2
    echo "Install it from: https://github.com/filipebarros/project-indicator" >&2
fi