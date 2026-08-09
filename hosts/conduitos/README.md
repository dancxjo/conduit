# ConduitOS freestanding host

This crate owns the first ConduitOS machine and production-kernel boundary
funded by issue #588. It builds one `no_std`, `no_main` x86_64 executable,
boots it through the pinned Limine protocol, normalizes bootloader observations
into bounded ConduitOS data, and gives the sole cooperative execution lane to
the production `conduit-kernel` scheduler. One real PIT interrupt wakes one
exact kernel interest and the resulting value crosses one exact Cord to one
bounded COM1 presentation.

The Limine request and response types are confined to `src/boot/limine.rs`.
Code outside that adapter consumes the boot-neutral types in
`src/boot/observation.rs`.

Run the complete proof with:

```console
cargo xtask conduitos prove --arch x86-64 --locked
```

The command mechanically checks the executable, assembles the same hybrid
BIOS/UEFI ISO twice, requires identical digests, and validates two real QEMU
boots with fresh `HostId` and `BootId` values. Each boot must emit exactly one
bounded boot Sign and one correlated kernel Sign proving the finite Host
offer, admitted memory, single execution lane, timer wake, serial
presentation, empty pending-operation set, and production scheduler identity.
It writes the evidence record to
`target/conduitos/x86_64/kernel-proof.json`.

The proof requires `curl`, `make`, `tar`, `xorriso`, and
`qemu-system-x86_64`. Missing tools, unsupported architecture backends,
malformed or absent boot/kernel responses, exceeded bounds, unavailable Bases,
QEMU timeouts, and stale identities are explicit proof refusals.

The proof topology is intentionally hand-lowered and is not a Form, Plan, or
Play. This slice adds no allocator, preemption, SMP, framebuffer, network, or
second runtime and does not activate the other architecture backends. Ordinary
checking, planning, lowering, reservation, and Play remain behind the next
acceptance slice in issue #588.
