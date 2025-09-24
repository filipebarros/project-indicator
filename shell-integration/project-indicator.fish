# Project Indicator Fish Shell Integration
# High-performance shell prompt integration with intelligent caching

# Global variables for caching
set -g __project_indicator_cache_dir "$HOME/.cache/project-indicator-fish"
set -g __project_indicator_cache_ttl 300  # 5 minutes in seconds
set -g __project_indicator_last_pwd ""
set -g __project_indicator_last_result ""
set -g __project_indicator_last_time 0

# Initialize cache directory
function __project_indicator_init_cache
    if not test -d "$__project_indicator_cache_dir"
        mkdir -p "$__project_indicator_cache_dir" 2>/dev/null
    end
end

# Get cache file path for a directory
function __project_indicator_cache_file -a pwd_hash
    echo "$__project_indicator_cache_dir/$pwd_hash.cache"
end

# Generate hash for directory path
function __project_indicator_hash_pwd -a path
    # Use a better hash function - combine multiple methods for uniqueness
    set -l clean_path (string replace -ra '[/.]' '_' -- "$path")
    set -l length (string length -- "$path")

    # Create a more unique hash by combining path hash with length
    printf '%s_%d' "$clean_path" "$length" | string sub -l 50
end

# Check if cache is valid based on directory modification time
function __project_indicator_cache_valid -a cache_file directory
    if not test -f "$cache_file"
        return 1
    end

    # Get cache modification time (improved cross-platform compatibility)
    set cache_time 0
    if command -q stat
        # Try GNU stat format first (Linux), then BSD stat format (macOS)
        set cache_time (stat -c %Y "$cache_file" 2>/dev/null; or stat -f %m "$cache_file" 2>/dev/null; or echo 0)
    end
    set current_time (date +%s)

    # Check TTL
    if test (math "$current_time - $cache_time") -gt $__project_indicator_cache_ttl
        return 1
    end

    # Check if directory has been modified since cache
    set dir_time 0
    if command -q stat
        set dir_time (stat -c %Y "$directory" 2>/dev/null; or stat -f %m "$directory" 2>/dev/null; or echo 0)
    end
    if test "$dir_time" -gt "$cache_time"
        return 1
    end

    # Check if project files have been modified (quick check for common files)
    for file in package.json Cargo.toml pyproject.toml go.mod composer.json
        if test -f "$directory/$file"
            set file_time (stat -c %Y "$directory/$file" 2>/dev/null; or stat -f %m "$directory/$file" 2>/dev/null; or echo 0)
            if test "$file_time" -gt "$cache_time"
                return 1
            end
        end
    end

    return 0
end

# Execute project-indicator and cache result
function __project_indicator_execute -a directory
    set -l result ""

    # Validate directory exists and is readable
    if not test -d "$directory"
        return 1
    end

    if not test -r "$directory"
        return 1
    end

    # Try to execute project-indicator with timeout protection
    if command -q project-indicator
        # Use timeout if available to prevent hanging
        if command -q timeout
            set result (timeout 10s project-indicator --format full "$directory" 2>/dev/null)
        else
            set result (project-indicator --format full "$directory" 2>/dev/null)
        end
        set -l exit_code $status

        # Only cache successful results
        if test $exit_code -eq 0 -a -n "$result"
            set -l pwd_hash (__project_indicator_hash_pwd "$directory")
            set -l cache_file (__project_indicator_cache_file "$pwd_hash")

            # Ensure cache directory exists
            set -l cache_dir (dirname "$cache_file")
            if not test -d "$cache_dir"
                mkdir -p "$cache_dir" 2>/dev/null
            end

            # Write to cache atomically with error handling
            if echo "$result" > "$cache_file.tmp" 2>/dev/null
                mv "$cache_file.tmp" "$cache_file" 2>/dev/null
            end
        end
    end

    echo "$result"
end

# Main project indicator function with caching
function __project_indicator_get -a directory
    # Use current directory if none provided
    set directory (test -n "$directory"; and echo "$directory"; or pwd)

    # Initialize cache
    __project_indicator_init_cache

    # Memory cache check (for same directory)
    set current_time (date +%s)
    if test "$directory" = "$__project_indicator_last_pwd" \
       -a (math "$current_time - $__project_indicator_last_time") -lt 30
        echo "$__project_indicator_last_result"
        return
    end

    # Disk cache check
    set -l pwd_hash (__project_indicator_hash_pwd "$directory")
    set -l cache_file (__project_indicator_cache_file "$pwd_hash")

    if __project_indicator_cache_valid "$cache_file" "$directory"
        set -l cached_result (cat "$cache_file" 2>/dev/null)
        if test -n "$cached_result"
            # Update memory cache
            set -g __project_indicator_last_pwd "$directory"
            set -g __project_indicator_last_result "$cached_result"
            set -g __project_indicator_last_time "$current_time"

            echo "$cached_result"
            return
        end
    end

    # Execute and cache
    set -l result (__project_indicator_execute "$directory")

    # Update memory cache
    set -g __project_indicator_last_pwd "$directory"
    set -g __project_indicator_last_result "$result"
    set -g __project_indicator_last_time "$current_time"

    echo "$result"
