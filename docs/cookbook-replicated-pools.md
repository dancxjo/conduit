# Replicated pool cookbook

Use a pool when one already-defined composite template must handle a finite
population of semantically identified work items. A pool does not make child
ports public and does not permit runtime graph editing.

State: **contract-only**. The referenced template and its exact implementation,
host, authority, and resource bindings are not installed by this snippet.

```panel
panel 1

pool workers : fixture/worker {
  maximum = 4
  admission = queue_bounded
  admission_queue = 8
  deadline_ms = 1000
  idle_timeout_ms = 250
  supervision = restart_bounded
  restart_attempts = 2
  restart_backoff_ms = 25
  cleanup = drain
}
```

The source fields above are complete: there are no runtime defaults. Exact
compilation must additionally bind the selected monotonic tick conversion,
per-instance child/cord/state/scheduler/host-operation/cancellation profile,
queued-request profile, normative evidence limit, and simultaneous old,
candidate, and rollback generation reserve.

Admission outcomes stay distinct:

- `reject` returns the request without storing it;
- `block` leaves the request with the caller and creates no hidden queue;
- `queue_bounded` stores only the authored queue maximum; and
- `fail` makes capacity denial the exact terminal admission outcome.

Supply request, work-unit, and caller-correlation identities from semantic
input. Do not use a timestamp, random slot, worker ID, or arrival counter.
Conduit combines those facts with plan, epoch, generation, and attempt to
derive stable instance and attempt correlation.

Before offering work, resolve authority, sensitivity, template identity, the
exact plan-pinned implementation-set identity, and the full resource profile.
Do not reduce implementation compatibility to a boolean assertion. If any fact
is absent, reject before children start. During a foreign/native step,
reconcile actual usage with the same profile; an excess is terminal
containment, not a request to grow the pool. Hosted adapters should pass #56
step observations through `observe_pool_step`; it commits the
implementation-machine copy only after the matching pool evidence transition
succeeds.

For a plan transition, reserve all three populations up front:

```text
old live + candidate live + rollback live
```

Then stop old-generation admission, cancel its queued requests with one exact
cause, drain or abort live instances, and retire only after cleanup. Issue #57
owns that orchestration; the pool controller supplies the bounded primitives.

Useful focused checks:

```sh
cargo test -p conduit-core --test pool_vectors
cargo test -p conduit-runtime --test replicated_pool_vectors
cargo test -p conduit-rp2040-hil --test pool_contract
bash browser/run-chromium-vectors.sh
```

The RP2040 test is a linked fixed-storage firmware oracle. Do not report
physical HIL unless a board and its transport evidence were actually present.
