#!/bin/bash
# Project Indicator shell integration installer.
#
# The hooks are ~15 lines each; this script just puts the right one in the
# right place. Caching is handled by the binary itself.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    echo "Usage: $0 [fish|zsh|bash]"
    echo "Installs the prompt hook for the given shell (default: \$SHELL)."
}

shell_type="${1:-$(basename "${SHELL:-}")}"

if ! command -v project-indicator >/dev/null 2>&1; then
    echo "⚠ project-indicator not found in PATH — install the binary first." >&2
fi

case "$shell_type" in
fish)
    target="$HOME/.config/fish/functions/fish_right_prompt.fish"
    if [[ -e "$target" ]]; then
        cp "$target" "$target.pre-project-indicator.bak"
        echo "ℹ Existing fish_right_prompt backed up to $target.pre-project-indicator.bak"
    fi
    mkdir -p "$(dirname "$target")"
    cp "$SCRIPT_DIR/project-indicator.fish" "$target"
    echo "✓ Installed fish hook to $target"
    ;;
zsh)
    line="source \"$SCRIPT_DIR/project-indicator.zsh\""
    if ! grep -qF "$line" "$HOME/.zshrc" 2>/dev/null; then
        printf '\n# Project Indicator prompt hook\n%s\n' "$line" >>"$HOME/.zshrc"
        echo "✓ Added source line to ~/.zshrc"
    else
        echo "ℹ ~/.zshrc already sources the hook"
    fi
    ;;
bash)
    line="source \"$SCRIPT_DIR/project-indicator.bash\""
    if ! grep -qF "$line" "$HOME/.bashrc" 2>/dev/null; then
        printf '\n# Project Indicator prompt hook\n%s\n' "$line" >>"$HOME/.bashrc"
        echo "✓ Added source line to ~/.bashrc"
    else
        echo "ℹ ~/.bashrc already sources the hook"
    fi
    ;;
*)
    usage
    exit 1
    ;;
esac

echo "Restart your shell to see the indicator."
