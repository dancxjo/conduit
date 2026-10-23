# CPU native Host Call bridge conformance

The target's ordinary-kernel storage assembly is shared directly with the CPU
conformance harness. Native service is split into bounded request extraction,
provider invocation and exact kernel completion. Native inputs/replies have fixed
storage; their futures borrow providers without borrowing the kernel. Software
selectors complete directly against the existing kernel. No adapter chooses a
node, changes a request identity, schedules work or creates Body start authority.

The retained Root-control test exercises both contiguous and borrowed assignment
forms. Each executes the complete 960-sample logical workload using real bound
Host Call adapters over explicitly inert CPU pulse/timer providers. It also covers:

- dropping an actual pending timer future before revocation and cancellation;
- provider pressure versus unavailable-provider failure;
- revoked pulse possession returning Denied without a sensor effect;
- publishing the first reading while the next timer invocation remains pending;
- rejecting a late completion after cancellation without retaining a value.

These execution/control cases allocate and reallocate nothing during Play.
The public native-construction proof, strict test lint and ESP32 build pass.
This is CPU/provider-fixture execution and firmware compilation, not physical
clock/sensor execution. The new bridge is not connected to a worker or the signed
start/release operations yet. The target build reports unused bridge items for
that unfinished wiring. The previously flashed physical preparation image remains
unchanged and unstarted. No new physical or overnight claim is made here.
