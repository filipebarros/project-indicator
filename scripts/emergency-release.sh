#!/bin/bash
# Emergency manual release script for project-indicator
# Use only when automated releases are broken or for hotfixes

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
print_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

print_warn "🚨 EMERGENCY RELEASE MODE"
print_warn "Normal releases should use PR labels and automation."
print_warn "This script is for emergencies only."
echo

# Basic checks
if [ ! -d ".git" ]; then
    print_error "Not in a git repository"
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    print_error "Uncommitted changes detected. Commit first."
    exit 1
fi

# Get current version
current_version=$(cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version')
print_info "Current version: $current_version"

# Ask for new version
echo "Enter new version (current: $current_version):"
read -p "New version: " new_version

if [[ ! $new_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    print_error "Invalid version format. Use x.y.z"
    exit 1
fi

# Confirm
print_warn "This will create release v$new_version"
read -p "Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    print_info "Cancelled"
    exit 0
fi

# Install cargo-edit if needed
if ! command -v cargo-set-version &> /dev/null; then
    print_info "Installing cargo-edit..."
    cargo install cargo-edit
fi

# Update version
print_info "Setting version to $new_version"
cargo set-version $new_version
cargo check

# Run tests
print_info "Running tests..."
cargo test

# Commit and tag
git add Cargo.toml Cargo.lock
git commit -m "Emergency release v$new_version"

tag="v$new_version"
git tag -a "$tag" -m "Emergency release $tag"

# Push
print_info "Pushing changes..."
git push origin main --tags

print_info "✅ Emergency release $tag created!"
print_info "GitHub Actions will build binaries automatically."
print_info "Remember to use PR labels for future releases."
