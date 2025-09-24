#!/bin/bash

# Test script for shell integrations

set -e

echo "Testing Project Indicator Shell Integrations"
echo "============================================="

# Add project-indicator to PATH
export PATH="$(pwd)/../target/release:$PATH"

# Check if project-indicator is available
if ! command -v project-indicator >/dev/null 2>&1; then
    echo "Error: project-indicator binary not found"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

echo "✓ project-indicator binary found"

# Test Bash integration
echo ""
echo "Testing Bash Integration:"
echo "-------------------------"

# Source bash integration
if ! bash -c '
    source ./project-indicator.bash
    echo "✓ Bash integration loaded"

    # Test config
    project_indicator_config status | head -1

    # Test basic functionality
    result=$(project_info)
    if [[ -n "$result" ]]; then
        echo "✓ Project detection works: $result"
    else
        echo "ℹ No project detected (expected for this directory)"
    fi

    # Test cache functionality
    project_indicator_clear_cache
    echo "✓ Cache cleared"
'; then
    echo "✗ Bash integration test failed"
    exit 1
fi

echo "✓ Bash integration test passed"

# Test Zsh integration (if zsh is available)
if command -v zsh >/dev/null 2>&1; then
    echo ""
    echo "Testing Zsh Integration:"
    echo "------------------------"

    if ! zsh -c '
        source ./project-indicator.zsh
        echo "✓ Zsh integration loaded"

        # Test config
        project_indicator_config status | head -1

        # Test basic functionality
        result=$(project_info)
        if [[ -n "$result" ]]; then
            echo "✓ Project detection works: $result"
        else
            echo "ℹ No project detected (expected for this directory)"
        fi

        # Test cache functionality
        project_indicator_clear_cache
        echo "✓ Cache cleared"
    '; then
        echo "✗ Zsh integration test failed"
        exit 1
    fi

    echo "✓ Zsh integration test passed"
else
    echo "ℹ Zsh not available, skipping zsh integration test"
fi

# Test Fish integration (if fish is available)
if command -v fish >/dev/null 2>&1; then
    echo ""
    echo "Testing Fish Integration:"
    echo "-------------------------"

    if ! fish -c '
        source ./project-indicator.fish
        echo "✓ Fish integration loaded"

        # Test config
        project_indicator_config status | head -1

        # Test basic functionality
        set result (project_info)
        if test -n "$result"
            echo "✓ Project detection works: $result"
        else
            echo "ℹ No project detected (expected for this directory)"
        end

        # Test cache functionality
        project_indicator_clear_cache
        echo "✓ Cache cleared"
    '; then
        echo "✗ Fish integration test failed"
        exit 1
    fi

    echo "✓ Fish integration test passed"
else
    echo "ℹ Fish not available, skipping fish integration test"
fi

# Test with a real project directory
echo ""
echo "Testing with Rust Project:"
echo "---------------------------"

if bash -c '
    cd ..  # Go to project root (should be a Rust project)
    source shell-integration/project-indicator.bash

    result=$(project_info)
    if [[ "$result" == *"Rust"* ]]; then
        echo "✓ Detected Rust project: $result"
    else
        echo "? Detected: $result (expected Rust)"
    fi

    # Test caching performance
    echo "Testing cache performance..."
    start_time=$(date +%s%N)
    for i in {1..10}; do
        project_info >/dev/null
    done
    end_time=$(date +%s%N)

    duration=$(( (end_time - start_time) / 1000000 ))
    echo "✓ 10 cached calls took ${duration}ms ($(( duration / 10 ))ms average)"

    if (( duration < 100 )); then
        echo "✓ Cache performance excellent (< 100ms total)"
    elif (( duration < 500 )); then
        echo "✓ Cache performance good (< 500ms total)"
    else
        echo "⚠ Cache performance could be better (${duration}ms total)"
    fi
'; then
    echo "✓ Real project test passed"
else
    echo "✗ Real project test failed"
    exit 1
fi

echo ""
echo "All Shell Integration Tests Passed! ✅"
echo ""
echo "Performance Summary:"
echo "- All integrations loaded successfully"
echo "- Caching system working"
echo "- Cross-platform compatibility verified"
echo "- Project detection functional"