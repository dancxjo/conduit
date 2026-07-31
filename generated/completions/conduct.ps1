
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
            [CompletionResult]::new('--compile-input', '--compile-input', [CompletionResultType]::ParameterName, 'Resolve and run against this explicit compile-input snapshot')
            [CompletionResult]::new('--check', '--check', [CompletionResultType]::ParameterName, 'Parse, resolve, and validate without starting nodes')
            [CompletionResult]::new('--explain', '--explain', [CompletionResultType]::ParameterName, 'Show exact node, port, cord, type, and flow resolution')
            [CompletionResult]::new('--run', '--run', [CompletionResultType]::ParameterName, 'Run the panel (the default mode)')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('--compatibility-demo', '--compatibility-demo', [CompletionResultType]::ParameterName, 'Run the finite batch compatibility demo instead of an exact plan')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('inspect', 'inspect', [CompletionResultType]::ParameterValue, 'Validate and describe one artifact without executing it')
            [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Compile source against explicit immutable inputs into one exact plan')
            [CompletionResult]::new('package', 'package', [CompletionResultType]::ParameterValue, 'Create, verify, or extract a bounded content-addressed package')
            break
        }
        'conduct;inspect' {
            [CompletionResult]::new('--type', '--type', [CompletionResultType]::ParameterName, 'Select a current artifact kind, or use marker-only detection')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'conduct;compile' {
            [CompletionResult]::new('--input', '--input', [CompletionResultType]::ParameterName, 'Read the sealed compile-input document from this JSON file')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'conduct;package' {
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create one deterministic thick or thin package')
            [CompletionResult]::new('verify', 'verify', [CompletionResultType]::ParameterValue, 'Validate package metadata against explicit trust observations')
            [CompletionResult]::new('extract', 'extract', [CompletionResultType]::ParameterValue, 'Validate and extract embedded blobs to digest-derived paths')
            break
        }
        'conduct;package;create' {
            [CompletionResult]::new('--manifest', '--manifest', [CompletionResultType]::ParameterName, 'Read the sealed package manifest from this JSON file')
            [CompletionResult]::new('--blob', '--blob', [CompletionResultType]::ParameterName, 'Add one exact embedded blob as SHA256=PATH; repeat as needed')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Write the deterministic package envelope to this new path')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'conduct;package;verify' {
            [CompletionResult]::new('--policy', '--policy', [CompletionResultType]::ParameterName, 'Read the explicit package trust policy from this JSON file')
            [CompletionResult]::new('--observations', '--observations', [CompletionResultType]::ParameterName, 'Read external signature verification observations from this JSON file')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
        'conduct;package;extract' {
            [CompletionResult]::new('--output-dir', '--output-dir', [CompletionResultType]::ParameterName, 'Create digest-derived blob paths beneath this directory')
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
