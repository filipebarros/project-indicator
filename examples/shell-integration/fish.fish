# Fish shell integration for project-indicator
# Add this to your ~/.config/fish/config.fish

# Basic right prompt with project indicator
function fish_right_prompt
    set -l project_info (project-indicator 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        echo -n (set_color brblack)"["(set_color normal)$project_info(set_color brblack)"]"(set_color normal)
    end
end

# Alternative: More detailed right prompt
function fish_right_prompt_detailed
    set -l project_info (project-indicator --format json 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        set -l language (echo $project_info | jq -r '.language.name // empty')
        set -l framework (echo $project_info | jq -r '.frameworks[0].name // empty')

        if test -n "$language"
            set -l output (set_color blue)$language(set_color normal)
            if test -n "$framework"
                set output $output(set_color brblack)" · "(set_color normal)(set_color green)$framework(set_color normal)
            end
            echo -n (set_color brblack)"["(set_color normal)$output(set_color brblack)"]"(set_color normal)
        end
    end
end

# Function to get project info for use in other contexts
function get_project_info
    project-indicator 2>/dev/null
end

# Function to get project language only
function get_project_language
    set -l project_info (project-indicator --format json 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        echo $project_info | jq -r '.language.name // empty'
    end
end

# Function to get project framework only
function get_project_framework
    set -l project_info (project-indicator --format json 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        echo $project_info | jq -r '.frameworks[0].name // empty'
    end
end

# Custom prompt that includes project info in the left prompt
function fish_prompt_with_project
    set -l last_status $status

    # User and host
    echo -n (set_color green)(whoami)(set_color normal)"@"(set_color blue)(hostname -s)(set_color normal)":"

    # Current directory
    echo -n (set_color yellow)(prompt_pwd)(set_color normal)

    # Project info
    set -l project_info (project-indicator 2>/dev/null)
    if test $status -eq 0; and test -n "$project_info"
        echo -n " "(set_color brblack)"("(set_color normal)$project_info(set_color brblack)")"(set_color normal)
    end

    # Git status (if you have fish_git_prompt)
    if functions -q fish_git_prompt
        echo -n (fish_git_prompt)
    end

    # Prompt character
    if test $last_status -eq 0
        echo -n (set_color green)" ❯ "(set_color normal)
    else
        echo -n (set_color red)" ❯ "(set_color normal)
    end
end

# Conditional loading - only enable if project-indicator is available
if command -v project-indicator >/dev/null 2>&1
    # Uncomment the function you want to use
    # functions -e fish_right_prompt; and functions -c fish_right_prompt_basic fish_right_prompt
    # functions -e fish_prompt; and functions -c fish_prompt_with_project fish_prompt
end

# Performance tip: Cache project info for the current directory
set -g __project_indicator_cache_dir ""
set -g __project_indicator_cache_result ""

function __get_cached_project_info
    set -l current_dir (pwd)
    if test "$current_dir" != "$__project_indicator_cache_dir"
        set -g __project_indicator_cache_dir $current_dir
        set -g __project_indicator_cache_result (project-indicator 2>/dev/null)
    end
    echo $__project_indicator_cache_result
end

# Cached version of right prompt for better performance
function fish_right_prompt_cached
    set -l project_info (__get_cached_project_info)
    if test -n "$project_info"
        echo -n (set_color brblack)"["(set_color normal)$project_info(set_color brblack)"]"(set_color normal)
    end
end