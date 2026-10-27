# Project Indicator — fish prompt hook
#
# Install: copy to ~/.config/fish/functions/fish_right_prompt.fish
# (or call `project_indicator_prompt` from your own fish_right_prompt).

function project_indicator_prompt
    command -q project-indicator; or return
    set -l info (project-indicator 2>/dev/null)
    set -l normal (set_color normal)
    set -l gray (set_color brblack)
    test -n "$info"
    and printf '%s[%s%s%s]%s' "$gray" "$normal" "$info" "$gray" "$normal"
end

function fish_right_prompt
    project_indicator_prompt
end
