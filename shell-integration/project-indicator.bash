# Project Indicator — bash prompt hook
#
# Caching is handled by the binary itself (persistent, mtime-invalidated,
# ~0.2ms warm hits), so this hook just calls it.
#
# Install: source this file from ~/.bashrc. It appends the indicator to PS1;
# call project_indicator_prompt from your own PROMPT_COMMAND if you already
# customize the prompt.

project_indicator_prompt() {
    command -v project-indicator >/dev/null 2>&1 || return
    local info
    info="$(project-indicator 2>/dev/null)"
    [[ -n "$info" ]] && printf '\001\033[90m\002[\001\033[0m\002%s\001\033[90m\002]\001\033[0m\002' "$info"
}

PS1="${PS1}\$(project_indicator_prompt) "
