# LoongArch graphical emulator preparation

For #6057, the optional `cargo xtask make conduitos prepare-loongarch64-domain-emulator --graphical` recipe prepares the reviewed diagnostic QEMU with SDL and pixman. It retains both existing LoongArch corrections. The ordinary serial recipe remains unchanged.

## Observed preparation

The [retained packet](../../targets/conduitos/proof/tools/loongarch64/graphics-d8b593cdf/index.json) records actual source `d8b593cdf668e95bdd5845200a33a12a6ab5c098`. Cold preparation, warm validation, and a second warm invocation without the local SDK environment variables passed. The executable still depends on the retained local SDK runtime libraries; this does not claim a portable static tool. Standard prerequisites and the supported command are described in the [tool guide](../../targets/conduitos/proof/tools/loongarch64/README.md).

The serial cache key remains `83b88accc2d317b10d86224fef8969c3aaa901eba19e5b8dd171a0df5ca7e3fb`. The separate graphics key is `225a0b2f8dc09b3cf8fc174a6cbcaf961f5df9997326cceb7ee902540250938c`. Actual executable SHA-256 is `ad9ed950138b7fc2e2c2e86dd258c94216b1116becbf76126b4e429eabe4c477`. The observed backend inventory contains `none`, `sdl`, and `curses`; QMP exposes `screendump`. The empty-machine probe pauses execution and runs no guest instructions.

Four focused tool tests, the command parsing test, strict xtask Clippy, and supported pipeline preflight passed. Earlier failed attempts remain in the packet: one lacked Ninja in its local prerequisite PATH, one completed linking but rejected QEMU's display-help footer, and two stopped at the pre-build storage guard. The corrected parser accepts the actual footer while retaining the backend bounds and required capabilities. Those failures are not passing preparation evidence.

## Remaining product proof

This packet proves source-built tool preparation and capability discovery. It proves no firmware framebuffer, graphical Conduit frame, checked Face/Mask/Show, GPU acceleration, input support, physical hardware, or accepted release. It changes no default live-product route. LoongArch's actual normal graphical product frame and its comparison with the other architectures remain required by #6057.
