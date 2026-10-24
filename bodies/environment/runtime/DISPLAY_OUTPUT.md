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
