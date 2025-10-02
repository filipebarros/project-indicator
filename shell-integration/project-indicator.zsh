#!/usr/bin/env zsh

# Project Indicator Zsh Shell Integration
# High-performance shell prompt integration with intelligent caching

# Global variables for caching
typeset -g __PROJECT_INDICATOR_CACHE_DIR="${HOME}/.cache/project-indicator-zsh"
typeset -g __PROJECT_INDICATOR_CACHE_TTL=300  # 5 minutes in seconds
typeset -g __PROJECT_INDICATOR_LAST_PWD=""
typeset -g __PROJECT_INDICATOR_LAST_RESULT=""
typeset -g __PROJECT_INDICATOR_LAST_TIME=0

# Cache prewarming (runs detection in background on cd)
typeset -g __PROJECT_INDICATOR_PREWARM_ENABLED=0
typeset -g __PROJECT_INDICATOR_PREWARM_PID=0

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

    # Use zsh's built-in string manipulation for better performance
    clean_path="${path//[\/.]/_}"
    length=${#path}

    # Use available hash commands for better distribution
    if (( $+commands[md5sum] )); then
        hash=$(echo -n "$path" | md5sum | cut -d' ' -f1)
    elif (( $+commands[shasum] )); then
        hash=$(echo -n "$path" | shasum | cut -d' ' -f1)
    elif (( $+commands[md5] )); then
        # macOS md5 command
        hash=$(echo -n "$path" | md5)
    else
        # Fallback to zsh built-in arithmetic hash
        hash="${clean_path}_${length}_$(( [##16] ${#path} + ${path[(I)[a-z]]} ))"
    fi

    # Limit hash length and ensure it's valid
    echo "${hash:0:32}"
}

# Get file modification time (cross-platform, zsh optimized)
__project_indicator_get_mtime() {
    local file="$1"
    [[ -f "$file" ]] || { echo 0; return 1; }

    # Use zsh's stat builtin if available (zsh/stat module)
    if (( $+modules[zsh/stat] )); then
        local -a stat_result
        zstat -A stat_result +mtime "$file" 2>/dev/null && echo "$stat_result[1]" && return
    fi

    # Fallback to external stat command
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
    current_time=$EPOCHSECONDS

    # Check TTL
    (( current_time - cache_time > __PROJECT_INDICATOR_CACHE_TTL )) && return 1

    # Check if directory has been modified since cache
    dir_time=$(__project_indicator_get_mtime "$directory")
    (( dir_time > cache_time )) && return 1

    # Check if project files have been modified (using zsh arrays)
    local project_files=(package.json Cargo.toml pyproject.toml go.mod composer.json)
    local file
    for file in $project_files; do
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
    if (( $+commands[project-indicator] )); then
        result=$(project-indicator --format full "$directory" 2>/dev/null)
        local exit_code=$?

        # Only cache successful results
        if (( exit_code == 0 )) && [[ -n "$result" ]]; then
            local pwd_hash cache_file
            pwd_hash=$(__project_indicator_hash_pwd "$directory")
            cache_file=$(__project_indicator_cache_file "$pwd_hash")

            # Write to cache atomically
            echo "$result" >! "${cache_file}.tmp" 2>/dev/null && \
            mv "${cache_file}.tmp" "$cache_file" 2>/dev/null
        fi
    fi

    echo "$result"
}

# Cache prewarming - runs detection in background on directory change
__project_indicator_prewarm_start() {
    local directory="${1:-$PWD}"

    # Kill existing prewarm worker if any
    if (( __PROJECT_INDICATOR_PREWARM_PID )); then
        kill "$__PROJECT_INDICATOR_PREWARM_PID" 2>/dev/null
        wait "$__PROJECT_INDICATOR_PREWARM_PID" 2>/dev/null 2>&1
        __PROJECT_INDICATOR_PREWARM_PID=0
    fi

    # Run detection in background to populate cache
    (__project_indicator_execute "$directory" >/dev/null 2>&1) &
    __PROJECT_INDICATOR_PREWARM_PID=$!
}

