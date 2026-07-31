# Bounded conduct run stream current form

Status: C3/C4 normative hosted transport and presentation contract

Run-stream schema marker: `0`

This document corrects the compatibility-channel framing introduced by
specification 020. It preserves presentation bytes, semantic values, executor
transactions, immutable evidence, diagnostics, and status as distinct facts.
It does not infer a value, event, node, port, type, or transaction from an
implementation `Read` or `Write` boundary.

## current form withdrawal

`conduit.run` remains current in specification 020 and
`conformance/c3/conduct-output.json` as the exact pre-release contract that
called an arbitrary process write a `record = "value"`. Conduit deliberately
withdraws that writer before a release rather than silently changing version
1. Current `conduct --run --format=ndjson` output is exclusively
`conduit.run`. Readers recognize current only to reject it as withdrawn and
reject every other unsupported schema/version combination without fallback.

Finite `conduit.result`, diagnostics, human output, and the ordinary
`conduct [--check|--explain|--run] [PANEL|-]` interface are unchanged.

## Tagged outer stream

Every compact newline-terminated record contains:

- `schema = "conduit.run"`;
- `schema_version = 2`;
- a zero-based contiguous global `sequence`; and
- an explicit `record` discriminator.

The sequence is the order in which records reach this adapter. It does not
replace a semantic publication sequence or an `ExecutionEvent` sequence.
Advancement is checked before serialization. Exhaustion rejects the record
with stable reason `run-stream-sequence-overflow`.

Diagnostics, terminal status, and progress remain on stderr under
specifications 016, 018, and 020. They are never run-stream records.

## Bounded compatibility channel chunks

Displaced runtime stdout and stderr writes become `channel_chunk` records:

```json
{"schema":"conduit.run","schema_version":0,"sequence":7,"record":"channel_chunk","channel":"stdout","encoding":"hex","payload_bytes":3,"payload_hex":"616263"}
```

The hosted compatibility limits are:

| Quantity | Maximum |
|---|---:|
| decoded bytes per channel chunk | 4,096 bytes |
| encoded hex payload | 8,192 bytes |
| complete serialized channel record including newline | 8,448 bytes |
| complete summary record including newline | 512 bytes |
| future direct structured record including newline | 65,536 bytes |

Multiplication, record-size addition, sequence advancement, and returned write
counts are checked. Serialization writes into a buffer whose capacity is the
declared record ceiling; it cannot grow beyond that ceiling. Hex staging is
at most twice one channel chunk. A caller-provided slice is traversed in
4,096-byte pieces and is never cloned or encoded as one whole temporary.

An empty `Write::write` emits no record and returns zero. A nonempty write at
or below the chunk limit emits one record. A larger write emits a deterministic
ordered series of maximum-sized chunks followed by at most one remainder.
Each `payload_bytes` equals the decoded hex length.

Chunk boundaries have no semantic identity. One logical value may span
several chunks, and one chunk may contain bytes from several logical values.
No channel record is named `value`, and channel records contain no inferred
node, port, type, publication, transaction, or event field.

Filtering the global record sequence by one channel and concatenating decoded
payloads reproduces exactly the bytes presented to that channel adapter.
stdout and stderr share one sequence, so their observed adapter interleaving
is retained without claiming a clock or scheduler order.

## Structured executor records

Typed publications and immutable evidence do not pass through the
compatibility channel writer. They use dedicated methods on the outer stream
and arrive from executor-owned transactions or observations.

The current implementation provides a direct `execution_event` path accepting the
owned representation of specification 012. It nests that evidence unchanged
under the tagged outer transport record and applies the 65,536-byte outer
record ceiling. It never parses channel bytes, logs, delimiters, or timing to
construct evidence.

Issue #23 may add a direct typed-publication record only when it can name the
exact plan/run, semantic source port and type, executor transaction, and value
identity. That record must use a dedicated structured method with the same
bounded outer serializer. It must not route a publication through
`Write::write`, reinterpret `channel_chunk`, or claim the CLI chunk is an
executor representation fragment.

A `channel_chunk` fails classification as either a typed publication or
`ExecutionEvent`, even if its bytes happen to contain their JSON encoding.

## Completion and output failures

The successful `summary` record retains the specification 020 fields but uses
the current outer schema. It is a transport completion fact, not immutable
execution evidence. Failed execution appends no successful summary.

Each complete encoded record is staged before its first output write. If a
later chunk fails after earlier chunks were accepted, `Write::write` returns
the accepted decoded-byte count in the ordinary Rust partial-write form.
The caller can retry the remainder. A failure before any accepted chunk is
returned directly.

A downstream broken pipe during the first or any later record is calm
termination with no diagnostic. Every other output failure remains
`CND-IO-002` on diagnostic stderr. No recovery path appends another record
after a non-broken output failure.

## Conformance and migration

`conformance/c3/conduct-run-stream.json` freezes limits, split and
coalesced reconstruction, nonsemantic classification, global interleaving,
arithmetic boundaries, partial failures, clean machine stdout, direct
structured evidence, and version policy.

The historical current fixture is unchanged. This is intentional evidence of the
withdrawn contract, not a second supported writer. Migrating a consumer means
selecting current and treating `channel_chunk` only as compatibility I/O. A
consumer that needs semantic publications or evidence must select the
corresponding structured discriminator and validate its nested identity.

## Normative requirements

| ID | Obligation |
|---|---|
| RUN-001 | Emit current run NDJSON exclusively as tagged `conduit.run` records |
| RUN-002 | Preserve current bytes as historical fixtures while explicitly rejecting the withdrawn writer |
| RUN-003 | Represent compatibility stdout/stderr writes only as nonsemantic `channel_chunk` records |
| RUN-004 | Limit decoded chunks to 4,096 bytes, hex payloads to 8,192 bytes, and serialized channel records to 8,448 bytes |
| RUN-005 | Check sequence, multiplication, serialized-size, and write-count arithmetic before committing output |
| RUN-006 | Split large writes without staging or allocating in proportion to the arbitrary whole input slice |
| RUN-007 | Emit nothing for empty writes and preserve exact per-channel reconstruction across split and coalesced writes |
| RUN-008 | Preserve one contiguous global adapter-observation sequence across stdout and stderr |
| RUN-009 | Never classify or enrich a channel chunk as a typed value, publication, transaction, or execution event |
| RUN-010 | Route immutable executor evidence directly through a bounded structured record path |
| RUN-011 | Require future typed publications to use a direct bounded executor-owned path rather than channel bytes |
| RUN-012 | Retain calm first/later broken-pipe termination and structured non-broken output failure |
| RUN-013 | Keep diagnostics, status, progress, and prose outside machine stdout |
| RUN-014 | Retain human output and the canonical conduct invocation unchanged |
