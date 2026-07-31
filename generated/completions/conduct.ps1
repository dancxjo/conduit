
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
            [CompletionResult]::new('--enable-file-write', '--enable-file-write', [CompletionResultType]::ParameterName, 'Explicitly install the bounded example file-write provider')
            [CompletionResult]::new('--enable-file-watch', '--enable-file-watch', [CompletionResultType]::ParameterName, 'Explicitly install the bounded example file-watch provider')
            [CompletionResult]::new('--enable-storage-cache', '--enable-storage-cache', [CompletionResultType]::ParameterName, 'Explicitly install the bounded evictable blob-cache provider')
            [CompletionResult]::new('--enable-media-ffmpeg', '--enable-media-ffmpeg', [CompletionResultType]::ParameterName, 'Explicitly install the bounded FFmpeg-overlapping media providers')
            [CompletionResult]::new('--enable-media-sox', '--enable-media-sox', [CompletionResultType]::ParameterName, 'Explicitly install the bounded SoX-overlapping media providers')
            [CompletionResult]::new('--enable-process-exec', '--enable-process-exec', [CompletionResultType]::ParameterName, 'Explicitly install the bounded closed-inventory process provider')
            [CompletionResult]::new('--enable-socket-loopback', '--enable-socket-loopback', [CompletionResultType]::ParameterName, 'Explicitly install the bounded numeric-loopback socket providers')
            [CompletionResult]::new('--enable-http-client-loopback', '--enable-http-client-loopback', [CompletionResultType]::ParameterName, 'Explicitly install the bounded numeric-loopback HTTP client provider')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('inspect', 'inspect', [CompletionResultType]::ParameterValue, 'Validate and describe one artifact without executing it')
            [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Compile source against explicit immutable inputs into one exact plan')
            [CompletionResult]::new('package', 'package', [CompletionResultType]::ParameterValue, 'Create, verify, or extract a bounded content-addressed package')
            [CompletionResult]::new('capsule', 'capsule', [CompletionResultType]::ParameterValue, 'Pack, inspect, check, unpack, or diff an authored panel capsule')
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
        'conduct;capsule' {
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Select human, finite JSON, or streaming NDJSON primary output')
            [CompletionResult]::new('--diagnostic-format', '--diagnostic-format', [CompletionResultType]::ParameterName, 'Select human or lossless JSON diagnostics on stderr')
            [CompletionResult]::new('--color', '--color', [CompletionResultType]::ParameterName, 'Select diagnostic terminal styling')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress nonessential status and progress, never values or diagnostics')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Add bounded resolution status detail; repeat for future detail levels')
            [CompletionResult]::new('--verbose-diagnostics', '--verbose-diagnostics', [CompletionResultType]::ParameterName, 'Include related spans, notes, paths, and causes')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('pack', 'pack', [CompletionResultType]::ParameterValue, 'Create one canonical capsule JSON document without fetching artifacts')
            [CompletionResult]::new('inspect', 'inspect', [CompletionResultType]::ParameterValue, 'Validate and describe a capsule without executing its source')
            [CompletionResult]::new('check', 'check', [CompletionResultType]::ParameterValue, 'Validate the capsule and parse its authored panel offline')
            [CompletionResult]::new('explain', 'explain', [CompletionResultType]::ParameterValue, 'Resolve and explain the capsule source without fetching artifacts')
            [CompletionResult]::new('unpack', 'unpack', [CompletionResultType]::ParameterValue, 'Write source and optional auxiliary documents to a new directory')
            [CompletionResult]::new('diff', 'diff', [CompletionResultType]::ParameterValue, 'Compare authored, lock, reference, and presentation identities')
            break
        }
        'conduct;capsule;pack' {
            [CompletionResult]::new('--lock', '--lock', [CompletionResultType]::ParameterName, 'lock')
            [CompletionResult]::new('--presentation', '--presentation', [CompletionResultType]::ParameterName, 'presentation')
            [CompletionResult]::new('--references', '--references', [CompletionResultType]::ParameterName, 'references')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
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
        'conduct;capsule;inspect' {
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
        'conduct;capsule;check' {
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
        'conduct;capsule;explain' {
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
        'conduct;capsule;unpack' {
            [CompletionResult]::new('--output-dir', '--output-dir', [CompletionResultType]::ParameterName, 'output-dir')
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
        'conduct;capsule;diff' {
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
