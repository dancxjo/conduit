# LoongArch diagnostic emulator correction

The ordinary PLV3 backend requires the architected `CSR.MISC.DRDTL3` control
to deny direct application reads of Root's machine counter. Stock QEMU 10.2.1
marks `CSR.MISC` read-only. ConduitOS detects that missing control before
application entry and returns `protected-execution-unsupported`.

`qemu-misc-drdtl.patch` is a local diagnostic correction against the upstream
[QEMU 10.2.1 source](https://download.qemu.org/qemu-10.2.1.tar.xz)
(SHA-256 `a3717477d8e2c84d630bfffbc20f6cd3293eb45aa1e6dac6d0cc27689991c9e1`).
It makes only the PLV1–3 counter-disable bits writable, preserves the remaining
MISC fields, and ends translated blocks after a control change. QEMU's existing
`helper_rdtime_d` already enforces those bits. The intended behavior is defined
by the [LoongArch manual](https://loongson.github.io/LoongArch-Documentation/LoongArch-Vol1-EN.html#miscellaneous-controller-misc).

`qemu-nonfault-badi.patch` corrects a second diagnostic limitation: recording
an exception's instruction must not cause another exception by fetching the
same inaccessible address. It uses QEMU's non-faulting instruction probe and
retains BADI when instruction RAM is inaccessible. The original page/privilege
exception still reaches ConduitOS. ConduitOS independently validates syscall
instructions from its immutable admitted code copy, rather than relying on
BADI across Root page-table refills.

The internal diagnostic build uses both patches, `loongarch64-softmmu`, and the package-version
suffix `conduit-diagnostic-misc-drdtl`. The supported proof entrance is
`cargo xtask make conduitos loongarch64-ordinary-domain-proof`. It may find this
locally prepared emulator at
`target/conduitos/toolchain/loongarch64-misc-drdtl/qemu-system-loongarch64`;
otherwise it uses the existing emulator discovery. The proof receipt records
the actual emulator version and executable digest and identifies this correction.
The correction is not an upstream QEMU release, physical-hardware evidence or
accepted Conduit release evidence. Keep results from this diagnostic emulator
separate from stock-emulator refusal evidence.