end

# Public function for getting project info
function project_info -a directory
    __project_indicator_get "$directory"
end

# Clear project indicator cache
function project_indicator_clear_cache
    if test -d "$__project_indicator_cache_dir"
        rm -rf "$__project_indicator_cache_dir"/*
        echo "Project indicator cache cleared"
    end

    # Clear memory cache
    set -g __project_indicator_last_pwd ""
    set -g __project_indicator_last_result ""
    set -g __project_indicator_last_time 0
end

# Right prompt function (example usage) - handles new format
function fish_right_prompt_project_indicator
    set -l project_info (__project_indicator_get)
    if test -n "$project_info"
        set_color brblack
        echo -n "["
        set_color normal

        # Parse new format: icon1|color1•icon2|color2•icon3|color3
        set -l components (string split "•" $project_info)

        for component in $components
            set -l parts (string split "|" $component)
            if test (count $parts) -eq 2
                set -l icon $parts[1]
                set -l color $parts[2]

                if test -n "$color" -a -n "$icon"
                    set_color $color
                    echo -n $icon
                    set_color normal
                end
            else
                # Fallback for malformed components
                echo -n $component
            end
        end

        set_color brblack
        echo -n "]"
        set_color normal
    end
end

# Alternative: Function to add project info to existing right prompt - handles new format
function __project_indicator_format -a info
    if test -n "$info"
        set_color brblack
        echo -n " ["
        set_color normal

        # Parse new format: icon1|color1•icon2|color2•icon3|color3
        set -l components (string split "•" $info)

        for component in $components
            set -l parts (string split "|" $component)
            if test (count $parts) -eq 2
                set -l icon $parts[1]
                set -l color $parts[2]

                if test -n "$color" -a -n "$icon"
                    set_color $color
                    echo -n $icon
                    set_color normal
                end
            else
                # Fallback for malformed components
                echo -n $component
            end
        end

        set_color brblack
        echo -n "]"
        set_color normal
    end
end

# Configuration function
function project_indicator_config
    switch "$argv[1]"
        case "ttl"
            if test -n "$argv[2]"
                set -g __project_indicator_cache_ttl "$argv[2]"
                echo "Cache TTL set to $argv[2] seconds"
            else
                echo "Current TTL: $__project_indicator_cache_ttl seconds"
            end
        case "status"
            echo "Project Indicator Fish Integration Status:"
            echo "Cache directory: $__project_indicator_cache_dir"
            echo "Cache TTL: $__project_indicator_cache_ttl seconds"
            echo "Last directory: $__project_indicator_last_pwd"
            echo "Binary available: "(command -q project-indicator; and echo "yes"; or echo "no")
            if test -d "$__project_indicator_cache_dir"
                set cache_files (count "$__project_indicator_cache_dir"/*.cache 2>/dev/null)
                echo "Cached directories: $cache_files"
            else
                echo "Cached directories: 0"
            end
        case "*"
            echo "Usage: project_indicator_config [ttl SECONDS|status]"
            echo ""
            echo "Commands:"
            echo "  ttl SECONDS  Set cache TTL (time-to-live) in seconds"
            echo "  status       Show current configuration and status"
    end
end

# Event handler to clear memory cache on directory change
# This helps prevent stale results when switching between directories quickly
function __project_indicator_pwd_changed --on-variable PWD
    # Only clear if we're moving to a different directory
    if test "$PWD" != "$__project_indicator_last_pwd"
        # Keep the cache but mark it as potentially stale
        set -g __project_indicator_last_time 0
    end
end

# Initialization
__project_indicator_init_cache

# Validation and error handling
function __project_indicator_validate_setup
    # Check if project-indicator binary is available
    if not command -q project-indicator
        echo "Warning: project-indicator binary not found in PATH" >&2
        echo "Install it with: cargo install project-indicator" >&2
        echo "Or download from: https://github.com/filipebarros/project-indicator" >&2
        return 1
    end

    # Test if binary is working
    if not project-indicator --help >/dev/null 2>&1
        echo "Error: project-indicator binary found but not working correctly" >&2
        return 1
    end

    return 0
end

# Check setup on load but don't fail
if not __project_indicator_validate_setup
    # Define fallback functions that do nothing
    function project_info -a directory
        echo ""
    end

    function fish_right_prompt_project_indicator
        # Return empty string for broken setups
    end
end