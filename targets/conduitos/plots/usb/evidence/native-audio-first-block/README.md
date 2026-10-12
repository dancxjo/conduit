# Retained native audio evidence

These archives retain executions, rather than rerun results. One identical
checked Source package and Root request carried eight exact PCM packets on
x86_64 and AArch64 QEMU, reached terminal phase 3, and retired. The cross
architecture JSON names both distinct kernel commits and the package digest.
The x86 archive includes the exact checked package, original PCM, independent
terminal decoder and wire checker. The AArch archive retains its own run.

The disconnect archive records exactly four original packets followed by an
explicit usbredir device disconnect, no later ISO packets, failed terminal
phase 1, retained original PCM and retirement. It does not identify a decoded
native provider-lost variant separately from the checked failed terminal.

The decoder uses conduit-core canonical validation. Build it in an isolated
crate with the corresponding workspace core dependency; run with serial.log,
original-pcm.bin, and optional expected terminal phase (default 3; fault 1).
Verifier scripts and exact input are retained in the archives. Local absolute
paths in run wrappers are provenance, not portable commands.

This proves one-block emulator execution only. Continuous refill, 64-TRB
reuse, full fault/cancellation coverage, other architecture cells and physical
or audible output remain unproven. None of these archives closes #5254/#5258.
