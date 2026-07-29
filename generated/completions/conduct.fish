complete -c conduct -l format -d 'Select human, finite JSON, or streaming NDJSON primary output' -r -f -a "human\t''
json\t''
ndjson\t''"
complete -c conduct -l diagnostic-format -d 'Select human or lossless JSON diagnostics on stderr' -r -f -a "human\t''
json\t''"
complete -c conduct -l color -d 'Select diagnostic terminal styling' -r -f -a "auto\t''
always\t''
never\t''"
complete -c conduct -l check -d 'Parse, resolve, and validate without starting nodes'
complete -c conduct -l explain -d 'Show exact node, port, cord, type, and flow resolution'
complete -c conduct -l run -d 'Run the panel (the default mode)'
complete -c conduct -s q -l quiet -d 'Suppress nonessential status and progress, never values or diagnostics'
complete -c conduct -s v -d 'Add bounded resolution status detail; repeat for future detail levels'
complete -c conduct -l verbose-diagnostics -d 'Include related spans, notes, paths, and causes'
complete -c conduct -s h -l help -d 'Print help'
complete -c conduct -s V -l version -d 'Print version'
