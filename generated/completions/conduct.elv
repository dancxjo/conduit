
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
            cand --compile-input 'Resolve and run against this explicit compile-input snapshot'
            cand --check 'Parse, resolve, and validate without starting nodes'
            cand --explain 'Show exact node, port, cord, type, and flow resolution'
            cand --run 'Run the panel (the default mode)'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand --compatibility-demo 'Run the finite batch compatibility demo instead of an exact plan'
            cand --enable-file-write 'Explicitly install the bounded example file-write provider'
            cand --enable-file-watch 'Explicitly install the bounded example file-watch provider'
            cand --enable-storage-cache 'Explicitly install the bounded evictable blob-cache provider'
            cand --enable-process-exec 'Explicitly install the bounded closed-inventory process provider'
            cand --enable-socket-loopback 'Explicitly install the bounded numeric-loopback socket providers'
            cand --enable-http-client-loopback 'Explicitly install the bounded numeric-loopback HTTP client provider'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand inspect 'Validate and describe one artifact without executing it'
            cand compile 'Compile source against explicit immutable inputs into one exact plan'
            cand package 'Create, verify, or extract a bounded content-addressed package'
            cand capsule 'Pack, inspect, check, unpack, or diff an authored panel capsule'
        }
        &'conduct;inspect'= {
            cand --type 'Select a current artifact kind, or use marker-only detection'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;compile'= {
            cand --input 'Read the sealed compile-input document from this JSON file'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;package'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand create 'Create one deterministic thick or thin package'
            cand verify 'Validate package metadata against explicit trust observations'
            cand extract 'Validate and extract embedded blobs to digest-derived paths'
        }
        &'conduct;package;create'= {
            cand --manifest 'Read the sealed package manifest from this JSON file'
            cand --blob 'Add one exact embedded blob as SHA256=PATH; repeat as needed'
            cand --output 'Write the deterministic package envelope to this new path'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;package;verify'= {
            cand --policy 'Read the explicit package trust policy from this JSON file'
            cand --observations 'Read external signature verification observations from this JSON file'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;package;extract'= {
            cand --output-dir 'Create digest-derived blob paths beneath this directory'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;capsule'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand pack 'Create one canonical capsule JSON document without fetching artifacts'
            cand inspect 'Validate and describe a capsule without executing its source'
            cand check 'Validate the capsule and parse its authored panel offline'
            cand unpack 'Write source and optional auxiliary documents to a new directory'
            cand diff 'Compare authored, lock, reference, and presentation identities'
        }
        &'conduct;capsule;pack'= {
            cand --lock 'lock'
            cand --presentation 'presentation'
            cand --references 'references'
            cand --output 'output'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;capsule;inspect'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;capsule;check'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;capsule;unpack'= {
            cand --output-dir 'output-dir'
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'conduct;capsule;diff'= {
            cand --format 'Select human, finite JSON, or streaming NDJSON primary output'
            cand --diagnostic-format 'Select human or lossless JSON diagnostics on stderr'
            cand --color 'Select diagnostic terminal styling'
            cand -q 'Suppress nonessential status and progress, never values or diagnostics'
            cand --quiet 'Suppress nonessential status and progress, never values or diagnostics'
            cand -v 'Add bounded resolution status detail; repeat for future detail levels'
            cand --verbose-diagnostics 'Include related spans, notes, paths, and causes'
            cand -h 'Print help'
            cand --help 'Print help'
        }
    ]
    $completions[$command]
}
