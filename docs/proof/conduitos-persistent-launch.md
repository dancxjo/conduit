# Persistent graphical product launch (#6057)

The development entrance `cargo xtask make host launch OUTPUT` opens a verified
PROFILE-built product and waits until its QEMU window closes. It does not use
the boot verifier's bounded receipt/guest-stop loop. `verify --boot` remains the
separate entrance for two-boot acceptance.

```sh
cargo xtask make host build targets/conduitos/profiles/conduitos-aarch64-graphical.host.conduit --output target/host-build/aarch64-graphics
cargo xtask make host launch target/host-build/aarch64-graphics
```

Select an already prepared graphical product. The launcher checks the complete
retained BUILD/image binding before running and checks it again on return.
Execution does not download firmware or rebuild the guest. Each IA-32/RISC-V
session owns a fresh private VARS copy, which is removed when the session
returns; firmware templates remain unchanged. The session directory is private.
Close the window to stop. A graphical desktop and the selected QEMU window
backend must be available.

| Target | Launch route | Local input |
|---|---|---|
| x86_64 | Existing visible q35 product route | Existing admitted keyboard/pointer behavior |
| AArch64 | virt/neoverse-n2, firmware ramfb, SDL window | Read-only snapshot |
| IA-32 | q35/max, pinned UEFI32, std VGA, SDL window | Read-only snapshot |
| RISC-V64 | Prepared graphical virt/rv64 tool, paired firmware, ramfb, SDL window | Read-only snapshot |
| LoongArch64 | Unavailable until normal graphical product qualification | Not claimed |

Serial-only products refuse this window entrance before selecting an emulator.
No local input provider is added to a graphical snapshot. The launcher prints
that limitation and does not fabricate an interactive guest revision. Session
launch/exit records are labelled human-session records, not boot-proof receipts.

## Retained local checks

The [evidence packet](../../targets/conduitos/proof/persistent-launch/b8ccb1cca/index.json)
contains the full commands, raw output, session records, runners and hashes.
Two focused hosted tests passed: independently writable 0600 VARS copies do
not modify the template or each other, cleanup removes only owned copies, and
dry-run preparation neither reads a template nor creates a copy. Strict xtask
Clippy with warnings denied and all targets passed.

Three supervised sessions use the previously retained normal graphical
products, without rebuilding their guests:

| Product | Guest source | Observed session |
|---|---|---|
| AArch64 | `f96860659760660fc796e7694df5bee4bd9f2bb8` | Product-ready and shown Face; alive for at least three more seconds |
| IA-32 | `64d5130753466f44c0b605ad6b366d54083eeafd` | Product-ready and shown Face; alive for at least three more seconds |
| RISC-V64 | `61084bb3ae35324bfb7dc2c280b37b6c60b7b00e` | Product-ready and shown Face; alive for at least three more seconds |

Each supervisor then sends SIGTERM only to its owned QEMU child. The launchers
return zero, retain successful exit records, and independently checked ISO hashes
remain unchanged. Face/Show, PROFILE/BUILD/logical-image bindings and the common
authored Face Source were independently joined from complete serial records.
The AArch64 session preceded an IA-only window-backend change; its own recipe
was unchanged. The packet makes this tool-source qualification explicit.

These sessions use SDL's dummy backend. They prove actual normal-product
execution and persistent process lifetime, not a visible attended window,
human enactment, physical scanout or keyboard/pointer parity. x86_64 delegates
the existing visible route and was not rerun in this packet. A valid historical
serial-only product was independently refused before emulator selection.

An initial plain Cargo check was stopped with status 143 to prevent disk
exhaustion. It is retained as an aborted check, not a pass. Disposable compiler
outputs were reclaimed before the successful bounded-cache checks above.

This launcher does not complete #6057. The LoongArch normal graphical product,
all-five comparison, truthful matrices and required integration acceptance
remain separate outstanding obligations.
