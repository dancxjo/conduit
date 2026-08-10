# Exact local timing profile

Issue #706 earns one deterministic timing guarantee for one exact local Plan.
It does not claim that Conduit, ConduitOS, the Host, or the authored Form is
generally real-time.

The authored Form remains the ordinary platform-neutral
`time/tick -> presentation/tick` chain. Its timing requirement is only a
deadline in microseconds. The selected boot-scoped Host separately offers a
clock observation basis, resolution/error, timer wake latency, kernel-step
cost and count, presentation cost, and finite execution resources. Planning
adds those bounds and seals them into the exact Plan timing basis. A deadline
below that sum is refused before an active Play exists.

The admitted basis includes the arena, Cord item/byte storage, wake and timer
slots, Base scratch, mandatory Sign storage, and fault reserve. Optional
inspection is excluded from the strict path. The Play uses the already
installed `conduit-kernel` fixed scheduler: it performs no graph scan,
implementation lookup, queue creation, heap growth, or retry loop.

Run the repository proof with:

```text
cargo xtask conduitos timing-profile
```

The command executes the accepted Plan with a deterministic clock, proves the
unschedulable planning refusal, and checks distinct exact Signs for deadline
met, deadline miss, timer/Base loss, cancellation, and stale timing basis.
Every Sign retains exact Plan and active-Play identity. The output labels its
proof class `deterministic-emulator` and sets `physical_claim` to false.

A physical timing claim requires a separate issue, pinned hardware and build,
measurement method, environmental assumptions, raw evidence, and explicit
acceptance. This slice adds no scheduler, RTOS, remote guarantee,
mixed-criticality framework, migration, work stealing, or generic optimizer.
