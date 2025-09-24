#!/bin/bash
# Project Indicator Shell Integration Installer
# Automatic installation script for Fish, Bash, and Zsh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Default installation settings
SHELL_TYPE=""
CONFIG_FILE=""
AUTO_INSTALL=false
FORCE_INSTALL=false

# Helper functions
log_info() {
    printf "${BLUE}ℹ${NC} %s\n" "$1"
}

log_success() {
    printf "${GREEN}✓${NC} %s\n" "$1"
}

log_warning() {
    printf "${YELLOW}⚠${NC} %s\n" "$1"
}

log_error() {
    printf "${RED}✗${NC} %s\n" "$1" >&2
}

# Detect current shell
detect_shell() {
    if [[ -n "${FISH_VERSION:-}" ]]; then
        echo "fish"
    elif [[ -n "${ZSH_VERSION:-}" ]]; then
        echo "zsh"
    elif [[ -n "${BASH_VERSION:-}" ]]; then
        echo "bash"
    else
        # Fallback to shell detection via $SHELL or ps
        local shell_name
        shell_name=$(basename "${SHELL:-$(ps -p $$ -o comm=)}")
        case "$shell_name" in
            fish) echo "fish" ;;
            zsh) echo "zsh" ;;
            bash) echo "bash" ;;
            *) echo "unknown" ;;
        esac
    fi
}

# Get config file for shell
get_config_file() {
    local shell="$1"
    case "$shell" in
        fish)
            echo "${XDG_CONFIG_HOME:-$HOME/.config}/fish/config.fish"
            ;;
        zsh)
            if [[ -f "$HOME/.zshrc" ]]; then
                echo "$HOME/.zshrc"
            else
                echo "$HOME/.zshrc"
            fi
            ;;
        bash)
            if [[ -f "$HOME/.bashrc" ]]; then
                echo "$HOME/.bashrc"
            elif [[ -f "$HOME/.bash_profile" ]]; then
                echo "$HOME/.bash_profile"
            else
                echo "$HOME/.bashrc"
            fi
            ;;
        *)
            return 1
            ;;
    esac
}

# Check if project-indicator binary is available
check_binary() {
    if ! command -v project-indicator >/dev/null 2>&1; then
        log_error "project-indicator binary not found in PATH"
        log_info "Install options:"
        log_info "  1. cargo install project-indicator"
        log_info "  2. Download from: https://github.com/filipebarros/project-indicator/releases"
        return 1
    fi

    # Test if binary works
    if ! project-indicator --help >/dev/null 2>&1; then
        log_error "project-indicator binary found but not working correctly"
        return 1
    fi

    log_success "project-indicator binary found and working"
    return 0
}

# Install integration for specific shell
install_integration() {
    local shell="$1"
    local config_file="$2"
    local integration_file="$SCRIPT_DIR/project-indicator.$shell"

    # Check if integration file exists
    if [[ ! -f "$integration_file" ]]; then
        log_error "Integration file not found: $integration_file"
        return 1
    fi

    # Create config directory if it doesn't exist
    local config_dir
    config_dir=$(dirname "$config_file")
    if [[ ! -d "$config_dir" ]]; then
        log_info "Creating config directory: $config_dir"
        mkdir -p "$config_dir"
    fi

    # Create config file if it doesn't exist
    if [[ ! -f "$config_file" ]]; then
        log_info "Creating config file: $config_file"
        touch "$config_file"
    fi

    # Check if already installed
    local source_line="source \"$integration_file\""
    if grep -Fq "$integration_file" "$config_file" 2>/dev/null; then
        if [[ "$FORCE_INSTALL" == "true" ]]; then
            log_warning "Integration already exists, but force flag specified"
        else
            log_warning "Integration already installed in $config_file"
            log_info "Use --force to reinstall"
            return 0
        fi
    fi

    # Add source line to config file
    log_info "Adding integration to $config_file"
    {
        echo ""
        echo "# Project Indicator Shell Integration"
        echo "$source_line"
        echo ""
    } >> "$config_file"

    log_success "Integration installed successfully!"

    # Show usage instructions
    show_usage_instructions "$shell"
}

