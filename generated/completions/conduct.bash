_conduct() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="conduct"
                ;;
            conduct,compile)
                cmd="conduct__subcmd__compile"
                ;;
            conduct,inspect)
                cmd="conduct__subcmd__inspect"
                ;;
            conduct,package)
                cmd="conduct__subcmd__package"
                ;;
            conduct__subcmd__package,create)
                cmd="conduct__subcmd__package__subcmd__create"
                ;;
            conduct__subcmd__package,extract)
                cmd="conduct__subcmd__package__subcmd__extract"
                ;;
            conduct__subcmd__package,verify)
                cmd="conduct__subcmd__package__subcmd__verify"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        conduct)
            opts="-q -v -h -V --check --explain --run --format --diagnostic-format --color --quiet --verbose-diagnostics --compile-input --compatibility-demo --enable-file-write --enable-file-watch --enable-storage-cache --enable-process-exec --enable-socket-loopback --help --version inspect compile package"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                --compile-input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__compile)
            opts="-q -v -h --input --format --diagnostic-format --color --quiet --verbose-diagnostics --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --input)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__inspect)
            opts="-q -v -h --type --format --diagnostic-format --color --quiet --verbose-diagnostics --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --type)
                    COMPREPLY=($(compgen -W "auto panel lowered-source execution-plan evidence diagnostic conformance package" -- "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__package)
            opts="-q -v -h --format --diagnostic-format --color --quiet --verbose-diagnostics --help create verify extract"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__package__subcmd__create)
            opts="-q -v -h --manifest --blob --output --format --diagnostic-format --color --quiet --verbose-diagnostics --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --manifest)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --blob)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__package__subcmd__extract)
            opts="-q -v -h --output-dir --format --diagnostic-format --color --quiet --verbose-diagnostics --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output-dir)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
        conduct__subcmd__package__subcmd__verify)
            opts="-q -v -h --policy --observations --format --diagnostic-format --color --quiet --verbose-diagnostics --help"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --policy)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --observations)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --format)
                    COMPREPLY=($(compgen -W "human json ndjson" -- "${cur}"))
                    return 0
                    ;;
                --diagnostic-format)
                    COMPREPLY=($(compgen -W "human json" -- "${cur}"))
                    return 0
                    ;;
                --color)
                    COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _conduct -o nosort -o bashdefault -o default conduct
else
    complete -F _conduct -o bashdefault -o default conduct
fi
