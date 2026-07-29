
use builtin;
use str;

set edit:completion:arg-completer[conduct] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'conduct'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'conduct'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand --check 'Parse, resolve, and validate without starting nodes'
            cand --explain 'Show exact node, port, cord, type, and flow resolution'
            cand --run 'Run the panel (the default mode)'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
    ]
    $completions[$command]
}
