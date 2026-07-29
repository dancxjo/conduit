# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_conduct_global_optspecs
    string join \n check explain run format= diagnostic-format= color= q/quiet v verbose-diagnostics h/help V/version
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
complete -c conduct -n "__fish_conduct_needs_command" -l check -d 'Parse, resolve, and validate without starting nodes'
complete -c conduct -n "__fish_conduct_needs_command" -l explain -d 'Show exact node, port, cord, type, and flow resolution'
complete -c conduct -n "__fish_conduct_needs_command" -l run -d 'Run the panel (the default mode)'
complete -c conduct -n "__fish_conduct_needs_command" -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -n "__fish_conduct_needs_command" -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -n "__fish_conduct_needs_command" -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -n "__fish_conduct_needs_command" -s h -l help -d 'Print help'
complete -c conduct -n "__fish_conduct_needs_command" -s V -l version -d 'Print version'
complete -c conduct -n "__fish_conduct_needs_command" -a "inspect" -d 'Validate and describe one artifact without executing it'
complete -c conduct -n "__fish_conduct_using_subcommand inspect" -l type -d 'Select a frozen artifact kind, or use marker-only detection' -r -f -a "auto\t''
panel\t''
lowered-source\t''
execution-plan\t''
evidence\t''
diagnostic\t''
conformance\t''"
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
