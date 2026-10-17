# Shared-pool dispatch inspection

Issue #5573 adds a bounded read-only receipt view. It cannot admit an operation,
select a member, retry execution, or reserve resources. The only mutable input
to rendering is the caller's exact-arithmetic work budget.

`conduit_observatory::render_pool_dispatch` joins the ordinary atomic pool
admission account to forecast-preparation accounts for the exact Plan, pool,
operation, demand and realization. It retains hard eligibility, decisive Signs,
fresh/stale/missing cost dispositions, the selected member, original-unit stage
ranges, and an owner-sampled occupied/waiting population immediately before
selection. Waiting includes the selected queue head until its owner retires it.

For a completed native local-model call,
`LocalModelCompletedWork::render_dispatch_inspection` also checks the actual
selection, exact realization and clock, input port/kind/extent/content digest,
and startup-configuration digest. The closed receipt's fields remain private;
rendering cannot turn a caller's public measurement DTO into a native completion.
Input and authored configuration contents are not printed.

The native suffix retains Produced, Truncated, Refused, Failed, Cancelled,
ProviderLost and InvalidStructuredResult separately. It reports the adapter-call
interval in clock ticks and scale with resolution and uncertainty. This interval
includes effects the adapter awaited; it is not total fulfilled Time. Queue and
warmup latency are not measured here, and transfer latency is not separately
measured. The generic receipt does not retain an execution outcome, so its
missing fields are qualified with `dispatch_receipt_`. A native suffix supplies
the actual outcome without rewriting that generic receipt.

Setup, queue, service and transfer are predictions in their owners' original
units, with an exact total in seconds. The current model does not retain a
separate warmup stage: the view reports that absence and does not relabel setup
as warmup or infer residency. Historical Signs identify source custody; matching
an identity does not authenticate a producer or renew freshness.

## Reproduce the proof

From the repository root, using the pinned toolchain:

```sh
cargo test -p conduit-observatory --test pool_dispatch
cargo test -p conduit-planner --test pool_cost_preparation
cargo test -p conduit-std-host --lib actual_native_completion_joins_atomic_dispatch -- --nocapture
```

The last command prints the native reports. It prepares a real forecast, uses
ordinary kernel selection, admits the selected native member, and executes a
deterministic adapter. It covers all seven terminal outcomes, rejects changed
request content and startup configuration, and verifies repeated inspection
leaves pool occupation unchanged. This is deterministic native execution, not
Ollama inference, a two-machine session, physical capacity, or a latency benchmark.

The Observatory tests independently cover short/long demands selecting different
members, a real waiting-queue snapshot, hard refusal, full population, stale and
missing costs, foreign operation/Sign, forged stages, input extent/duplicate
ports, numeric exhaustion and excessive occupation counters. Rendering retains
at most 32 forecast accounts, 32 inputs and 64 KiB of input extent per account,
and 512 KiB of output; the native suffix reserves a finite 1024-byte margin.
