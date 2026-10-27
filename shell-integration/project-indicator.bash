# Project Indicator — bash prompt hook
#
# Install: source this file from ~/.bashrc. It appends the indicator to PS1;
# call project_indicator_prompt from your own PROMPT_COMMAND if you already
# customize the prompt.

project_indicator_prompt() {
    command -v project-indicator >/dev/null 2>&1 || return
    local info rest wrapped="" esc=$'\033' re
    info="$(project-indicator 2>/dev/null)"
    # Readline counts invisible bytes as width unless every escape sequence is
    # wrapped in \001..\002; visible glyphs must stay outside the markers.
    rest=$info
    re="^([^${esc}]*)(${esc}\[[0-9;]*m)(.*)\$"
    while [[ "$rest" =~ $re ]]; do
        wrapped+="${BASH_REMATCH[1]}"$'\001'"${BASH_REMATCH[2]}"$'\002'
        rest="${BASH_REMATCH[3]}"
    done
    info="$wrapped$rest"
    [[ -n "$info" ]] && printf '\001\033[90m\002[\001\033[0m\002%s\001\033[90m\002]\001\033[0m\002' "$info"
}

# Idempotent: re-sourcing ~/.bashrc must not stack the hook
[[ "$PS1" == *project_indicator_prompt* ]] || PS1="${PS1}\$(project_indicator_prompt) "
