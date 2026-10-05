# ConduitOS architecture proof appliances

This tree owns the bounded A-rung entrypoints used by repository proof commands.
They reuse ConduitOS machine and runtime mechanics, but they are not product
binaries and cannot be selected through ordinary product make.

The ordinary ConduitOS product entrypoints remain under `src/`. Architecture
proofs enter through `cargo xtask make conduitos ...`; direct Cargo binary invocation
is an internal build detail.

The IA-32 appliance also exercises timer takeover for
[#4973](https://github.com/dancxjo/conduit/issues/4973). It deliberately leaves a
periodic PIT IRQ pending before machine initialization, requires an unarmed
timer to remain clean, then cancels and rearms an exact kernel interest. A stale
cancellation must refuse while the current generation delivers exactly one
wake. Run it through `cargo xtask make conduitos prove --arch ia32`.

The original failed [target job](https://github.com/dancxjo/conduit/actions/runs/37178286474/job/111367588847)
and its `diagnostics-conduitos-ia32` artifact remain historical evidence. Its
generic terminal receipt cannot recover the original operation or reason.
The controlled takeover reproduction reports `wake-active` / `stale-wake`
before repair; it demonstrates the inherited-IRQ lifecycle fault without
claiming that historical receipt distinguished stale wake from overflow.
Deterministic mailbox and timer-slot conformance runs in
`cargo xtask check workspace-test-hosts`. The appliance and canonical live-media
boot establish emulator execution, not physical timing or hardware acceptance.