# Show usage instructions for each shell
show_usage_instructions() {
    local shell="$1"

    echo ""
    log_info "Usage Instructions for $shell:"
    echo ""

    case "$shell" in
        fish)
            echo "Add to your prompt function:"
            echo "  function fish_right_prompt"
            echo "      fish_right_prompt_project_indicator"
            echo "  end"
            echo ""
            echo "Or get project info directly:"
            echo "  project_info"
            ;;
        bash)
            echo "Add to your PS1 variable:"
            echo "  export PS1='\\u@\\h:\\w\$(project_indicator_ps1)\\$ '"
            echo ""
            echo "Or get project info directly:"
            echo "  project_info"
            ;;
        zsh)
            echo "Add to your right prompt:"
            echo "  export RPS1='\$(project_indicator_rprompt)'"
            echo ""
            echo "Or add to main prompt:"
            echo "  export PROMPT='%n@%m:%~\$(project_indicator_prompt)%# '"
            echo ""
            echo "Or get project info directly:"
            echo "  project_info"
            ;;
    esac

    echo ""
    echo "Configuration commands:"
    echo "  project_indicator_config status    # Show status"
    echo "  project_indicator_config ttl 600   # Set cache TTL"
    echo "  project_indicator_clear_cache       # Clear cache"
    echo ""
    echo "Restart your shell or run: source $CONFIG_FILE"
}

# Interactive shell selection
select_shell() {
    echo "Available shell integrations:"
    echo "  1) Fish"
    echo "  2) Bash"
    echo "  3) Zsh"
    echo "  4) All shells"
    echo ""
    read -p "Select shell (1-4): " choice

    case "$choice" in
        1) echo "fish" ;;
        2) echo "bash" ;;
        3) echo "zsh" ;;
        4) echo "all" ;;
        *)
            log_error "Invalid selection"
            exit 1
            ;;
    esac
}

# Install for all shells
install_all() {
    local shells=("fish" "bash" "zsh")
    local success_count=0

    for shell in "${shells[@]}"; do
        log_info "Installing $shell integration..."
        local config_file
        if config_file=$(get_config_file "$shell"); then
            if install_integration "$shell" "$config_file"; then
                ((success_count++))
            fi
        else
            log_warning "Could not determine config file for $shell"
        fi
        echo ""
    done

    log_success "Installed integration for $success_count out of ${#shells[@]} shells"
}

# Show help
show_help() {
    cat << EOF
Project Indicator Shell Integration Installer

USAGE:
    $0 [OPTIONS] [SHELL]

ARGUMENTS:
    SHELL    Shell to install for (fish, bash, zsh, all)

OPTIONS:
    -h, --help     Show this help message
    -f, --force    Force reinstallation even if already installed
    -y, --yes      Skip confirmation prompts
    --check        Only check if binary is available

EXAMPLES:
    $0                    # Interactive installation
    $0 fish               # Install for Fish shell
    $0 --force zsh        # Force reinstall for Zsh
    $0 --yes all          # Install for all shells without prompts

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -f|--force)
                FORCE_INSTALL=true
                shift
                ;;
            -y|--yes)
                AUTO_INSTALL=true
                shift
                ;;
            --check)
                check_binary
                exit $?
                ;;
            fish|bash|zsh|all)
                SHELL_TYPE="$1"
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

# Main installation function
main() {
    parse_args "$@"

    echo "Project Indicator Shell Integration Installer"
    echo "============================================="
    echo ""

    # Check for binary
    if ! check_binary; then
        exit 1
    fi

    # Determine shell type
    if [[ -z "$SHELL_TYPE" ]]; then
        local detected_shell
        detected_shell=$(detect_shell)

        if [[ "$detected_shell" != "unknown" && "$AUTO_INSTALL" == "true" ]]; then
            SHELL_TYPE="$detected_shell"
            log_info "Auto-detected shell: $SHELL_TYPE"
        else
            if [[ "$detected_shell" != "unknown" ]]; then
                log_info "Detected shell: $detected_shell"
                echo ""
            fi
            SHELL_TYPE=$(select_shell)
        fi
    fi

    echo ""

    # Install based on shell type
    if [[ "$SHELL_TYPE" == "all" ]]; then
        install_all
    else
        CONFIG_FILE=$(get_config_file "$SHELL_TYPE")
        if [[ -z "$CONFIG_FILE" ]]; then
            log_error "Unsupported shell: $SHELL_TYPE"
            exit 1
        fi

        log_info "Installing $SHELL_TYPE integration..."
        log_info "Config file: $CONFIG_FILE"
        echo ""

        install_integration "$SHELL_TYPE" "$CONFIG_FILE"
    fi

    echo ""
    log_success "Installation complete!"
    log_info "See README.md for advanced configuration options"
}

# Run main function
main "$@"