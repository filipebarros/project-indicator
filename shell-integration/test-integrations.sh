#!/bin/bash
# Smoke-test the shell prompt hooks against a release build.
#
# Each hook is loaded in its shell inside a fixture project with an isolated
# XDG_CACHE_HOME; the prompt function must emit non-empty output.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PATH="$SCRIPT_DIR/../target/release:$PATH"

if ! command -v project-indicator >/dev/null 2>&1; then
    echo "Error: project-indicator binary not found — run 'cargo build --release' first" >&2
    exit 1
fi

fixture="$(mktemp -d)"
cache="$(mktemp -d)"
trap 'rm -rf "$fixture" "$cache"' EXIT
printf '{"name":"t","dependencies":{"react":"1"}}' >"$fixture/package.json"
export XDG_CACHE_HOME="$cache"

fail=0

echo "Testing bash hook..."
out="$(cd "$fixture" && bash -c "source '$SCRIPT_DIR/project-indicator.bash'; project_indicator_prompt")"
if [[ -n "$out" ]]; then echo "✓ bash: $out"; else
    echo "✗ bash hook produced no output"
    fail=1
fi
# Readline needs \001..\002 around every escape sequence or it miscounts width
if [[ "$out" == *$'\033'* && "$out" != *$'\001\033[38;2'* ]]; then
    echo "✗ bash hook: colour escapes are not wrapped in \\001..\\002"
    fail=1
fi

echo "Testing zsh hook..."
if command -v zsh >/dev/null 2>&1; then
    out="$(cd "$fixture" && zsh -c "source '$SCRIPT_DIR/project-indicator.zsh'; project_indicator_prompt")"
    if [[ -n "$out" ]]; then echo "✓ zsh: $out"; else
        echo "✗ zsh hook produced no output"
        fail=1
    fi
    # zle needs %{ %} around escape codes or it miscounts width
    if [[ "$out" == *$'\033'* && "$out" != *'%{'$'\033'* ]]; then
        echo "✗ zsh hook: colour escapes are not wrapped in %{ %}"
        fail=1
    fi
else
    echo "- zsh not available, skipped"
fi

echo "Testing fish hook..."
if command -v fish >/dev/null 2>&1; then
    out="$(cd "$fixture" && fish -c "source '$SCRIPT_DIR/project-indicator.fish'; project_indicator_prompt")"
    if [[ -n "$out" ]]; then echo "✓ fish: $out"; else
        echo "✗ fish hook produced no output"
        fail=1
    fi
else
    echo "- fish not available, skipped"
fi

# The first hook call should have populated the binary's persistent cache
if [[ -d "$cache/project-indicator/results" ]]; then
    echo "✓ persistent cache populated at \$XDG_CACHE_HOME/project-indicator/results"
else
    echo "✗ persistent cache was not populated"
    fail=1
fi

exit "$fail"
