# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_conduct_global_optspecs
    string join \n check explain run format= diagnostic-format= color= q/quiet v verbose-diagnostics compile-input= compatibility-demo enable-file-write enable-file-watch h/help V/version
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
complete -c conduct -n "__fish_conduct_needs_command" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_needs_command" -s V -l version -d 'Print version'
complete -c conduct -n "__fish_conduct_needs_command" -a "inspect" -d 'Validate and describe one artifact without executing it'
complete -c conduct -n "__fish_conduct_needs_command" -a "compile" -d 'Compile source against explicit immutable inputs into one exact plan'
complete -c conduct -n "__fish_conduct_needs_command" -a "package" -d 'Create, verify, or extract a bounded content-addressed package'
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
