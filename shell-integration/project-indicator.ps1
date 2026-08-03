# Project Indicator — PowerShell prompt hook
#
# Caching is handled by the binary itself (persistent, mtime-invalidated,
# ~0.2ms warm hits), so this hook just calls it.
#
# Install: dot-source this file from your $PROFILE:
#   . C:\path\to\project-indicator.ps1
# It wraps your existing prompt function and appends the indicator.

function Get-ProjectIndicator {
    if (-not (Get-Command project-indicator -ErrorAction SilentlyContinue)) {
        return ""
    }
    $info = & project-indicator 2>$null
    if ([string]::IsNullOrWhiteSpace($info)) {
        return ""
    }
    return " $([char]27)[90m[$([char]27)[0m$info$([char]27)[90m]$([char]27)[0m"
}

$script:PreviousPromptFunction = $function:prompt

function prompt {
    $previous = & $script:PreviousPromptFunction
    "$previous$(Get-ProjectIndicator) "
}
