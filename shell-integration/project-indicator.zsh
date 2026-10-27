# Project Indicator — zsh prompt hook
#
# Install: source this file from ~/.zshrc. It appends the indicator to
# RPROMPT; adapt project_indicator_prompt into your own prompt if you
# already customize RPROMPT.

project_indicator_prompt() {
    command -v project-indicator >/dev/null 2>&1 || return
    local info rest wrapped=""
    info="$(project-indicator 2>/dev/null)"
    # %{ %} marks every escape sequence as zero-width; visible glyphs stay
    # outside, and literal percent signs must be doubled.
    rest=$info
    local re=$'^([^\e]*)(\e\\[[0-9;]*m)(.*)$'
    while [[ "$rest" =~ $re ]]; do
        wrapped+="${match[1]//\%/%%}%{${match[2]}%}"
        rest="${match[3]}"
    done
    info="${wrapped}${rest//\%/%%}"
    [[ -n "$info" ]] && printf '%%F{8}[%%f%s%%F{8}]%%f' "$info"
}

setopt PROMPT_SUBST
# Idempotent: re-sourcing ~/.zshrc must not stack the hook
[[ "$RPROMPT" == *project_indicator_prompt* ]] ||
    RPROMPT='$(project_indicator_prompt)'"${RPROMPT:+ $RPROMPT}"
