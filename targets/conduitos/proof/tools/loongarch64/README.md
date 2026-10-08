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

Prepare the reviewed diagnostic emulator through
`cargo xtask make conduitos prepare-loongarch64-domain-emulator`, then run
`cargo xtask make conduitos loongarch64-ordinary-domain-proof`.
Preparation needs a C toolchain, curl, tar, patch, Ninja, Python with venv,
and development packages for GLib, libfdt and zlib. CI acquires those packages
only for the LoongArch lane and runs the same preparation entrance.

Preparation verifies the pinned source archive, applies both repository patches
with zero fuzz, disables dependency downloads and builds only
`loongarch64-softmmu` with the `conduit-diagnostic-misc-drdtl` version suffix.
QEMU supplies its pinned Python build wheels in the verified source archive.
The receipt retains source/patch digests, configure arguments, observed build
tool/library versions and executable digest. A warm tool must match its actual
receipt before selection. Interrupted builds retain separate attempt directories.
Compiler and library environments may differ; this does not claim identical
emulator bytes across hosts.

The prepared tool lives in an input-digest directory beneath
`target/conduitos/toolchain/loongarch64-domain-qemu`. Discovery also retains the
older internal diagnostic location at
`target/conduitos/toolchain/loongarch64-misc-drdtl/qemu-system-loongarch64`;
otherwise it uses the existing emulator discovery. The proof receipt records
the actual emulator version and executable digest and identifies this correction.
The correction is not an upstream QEMU release, physical-hardware evidence or
accepted Conduit release evidence. Keep results from this diagnostic emulator
separate from stock-emulator refusal evidence.
