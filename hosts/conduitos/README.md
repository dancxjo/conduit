# ConduitOS freestanding host

This crate owns the first ConduitOS boot boundary funded by issue #588. It
builds one `no_std`, `no_main` x86_64 executable, boots it through the pinned
Limine protocol, normalizes bootloader observations into bounded ConduitOS
data, emits one structured boot Sign over COM1, and exits QEMU
deterministically.

The Limine request and response types are confined to `src/boot/limine.rs`.
Code outside that adapter consumes the boot-neutral types in
`src/boot/observation.rs`.

Run the complete proof with:

```console
cargo xtask conduitos prove --arch x86-64 --locked
```

The command mechanically checks the executable, assembles the same hybrid
BIOS/UEFI ISO twice, requires identical digests, and validates two real QEMU
boots with fresh `HostId` and `BootId` values. It writes the evidence record to
`target/conduitos/x86_64/boot-proof.json`.

The proof requires `curl`, `make`, `tar`, `xorriso`, and
`qemu-system-x86_64`. Missing tools, unsupported architecture backends,
malformed or absent boot responses, exceeded bounds, QEMU timeouts, and stale
identities are explicit proof refusals.

This slice does not start a Conduit Plan or Play, claim kernel scheduling
ownership, add a second runtime, or activate the other architecture backends.
Those remain behind the acceptance sequence recorded in issue #588.
