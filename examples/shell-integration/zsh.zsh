# Zsh integration for project-indicator
# Add this to your ~/.zshrc

# Basic right prompt
RPS1='$(project_info=$(project-indicator 2>/dev/null); [[ -n "$project_info" ]] && echo "%F{8}[%f${project_info}%F{8}]%f")'

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

# More detailed right prompt
RPS1_DETAILED='$(
    project_info=$(project-indicator --format json 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        language=$(echo "$project_info" | jq -r ".language.name // empty")
        framework=$(echo "$project_info" | jq -r ".frameworks[0].name // empty")
        if [[ -n "$language" ]]; then
            output="%F{blue}$language%f"
            if [[ -n "$framework" ]]; then
                output="$output%F{8} · %f%F{green}$framework%f"
            fi
            echo "%F{8}[%f${output}%F{8}]%f"
        fi
    fi
)'

# Left prompt with project info
PS1_WITH_PROJECT='%F{green}%n%f@%F{blue}%m%f:%F{yellow}%~%f$(
    project_info=$(project-indicator 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo " %F{8}(%f${project_info}%F{8})%f"
    fi
)$(git_prompt_info) %F{green}❯%f '

# Git prompt info function (requires oh-my-zsh or similar)
git_prompt_info() {
    if git rev-parse --git-dir >/dev/null 2>&1; then
        local branch=$(git branch --show-current 2>/dev/null)
        if [[ -n "$branch" ]]; then
            echo " %F{magenta}($branch)%f"
        fi
    fi
}

# Performance optimized version with caching
typeset -g __project_indicator_cache_dir=""
typeset -g __project_indicator_cache_result=""

__get_cached_project_info() {
    local current_dir="$PWD"
    if [[ "$current_dir" != "$__project_indicator_cache_dir" ]]; then
        __project_indicator_cache_dir="$current_dir"
        __project_indicator_cache_result=$(project-indicator 2>/dev/null)
    fi
    echo "$__project_indicator_cache_result"
}

# Cached right prompt for better performance
RPS1_CACHED='$(
    project_info=$(__get_cached_project_info)
    [[ -n "$project_info" ]] && echo "%F{8}[%f${project_info}%F{8}]%f"
)'

# Starship-style integration
# If you use Starship, add this to your starship.toml:
# [custom.project_indicator]
# command = "project-indicator"
# when = true
# format = "[$output]($style) "
# style = "bold blue"

# Conditional loading - only enable if project-indicator is available
if command -v project-indicator >/dev/null 2>&1; then
    # Uncomment the prompt you want to use
    # RPS1="$RPS1_DETAILED"
    # PS1="$PS1_WITH_PROJECT"
    # RPS1="$RPS1_CACHED"  # Recommended for performance
fi

# Oh-My-Zsh plugin style
project_indicator_prompt() {
    local project_info=$(project-indicator 2>/dev/null)
    if [[ -n "$project_info" ]]; then
        echo "%{$fg[white]%}[%{$reset_color%}${project_info}%{$fg[white]%}]%{$reset_color%}"
    fi
}

# Powerlevel10k integration
# Add to your .p10k.zsh:
# typeset -g POWERLEVEL9K_RIGHT_PROMPT_ELEMENTS=(
#     status
#     command_execution_time
#     background_jobs
#     direnv
#     asdf
#     virtualenv
#     anaconda
#     pyenv
#     goenv
#     nodenv
#     nvm
#     nodeenv
#     rbenv
#     rvm
#     fvm
#     luaenv
#     jenv
#     plenv
#     phpenv
#     scalaenv
#     haskell_stack
#     kubecontext
#     terraform
#     aws
#     aws_eb_env
#     azure
#     gcloud
#     google_app_cred
#     context
#     nordvpn
#     ranger
#     nnn
#     vim_shell
#     midnight_commander
#     nix_shell
#     custom_project_indicator  # Add this
#     time
# )
#
# function prompt_custom_project_indicator() {
#     local project_info=$(project-indicator 2>/dev/null)
#     if [[ -n "$project_info" ]]; then
#         p10k segment -f 8 -i '' -t "$project_info"
#     fi
# }

# Simple function for manual use
pinfo() {
    project-indicator "$@"
}