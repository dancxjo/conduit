
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'conduct' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'conduct'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'conduct' {
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('--check', '--check', [CompletionResultType]::ParameterName, 'Parse, resolve, and validate without starting nodes')
            [CompletionResult]::new('--explain', '--explain', [CompletionResultType]::ParameterName, 'Show exact node, port, cord, type, and flow resolution')
            [CompletionResult]::new('--run', '--run', [CompletionResultType]::ParameterName, 'Run the panel (the default mode)')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