# chpwd hook for directory change
__project_indicator_chpwd() {
    # Start prewarm worker if prewarm mode enabled
    if (( __PROJECT_INDICATOR_PREWARM_ENABLED )); then
        __project_indicator_prewarm_start "$PWD"
    fi
}

# Register chpwd hook if not already registered
if [[ -z "${chpwd_functions[(r)__project_indicator_chpwd]}" ]]; then
    chpwd_functions+=(__project_indicator_chpwd)
fi

# Main project indicator function with caching
__project_indicator_get() {
    local directory="${1:-$PWD}"
    local current_time pwd_hash cache_file cached_result

    # Initialize cache
    __project_indicator_init_cache

    current_time=$EPOCHSECONDS

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
        setopt local_options nullglob
        local cache_files=("$__PROJECT_INDICATOR_CACHE_DIR"/*.cache)
        if (( ${#cache_files[@]} > 0 )); then
            rm -f "${cache_files[@]}" 2>/dev/null
        fi
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
        "prewarm")
            if [[ "${2:-}" == "on" ]]; then
                __PROJECT_INDICATOR_PREWARM_ENABLED=1
                echo "Cache prewarming enabled (background cache refresh)"
            elif [[ "${2:-}" == "off" ]]; then
                __PROJECT_INDICATOR_PREWARM_ENABLED=0
                # Kill any running prewarm worker
                if (( __PROJECT_INDICATOR_PREWARM_PID )); then
                    kill "$__PROJECT_INDICATOR_PREWARM_PID" 2>/dev/null
                    wait "$__PROJECT_INDICATOR_PREWARM_PID" 2>/dev/null 2>&1
                    __PROJECT_INDICATOR_PREWARM_PID=0
                fi
                echo "Cache prewarming disabled"
            else
                if (( __PROJECT_INDICATOR_PREWARM_ENABLED )); then
                    echo "Cache prewarming: enabled"
                else
                    echo "Cache prewarming: disabled"
                fi
            fi
            ;;
        "status")
            local cache_count=0
            echo "Project Indicator Zsh Integration Status:"
            echo "Cache directory: $__PROJECT_INDICATOR_CACHE_DIR"
            echo "Cache TTL: $__PROJECT_INDICATOR_CACHE_TTL seconds"
            if (( __PROJECT_INDICATOR_PREWARM_ENABLED )); then
                echo "Cache prewarming: enabled"
            else
                echo "Cache prewarming: disabled"
            fi
            echo "Last directory: $__PROJECT_INDICATOR_LAST_PWD"
            if (( $+commands[project-indicator] )); then
                echo "Binary available: yes"
            else
                echo "Binary available: no"
            fi
            if [[ -d "$__PROJECT_INDICATOR_CACHE_DIR" ]]; then
                # Use nullglob to handle case when no cache files exist
                setopt local_options nullglob
                cache_files=("$__PROJECT_INDICATOR_CACHE_DIR"/*.cache)
                cache_count=${#cache_files[@]}
            fi
            echo "Cached directories: $cache_count"
            ;;
        "info")
            local pwd_hash cache_file cache_time current_time age ttl_remaining
            pwd_hash=$(__project_indicator_hash_pwd "$PWD")
            cache_file=$(__project_indicator_cache_file "$pwd_hash")

            if [[ -f "$cache_file" ]]; then
                cache_time=$(__project_indicator_get_mtime "$cache_file")
                current_time=$EPOCHSECONDS
                age=$((current_time - cache_time))
                ttl_remaining=$((__PROJECT_INDICATOR_CACHE_TTL - age))

                echo "Cache info for current directory:"
                if __project_indicator_cache_valid "$cache_file" "$PWD"; then
                    echo "  Status: valid"
                else
                    echo "  Status: expired"
                fi
                echo "  Age: ${age}s"
                echo "  TTL remaining: ${ttl_remaining}s"
                echo "  Content: $(<"$cache_file" 2>/dev/null)"
            else
                echo "No cache for current directory"
            fi
            ;;
        "why-slow")
            local pwd_hash cache_file cache_time current_time age file_time
            pwd_hash=$(__project_indicator_hash_pwd "$PWD")
            cache_file=$(__project_indicator_cache_file "$pwd_hash")

            if [[ ! -f "$cache_file" ]]; then
                echo "No cache exists for current directory (cold start)"
            elif ! __project_indicator_cache_valid "$cache_file" "$PWD"; then
                echo "Cache invalidated because:"
                cache_time=$(__project_indicator_get_mtime "$cache_file")

                # Check each project file
                local project_files=(package.json Cargo.toml pyproject.toml go.mod composer.json)
                local file
                for file in $project_files; do
                    if [[ -f "$file" ]]; then
                        file_time=$(__project_indicator_get_mtime "$file")
                        if (( file_time > cache_time )); then
                            echo "  - $file was modified"
                        fi
                    fi
                done

                # Check TTL expiry
                current_time=$EPOCHSECONDS
                age=$((current_time - cache_time))
                if (( age > __PROJECT_INDICATOR_CACHE_TTL )); then
                    echo "  - Cache TTL expired (age: ${age}s, TTL: ${__PROJECT_INDICATOR_CACHE_TTL}s)"
                fi
            else
                echo "Cache is valid (using cached result)"
            fi
            ;;
        *)
            echo "Usage: project_indicator_config [ttl SECONDS|prewarm on|off|status|info|why-slow]"
            echo ""
            echo "Commands:"
            echo "  ttl SECONDS      Set cache TTL (time-to-live) in seconds"
            echo "  prewarm on|off   Enable/disable cache prewarming (background refresh on cd)"
            echo "  status           Show current configuration and status"
            echo "  info             Show cache info for current directory"
            echo "  why-slow         Explain why detection is slow (cache status)"
            ;;
    esac
}

# Prompt integration functions - handles new format with individual component colors
__project_indicator_format() {
    local info="$1"
    if [[ -n "$info" ]]; then
        # Parse new format: icon1|color1•icon2|color2•icon3|color3
        local output=" %F{240}[%f"

        # Split components by bullet separator using zsh array splitting
        local -a components
        components=(${(s:•:)info})

        for component in $components; do
            if [[ "$component" == *"|"* ]]; then
                # Split icon and color using zsh parameter expansion
                local icon="${component%|*}"
                local color="${component#*|}"

                if [[ -n "$icon" && -n "$color" ]]; then
                    # Use hex color directly with zsh's true color support
                    # zsh supports hex colors directly with %F{#rrggbb}
                    output+="%F{${color}}${icon}%f"
                fi
            else
                # Fallback for malformed components
                output+="$component"
            fi
        done

        output+="%F{240}]%f"
        print -n "$output"
    fi
}

# Ready-to-use right prompt function
project_indicator_rprompt() {
    local info
    if (( __PROJECT_INDICATOR_ASYNC_ENABLED )); then
        info=$(__project_indicator_get_async)
    else
        info=$(__project_indicator_get)
    fi
    __project_indicator_format "$info"
}

# Function for left prompt integration
project_indicator_prompt() {
    local info
    if (( __PROJECT_INDICATOR_ASYNC_ENABLED )); then
        info=$(__project_indicator_get_async)
    else
        info=$(__project_indicator_get)
    fi
    __project_indicator_format "$info"
}

# Async worker functions
__project_indicator_async_worker() {
    local directory="$1"
    local result=$(__project_indicator_execute "$directory")
    echo "$result"
}

__project_indicator_async_callback() {
    local result
    if zpty -r -t project_indicator_worker result '*' 0 2>/dev/null; then
        __PROJECT_INDICATOR_ASYNC_RESULT="${result%$'\n'}"
        __PROJECT_INDICATOR_LAST_RESULT="$__PROJECT_INDICATOR_ASYNC_RESULT"
        __PROJECT_INDICATOR_LAST_PWD="$PWD"
        __PROJECT_INDICATOR_LAST_TIME=$EPOCHSECONDS

        # Kill the worker
        zpty -d project_indicator_worker 2>/dev/null
        __PROJECT_INDICATOR_ASYNC_PID=0

        # Trigger prompt refresh
        zle && zle reset-prompt
    fi
}

__project_indicator_async_start() {
    local directory="${1:-$PWD}"

    # Kill existing worker if any
    if (( __PROJECT_INDICATOR_ASYNC_PID )); then
        zpty -d project_indicator_worker 2>/dev/null
        __PROJECT_INDICATOR_ASYNC_PID=0
    fi

    # Start new async worker
    zpty -b project_indicator_worker __project_indicator_async_worker "$directory"
    __PROJECT_INDICATOR_ASYNC_PID=1
}

# Async get with fallback to sync
__project_indicator_get_async() {
    local directory="${1:-$PWD}"
    local current_time pwd_hash cache_file cached_result

    # Initialize cache
    __project_indicator_init_cache

    current_time=$EPOCHSECONDS

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

    # Start async execution if enabled
    if (( __PROJECT_INDICATOR_ASYNC_ENABLED )) && (( $+modules[zsh/zpty] )); then
        # Return cached result while async loads
        echo "$__PROJECT_INDICATOR_LAST_RESULT"

        # Start async worker if not already running
        if ! (( __PROJECT_INDICATOR_ASYNC_PID )); then
            __project_indicator_async_start "$directory"
        fi
        return
    fi

    # Fallback to synchronous execution
    local result
    result=$(__project_indicator_execute "$directory")

    # Update memory cache
    __PROJECT_INDICATOR_LAST_PWD="$directory"
    __PROJECT_INDICATOR_LAST_RESULT="$result"
    __PROJECT_INDICATOR_LAST_TIME="$current_time"

    echo "$result"
}

# Hook function for prompt updates
__project_indicator_precmd() {
    # Clear memory cache if directory changed (for performance)
    if [[ "$PWD" != "$__PROJECT_INDICATOR_LAST_PWD" ]]; then
        __PROJECT_INDICATOR_LAST_TIME=0
    fi

    # Check async callback if enabled
    if (( __PROJECT_INDICATOR_ASYNC_ENABLED )) && (( __PROJECT_INDICATOR_ASYNC_PID )); then
        __project_indicator_async_callback
    fi
}

# Auto-setup precmd hook if not already present
if ! (( ${precmd_functions[(I)__project_indicator_precmd]} )); then
    precmd_functions+=(__project_indicator_precmd)
fi

# Advanced zsh integration examples
# Uncomment to use as right prompt:
# export RPS1='$(project_indicator_rprompt)'

# Or integrate into existing RPROMPT:
# export RPS1='%*$(project_indicator_rprompt)'

# For oh-my-zsh themes, you can use:
# ZSH_THEME_PROJECT_INDICATOR_PREFIX=" %F{240}[%f"
# ZSH_THEME_PROJECT_INDICATOR_SUFFIX="%F{240}]%f"

# Load zsh modules for better performance if available
zmodload zsh/stat 2>/dev/null
zmodload zsh/datetime 2>/dev/null
zmodload zsh/zpty 2>/dev/null  # For async support

# Initialization
__project_indicator_init_cache

# Silent fallback if binary not available
if ! (( $+commands[project-indicator] )); then
    # Define no-op stub functions (silent mode)
    project_info() { echo ""; }
    project_indicator_prompt() { echo ""; }
    project_indicator_rprompt() { echo ""; }
    return 0  # Exit integration script silently
fi
