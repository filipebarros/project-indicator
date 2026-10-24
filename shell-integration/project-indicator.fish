# Project Indicator — fish prompt hook
#
# Install: copy to ~/.config/fish/functions/fish_right_prompt.fish
# (or call `project_indicator_prompt` from your own fish_right_prompt).

function project_indicator_prompt
    command -q project-indicator; or return
    set -l info (project-indicator 2>/dev/null)
    test -n "$info"
    and echo -n (set_color brblack)"["(set_color normal)$info(set_color brblack)"]"(set_color normal)
end

function fish_right_prompt
    project_indicator_prompt
end
