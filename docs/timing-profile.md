# Exact local timing profile

Issue #706 earns one deterministic timing guarantee for one exact local plan.
It does not claim that Conduit, ConduitOS, the host, or the authored form is
generally real-time.

The authored form remains the ordinary platform-neutral
`time/tick -> presentation/tick` chain. Its timing requirement is only a
deadline in microseconds. The selected boot-scoped host separately offers a
clock observation basis, resolution/error, timer wake latency, kernel-step
cost and count, presentation cost, and finite execution resources. Planning
adds those bounds and seals them into the exact plan timing basis. A deadline
below that sum is refused before an active play exists.

The admitted basis includes the arena, cord item/byte storage, wake and timer
slots, base scratch, mandatory sign storage, and fault reserve. Optional
inspection is excluded from the strict path. The play uses the already
installed `conduit-kernel` fixed scheduler: it performs no graph scan,
implementation lookup, queue creation, heap growth, or retry loop.

Run the repository proof with:

```text
cargo xtask conduitos timing-profile
```

The command executes the accepted plan with a deterministic clock, proves the
unschedulable planning refusal, and checks distinct exact signs for deadline
met, deadline miss, timer/base loss, cancellation, and stale timing basis.
Every sign retains exact plan and active-play identity. The output labels its
proof class `deterministic-emulator` and sets `physical_claim` to false.

A physical timing claim requires a separate issue, pinned hardware and build,
measurement method, environmental assumptions, raw evidence, and explicit
acceptance. This slice adds no scheduler, RTOS, remote guarantee,
mixed-criticality framework, migration, work stealing, or generic optimizer.
