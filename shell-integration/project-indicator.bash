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
    local info esc=$'\033' re
    info="$(project-indicator 2>/dev/null)"
    # The output is ESC[...m<glyph>ESC[0m. Readline counts invisible bytes as
    # width unless each escape sequence is wrapped in \001..\002; the glyph
    # itself is visible and must stay outside the markers.
    re="^(${esc}\[[0-9;]*m)(.*)(${esc}\[0m)\$"
    if [[ "$info" =~ $re ]]; then
        info=$'\001'"${BASH_REMATCH[1]}"$'\002'"${BASH_REMATCH[2]}"$'\001'"${BASH_REMATCH[3]}"$'\002'
    fi
    [[ -n "$info" ]] && printf '\001\033[90m\002[\001\033[0m\002%s\001\033[90m\002]\001\033[0m\002' "$info"
}

PS1="${PS1}\$(project_indicator_prompt) "
