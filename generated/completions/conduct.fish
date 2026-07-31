# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_conduct_global_optspecs
    string join \n check explain run format= diagnostic-format= color= q/quiet v verbose-diagnostics compile-input= compatibility-demo enable-file-write enable-file-watch enable-storage-cache enable-media-ffmpeg enable-media-sox enable-process-exec enable-socket-loopback enable-http-client-loopback h/help V/version
end

function __fish_conduct_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_conduct_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_conduct_using_subcommand
    set -l cmd (__fish_conduct_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c conduct -n "__fish_conduct_needs_command" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_needs_command" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_needs_command" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_needs_command" -l compile-input -d 'Resolve and run against this explicit compile-input snapshot' -r -F
complete -c conduct -n "__fish_conduct_needs_command" -l check -d 'Parse, resolve, and validate without starting nodes'
complete -c conduct -n "__fish_conduct_needs_command" -l explain -d 'Show exact node, port, cord, type, and flow resolution'
complete -c conduct -n "__fish_conduct_needs_command" -l run -d 'Run the panel (the default mode)'
complete -c conduct -n "__fish_conduct_needs_command" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_needs_command" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_needs_command" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_needs_command" -l compatibility-demo -d 'Run the finite batch compatibility demo instead of an exact plan'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-file-write -d 'Explicitly install the bounded example file-write provider'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-file-watch -d 'Explicitly install the bounded example file-watch provider'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-storage-cache -d 'Explicitly install the bounded evictable blob-cache provider'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-media-ffmpeg -d 'Explicitly install the bounded FFmpeg-overlapping media providers'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-media-sox -d 'Explicitly install the bounded SoX-overlapping media providers'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-process-exec -d 'Explicitly install the bounded closed-inventory process provider'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-socket-loopback -d 'Explicitly install the bounded numeric-loopback socket providers'
complete -c conduct -n "__fish_conduct_needs_command" -l enable-http-client-loopback -d 'Explicitly install the bounded numeric-loopback HTTP client provider'
complete -c conduct -n "__fish_conduct_needs_command" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_needs_command" -s V -l version -d 'Print version'
complete -c conduct -n "__fish_conduct_needs_command" -a "inspect" -d 'Validate and describe one artifact without executing it'
complete -c conduct -n "__fish_conduct_needs_command" -a "compile" -d 'Compile source against explicit immutable inputs into one exact plan'
complete -c conduct -n "__fish_conduct_needs_command" -a "package" -d 'Create, verify, or extract a bounded content-addressed package'
complete -c conduct -n "__fish_conduct_needs_command" -a "capsule" -d 'Pack, inspect, check, unpack, or diff an authored panel capsule'
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l type -d 'Select a current artifact kind, or use marker-only detection' -r -f -a "auto\t''
panel\t''
lowered-source\t''
execution-plan\t''
evidence\t''
diagnostic\t''
conformance\t''
package\t''"
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand compile" -l input -d 'Read the sealed compile-input document from this JSON file' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand compile" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand compile" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand compile" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand compile" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand compile" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand compile" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand compile" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -f -a "create" -d 'Create one deterministic thick or thin package'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -f -a "verify" -d 'Validate package metadata against explicit trust observations'
complete -c conduct -n "__fish_conduct_using_subcommand package; and not __fish_seen_subcommand_from create verify extract" -f -a "extract" -d 'Validate and extract embedded blobs to digest-derived paths'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l manifest -d 'Read the sealed package manifest from this JSON file' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l blob -d 'Add one exact embedded blob as SHA256=PATH; repeat as needed' -r
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l output -d 'Write the deterministic package envelope to this new path' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l policy -d 'Read the explicit package trust policy from this JSON file' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l observations -d 'Read external signature verification observations from this JSON file' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from verify" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -l output-dir -d 'Create digest-derived blob paths beneath this directory' -r -F
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand package; and __fish_seen_subcommand_from extract" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "pack" -d 'Create one canonical capsule JSON document without fetching artifacts'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "inspect" -d 'Validate and describe a capsule without executing its source'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "check" -d 'Validate the capsule and parse its authored panel offline'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "explain" -d 'Resolve and explain the capsule source without fetching artifacts'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "unpack" -d 'Write source and optional auxiliary documents to a new directory'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and not __fish_seen_subcommand_from pack inspect check explain unpack diff" -f -a "diff" -d 'Compare authored, lock, reference, and presentation identities'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l lock -r -F
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l presentation -r -F
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l references -r -F
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l output -r -F
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from pack" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from inspect" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from check" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from explain" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -l output-dir -r -F
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from unpack" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_using_subcommand capsule; and __fish_seen_subcommand_from diff" -s h -l help -d 'Print help'
