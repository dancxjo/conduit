# Conduct generated assets and machine output current form

Status: C3 normative hosted CLI presentation and transport contract

This document extends the command contract in specification 018 with
generated completions and manual pages, quiet and general verbosity policy,
truthfully bounded progress state, finite result JSON, and streaming run
NDJSON. It does not change diagnostic current schema, create a second
diagnostic renderer, or claim that one-shot run output is immutable execution
evidence.

## One command model and generated assets

Parsing, `--help`, Bash, Zsh, Fish, PowerShell, and Elvish completions, and the
manual page use `conduct::command()`, the single Clap derive model. Generation
performs no registry, host, network, or model discovery.

The checked-in artifacts are:

```text
generated/completions/conduct.bash
generated/completions/_conduct
generated/completions/conduct.fish
generated/completions/conduct.ps1
generated/completions/conduct.elv
generated/man/conduct.1
```

`just cli-assets` is the sole update command. `just cli-assets-check`
regenerates in memory and fails on any byte drift; hosted CI runs that check.
The manual's stream, machine-output, and exit sections are deterministic
additions to the Clap-rendered option model.

## Independent selectors and streams

`--format=human|json|ndjson` selects primary stdout encoding.
`--diagnostic-format=human|json` independently selects diagnostics on stderr.
Neither option changes the meaning of the other.

| Operation | `human` | `json` | `ndjson` |
|---|---|---|---|
| check | accepted | finite result | `CND-CLI-003` |
| explain | accepted | finite result | `CND-CLI-003` |
| run/default | accepted | `CND-CLI-003` | streaming run records |

An unsupported pairing fails before reading source, leaves stdout empty, and
uses the selected diagnostic encoding. An unknown format spelling is the
ordinary argument diagnostic `CND-CLI-001`.

Machine stdout contains only its selected JSON values. It never contains ANSI,
carriage-return rewrites, cursor control, spinners, terminal status, progress,
or diagnostic prose. Human or diagnostic JSON failures stay on stderr and
leave machine stdout empty. Combining the two streams requires a future
explicit outer protocol; this command never merges them.

## Finite result envelope

Check and explain JSON each emit exactly one newline-terminated compact record:

```json
{
  "schema": "conduit.result",
  "schema_version": 0,
  "operation": "check",
  "result": {}
}
```

`operation` is `check` or `explain`. Check reports source panel version and
definition/root counts. Explain reports a structured resolution view:
logical composites and their children, cords, exports, and bindings plus
expanded nodes, ports, bounded cords, and pressure policies. It does not
serialize the human explanation and does not turn presentation prose into a
semantic plan.

## Streaming run envelope

Run NDJSON emits bounded, compact, newline-terminated `conduit.run` records.
Runtime process writes use the current `channel_chunk` form from specification
028, followed by one summary after successful completion.
Every record has `schema`, `schema_version`, a zero-based contiguous
`sequence`, and a `record` discriminator.

A channel record is lossless process I/O and does not claim a semantic value:

```json
{"schema":"conduit.run","schema_version":0,"sequence":0,"record":"channel_chunk","channel":"stdout","encoding":"hex","payload_bytes":2,"payload_hex":"00ff"}
```

`channel` is `stdout` or `stderr`. In NDJSON mode both runtime value channels
are encoded into the ordered stdout record stream; process stderr remains
exclusively diagnostic/status presentation.

The successful terminal record is:

```json
{"schema":"conduit.run","schema_version":0,"sequence":1,"record":"summary","nodes_completed":3,"cords_conducted":2}
```

The summary is an execution transport fact, not an `ExecutionEvent` from
specification 012. When issue #23 connects immutable lifecycle and flow
evidence to live execution, it must use an explicit tagged record and preserve
the already-versioned evidence schema rather than relabel this summary.

A downstream stdout closure at any record boundary is successful completion
with no diagnostic. A non-broken stdout failure is `CND-IO-002`. A failed run
does not append a successful summary.

## Quiet, verbosity, color, and progress

`-q/--quiet` suppresses nonessential status and progress only. It never
suppresses primary values or required diagnostics. Repeatable `-v` controls
general terminal status detail: one occurrence adds resolved node/cord counts;
two or more also report selected operation and primary format.
`--verbose-diagnostics` remains independent and controls related diagnostic
structure. Quiet and general verbosity conflict; quiet may be combined with
verbose diagnostics.

The color and environment precedence remains exactly specification 018.
`CI` does not override explicit terminal facts: typical redirected CI has no
status because stderr is not a terminal. Machine primary formats suppress
status even on a terminal. Diagnostic JSON, `TERM=dumb`, non-terminal stderr,
and quiet also suppress it. No mode uses cursor control.

`BoundedProgress` accepts only a positive known total and monotonic updates no
greater than that total; it records cancellation separately. Zero totals,
reverse movement, and overflow are rejected. It deliberately owns no
renderer. No current check, explain, or one-shot run operation has both
long-running work and a truthful progress total, so no current operation
renders progress. Future use must remain on terminal stderr and must be
suppressed for quiet, non-terminal, or machine-output execution.

## Conformance

`conformance/c3/conduct-output.json` freezes result envelopes, ordered run
records, every operation/format pairing, stream policy, quiet/verbosity,
bounded progress, pipe/output failures, and the generated artifact inventory.
The `conduct-cli` fixture is the canonical current base invocation contract.

Consumers must select a supported schema and operation pairing. They must not
infer result schema from diagnostic format, treat run records as finite
results, treat a summary as immutable evidence, or silently fall back from an
unsupported format.

Specification 021 adds finite `inspect` results and a generated
`conduct-inspect(1)` page without changing the result/diagnostic selectors or
the canonical run/check/explain defaults here.

Specification 028 defines the bounded run NDJSON writer and reader selection
policy. Writers and readers accept schema `0` only; no withdrawn form or
fallback reader is retained.

## Normative requirements

| ID | Obligation |
|---|---|
| OUT-001 | Generate help, completions, and the manual from the sole Clap command model |
| OUT-002 | Check in deterministic Bash, Zsh, Fish, PowerShell, Elvish, and man artifacts with one update and drift-check path |
| OUT-003 | Keep primary `--format` independent from diagnostic format and preserve stdout/stderr ownership |
| OUT-004 | Emit versioned finite check and structured explain results as `conduit.result` |
| OUT-005 | Emit lossless, ordered, discriminated run records as `conduit.run` |
| OUT-006 | Keep machine stdout free of prose, diagnostics, ANSI, cursor control, and progress |
| OUT-007 | Reject unsupported operation/format pairs with `CND-CLI-003` and clean stdout |
| OUT-008 | Let quiet suppress only status/progress and keep general and diagnostic verbosity distinct |
| OUT-009 | Permit progress only with a known positive bound and monotonic in-bound updates |
| OUT-010 | Suppress status/progress for machine output, non-terminals, diagnostic JSON, `TERM=dumb`, and quiet |
| OUT-011 | Treat broken stdout pipes as success and other output failures as `CND-IO-002` |
| OUT-012 | Preserve diagnostic current and execution-evidence identities instead of inventing replacement records |
