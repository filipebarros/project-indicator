# Project Indicator — zsh prompt hook
#
# Caching is handled by the binary itself (persistent, mtime-invalidated,
# ~0.2ms warm hits), so this hook just calls it.
#
# Install: source this file from ~/.zshrc. It appends the indicator to
# RPROMPT; adapt project_indicator_prompt into your own prompt if you
# already customize RPROMPT.

project_indicator_prompt() {
    command -v project-indicator >/dev/null 2>&1 || return
    local info
    info="$(project-indicator 2>/dev/null)"
    [[ -n "$info" ]] && printf '%%F{8}[%%f%s%%F{8}]%%f' "$info"
}

setopt PROMPT_SUBST
RPROMPT='$(project_indicator_prompt)'"${RPROMPT:+ $RPROMPT}"
