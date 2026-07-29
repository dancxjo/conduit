# Conduct command-line contract version 1

Status: C3 normative hosted CLI contract

This document freezes `conduct` command parsing, stream ownership, terminal
policy, and failure presentation over the structured diagnostic contract in
specification 016. It does not define a second error model, result/evidence
serialization, progress protocol, or execution-plan identity.

## Canonical command model

`conduct` has one `clap` derive command model and no subcommands:

```text
conduct PANEL.panel
conduct --run PANEL.panel
conduct --check PANEL.panel
conduct --explain PANEL.panel
conduct -
```

Run is the default. An absent panel and the exact path `-` read source from
stdin. An ordinary first positional value, including `run`, is a panel path
and is never reinterpreted as a subcommand.

Exactly zero or one of `--run`, `--check`, and `--explain` is accepted.
The diagnostic presentation flags established by specification 016 remain:

```text
--diagnostic-format=human|json
--color=auto|always|never
--verbose-diagnostics
```

Help and version are successful primary values on stdout. Argument failures
are `CND-CLI-001` structured diagnostics. Invalid presentation values are
argument failures too. Presentation flags are scanned before the command is
parsed so a valid `--diagnostic-format=json`, color choice, or verbosity choice
still governs an otherwise-invalid invocation.

## Streams and exits

Primary check, explanation, help, version, and run values go to stdout.
Diagnostics and interactive status go to stderr. A failure with
`--diagnostic-format=json` emits exactly one compact diagnostic JSON record on
stderr and leaves stdout empty. Diagnostic JSON is not result, plan, event, or
evidence JSON.

Success, help, version, and a closed downstream stdout pipe exit zero. Command,
source, resolution, runtime, and non-broken output failures exit two. A closed
stderr does not panic, retry, write to stdout, or change the failure exit.

Reading a missing file or non-UTF-8 source produces `CND-IO-001`. A stdout
failure other than `BrokenPipe` produces `CND-IO-002`. Parser, resolver, and
runtime failures use the adapters and `OwnedDiagnostic` model from
specification 016. The terminal renderer and JSON encoder in
`conduit-diagnostics` remain the only diagnostic presentation authorities.

## Terminal policy

Status is deliberately finite: one line at each applicable phase and one
finished line. Version 1 may emit `Checking`, `Resolving`, `Running`, and
`Finished`; it emits no spinner, animation, carriage-return rewrite, cursor
control, or banner. Status is enabled only for human diagnostics on a terminal
stderr when `TERM` is not `dumb`. It never decorates streaming stdout.

No current operation exposes a truthful, bounded progress total, so version 1
does not display progress. Specification work owned by issue #18 may add a
bounded progress mechanism without changing the stream rules here. That issue
also owns quiet/general verbosity, completions, manuals, and result/evidence
machine formats.

Human diagnostic color follows this precedence:

1. explicit `--color=always` or `--color=never`;
2. in auto mode, presence of `NO_COLOR` disables color;
3. nonzero `CLICOLOR_FORCE` enables color, including on non-terminals;
4. `TERM=dumb` or `CLICOLOR=0` disables color; and
5. otherwise color follows whether stderr is a terminal.

JSON diagnostics never contain ANSI escapes regardless of the color choice.
Color policy does not enable status on a non-terminal. Explicit color does not
override the `TERM=dumb` prohibition on status or cursor behavior.

## Dependency and cost decision

The reference uses `clap` 4.6.4 with derive, help, usage, suggestions, and
error-context support. Its optional color feature is disabled: Conduit's
existing renderer remains the sole color owner. `clap` 4.6.4 declares Rust
1.85 as its minimum supported Rust version, below this workspace's Rust 1.88
policy.

The following indicative measurements were taken on
`x86_64-unknown-linux-gnu` with Rust 1.95.0, release profile, and 1,000
launches of `conduct --version`:

| Measurement | baseline `2d082a1` | command model |
|---|---:|---:|
| Binary bytes | 932,208 | 1,539,120 |
| 1,000 launches | 0.63 s | 0.66 s |

The accepted impact is 606,912 bytes in this unstripped local release binary
and approximately 0.03 ms per launch in this single-run measurement. The
dependency replaces handwritten parsing and supplies one testable command
model; it does not add another diagnostic or terminal renderer.

## Conformance

`conformance/c3/conduct-cli-v1.json` is normative. Command cases execute the
canonical modes, stdin, diagnostic flags and formats, errors, help/version,
stream failures, and exact snapshots. Presentation cases freeze TTY,
non-TTY, environment, explicit-color, status, spinner, and cursor behavior.
The fixture records the reproducible measurement context and links warning
presentation to the normative structured-diagnostic warning vector.

## Normative requirements

| ID | Obligation |
|---|---|
| CLI-001 | Parse the canonical invocation with one command model and run as the default |
| CLI-002 | Keep run, check, and explain mutually exclusive without introducing subcommands |
| CLI-003 | Preserve diagnostic format, color, and verbose diagnostic flags |
| CLI-004 | Route every failure through the structured diagnostic model and renderer boundary |
| CLI-005 | Keep primary values on stdout and diagnostics/status on stderr |
| CLI-006 | Emit one diagnostic JSON record on stderr with clean stdout on failure |
| CLI-007 | Apply the frozen terminal, environment, and explicit color precedence |
| CLI-008 | Suppress status, animation, carriage returns, and cursor control off terminal stderr |
| CLI-009 | Treat a broken stdout pipe as successful downstream completion |
| CLI-010 | Diagnose input and non-broken output failures without panicking on closed stderr |
| CLI-011 | Keep diagnostic JSON distinct from result and evidence machine formats |
| CLI-012 | Measure and document parser dependency startup and binary-size impact |
