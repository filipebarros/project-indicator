# Project Indicator — PowerShell prompt hook
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

# Idempotent: dot-sourcing $PROFILE again must not wrap the wrapper (the
# wrapper would call itself forever)
if ("$function:prompt" -notmatch 'Get-ProjectIndicator') {
    $script:PreviousPromptFunction = $function:prompt

    function prompt {
        $previous = & $script:PreviousPromptFunction
        "$previous$(Get-ProjectIndicator) "
    }
}
