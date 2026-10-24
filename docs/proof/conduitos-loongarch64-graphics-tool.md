# LoongArch graphical emulator preparation

For #6057, the optional `cargo xtask make conduitos prepare-loongarch64-domain-emulator --graphical` recipe prepares the reviewed diagnostic QEMU with SDL and pixman. It retains both existing LoongArch corrections. The ordinary serial recipe remains unchanged.

## Observed preparation

The [retained packet](../../targets/conduitos/proof/tools/loongarch64/graphics-d8b593cdf/index.json) records actual source `d8b593cdf668e95bdd5845200a33a12a6ab5c098`. Cold preparation, warm validation, and a second warm invocation without the local SDK environment variables passed. The executable still depends on the retained local SDK runtime libraries; this does not claim a portable static tool. Standard prerequisites and the supported command are described in the [tool guide](../../targets/conduitos/proof/tools/loongarch64/README.md).

The serial cache key remains `83b88accc2d317b10d86224fef8969c3aaa901eba19e5b8dd171a0df5ca7e3fb`. The separate graphics key is `225a0b2f8dc09b3cf8fc174a6cbcaf961f5df9997326cceb7ee902540250938c`. Actual executable SHA-256 is `ad9ed950138b7fc2e2c2e86dd258c94216b1116becbf76126b4e429eabe4c477`. The observed backend inventory contains `none`, `sdl`, and `curses`; QMP exposes `screendump`. The empty-machine probe pauses execution and runs no guest instructions.

Four focused tool tests, the command parsing test, strict xtask Clippy, and supported pipeline preflight passed. Earlier failed attempts remain in the packet: one lacked Ninja in its local prerequisite PATH, one completed linking but rejected QEMU's display-help footer, and two stopped at the pre-build storage guard. The corrected parser accepts the actual footer while retaining the backend bounds and required capabilities. Those failures are not passing preparation evidence.

## Remaining product proof

This packet proves source-built tool preparation and capability discovery. It proves no firmware framebuffer, graphical Conduit frame, checked Face/Mask/Show, GPU acceleration, input support, physical hardware, or accepted release. It changes no default live-product route. The later [normal-product report](conduitos-loongarch64-firmware-frame.md) and [five-architecture comparison](conduitos-firmware-frame-comparison.md) provide separate evidence for those #6057 requirements.

## Actual firmware-path probe

A later [retained machine probe](../../targets/conduitos/proof/tools/loongarch64/graphics-d8b593cdf/firmware-probes/index.json) uses this exact prepared executable with the existing reviewed Debian EDK2 ROM and a retained normal PROFILE-built ISO at historical source `79a04199eeaea4721bb2405c7bbb699a72cea25c`. Actual ELF/ISO and firmware digests are checked before launch. Each boot supplies fresh bounded 4096-byte cryptographic entropy through the existing admitted firmware channel; secret bytes and their hashes are not retained.

Without explicit `ramfb`, the product reaches its serial ready record, independently joined to the manifest's Profile/Build/logical Image. With `ramfb`, it reaches the expected `observatory-framebuffer-unrepresented` refusal and QMP captures 800×600. This confirms that the canonical prepared tool can boot the reviewed firmware/product path and expose a framebuffer to the historical boot contract. It still proves no checked graphical Show. The no-ramfb 640×384 image equals the separately retained paused-before-guest console placeholder exactly; screenshot success is not counted as graphics.

The original probe receipt's inherited `template_vars_unchanged` field denotes only the unchanged selected firmware ROM here. These launches use one BIOS ROM and no VARS pair; the separate audit records that correction. This additional evidence does not relabel the historical ISO as current source or change any public launch route.
