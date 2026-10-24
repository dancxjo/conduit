# RISC-V emulator preparation

The protected serial route prepares pinned QEMU 10.2.1 and OpenSBI 1.8.1 with
`cargo xtask make conduitos prepare-riscv64-domain-emulator`. Its recipe, cache
identity, version and discovery remain unchanged.

`cargo xtask make conduitos prepare-riscv64-domain-emulator --graphical`
selects a separate SDL/pixman recipe and the
`conduit-protected-riscv64-graphics` version suffix. It uses the same verified
source archive and OpenSBI package/firmware pins. The source build additionally
needs SDL2 and pixman development packages through normal pkg-config discovery,
beside the serial C/Ninja/Python-venv/GLib/libfdt/zlib prerequisites. No package
is installed into the host by preparation. Dependency downloads stay disabled.

A receipt records actual executable, recipe, source, firmware and library
identities. Cold and warm validation check the binary digest before its version
query, then independently require SDL and QMP `screendump`. The empty-machine
QMP probe pauses execution and retains its transcript; it executes no guest
instructions. The serial receipt remains readable without graphics fields,
while a graphical receipt cannot substitute the serial version or claim a
product frame. Graphical selection requires its own valid prepared receipt.

This proves tool preparation and capabilities only. Firmware GOP discovery,
checked graphical PROFILE/BUILD, framebuffer custody/mapping/cache completion,
Face/Mask/Show, screenshots and input require their own actual product proof.
This option changes no default live-product route, admits no physical display
or GPU acceleration, and does not establish the five-architecture #6057 exit.

## Retained local preparation

At source `6162901d6e02b7e6d7d6b3566627ec4dec0c395a`, actual cold preparation,
warm validation and a second warm invocation without the local SDK environment
variables passed. The [packet](graphics-6162901d6/index.json) retains all commands,
logs, capability transcripts, digests and the unchanged serial-key regression.
The local binary links retained owned SDK runtime libraries; the plain warm
check does not claim a portable static executable or independence from those
files. SDL2 2.32.10, pixman 0.46.4 and GLib 2.88.0 were observed.

The separate firmware review rechecks the ThingOS-referenced prebuilt EDK2
archive against GitHub's release asset digest and exact source/recipe/license
Git blobs. It is provenance review, not an implemented acquisition entrance or
a current product boot. The stock paused QEMU control is separately labelled;
it is not substituted for the canonical source-built cold/warm validation.
