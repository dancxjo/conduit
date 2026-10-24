# Ordinary display output construction

`PreparedDisplayKernel` executes the checked display-output Source in the shared
fixed scheduler. `prepare` retains the fixed-store conformance profile.
`prepare_with_storage` accepts independently prepared finite value and sign
stores, and puts the unchanged scheduler in boxed custody through its existing
boxed constructor. This avoids returning the large scheduler by value through
preparation frames. Heap-backed stores still have fixed admitted capacities;
Play does not allocate.

Both entrances validate the exact Plan and lowering, numeric kernel tables,
current retained software selection, and output shape before transferring the
software. Supplied stores must be empty, provide at least twelve value slots
and 12 KiB of live value capacity, cover the lowered sign budget, and reserve
256 remote-sign slots for the finite Fore profile. Root separately admits the
stores' backing, the scheduler allocation and temporary construction headroom.
This API is not an allocator admission guarantee or effect authority.

The public `cargo xtask prove environment display-output` proof runs the actual
Source with both fixed and finite hosted stores. It checks two successive
frames, invalid brightness, pre-service cancellation and zero allocation during
Play. Undersized values, insufficient sign or remote-sign backing, occupied
values and occupied signs refuse without consuming retained software owners.

This construction prerequisite is tracked by #6374. It does not install the
output graph on the MCU, authenticate a Fore caller, issue a native capability,
start a Body, or replace the live bench's coordination loop. Compact assignment
of this graph's external Fore, combined sensor/Mask/output admission and physical
execution remain required for the continuing native appliance.

## Compact native preparation

`PreparedAssignedDisplayKernel::prepare` accepts the canonical header and
borrowed records, a trusted linked Make profile, the current Root scope, retained
software and independently admitted finite stores. It does not deserialize a
rich Plan or reconstruct software from advertised descriptors. The shared
assigned-table builder and original kernel constructor preflight the complete
numeric topology before exclusive software transfer.

The retained-owner check matches current Host/Boot/offer epoch, Source family,
exact kind/implementation/artifact identities, actual program/selector text and
pure Host Call ABI. All three initialized portable owners are checked before
any is transferred. A dropped preflight keeps them available; a successful
transfer withdraws them from future offers and replay refuses as already
selected. There is no automatic epoch increment: Root owns that transition.

The compact result retains numeric Fore pins. Root must independently retain
and enforce their typed value contracts, bind the actual GPIO owner and native
authority, admit complete memory/work and issue Body start before stepping.
Preparation creates no native provider, capability possession, Line or Body.
The checked display CPU proof now uses this production preparation entrance,
including stale epoch, foreign Source/Boot, occupied storage and zero-frame
refusals followed by successful execution of the original owners.
