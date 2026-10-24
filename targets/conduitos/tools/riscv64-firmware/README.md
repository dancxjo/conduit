# Reviewed RISC-V firmware pair (#6057, held preparation)

This pin selects the RISC-V64 CODE/VARS pair from rust-osdev/ovmf-prebuilt's
`edk2-stable202508-r1` release. The ThingOS reference identified the candidate;
the release asset digest, archive size, both regular archive members and their
individual bytes were independently checked. Exact publisher recipe, EDK2
recipe and license Git blobs were also checked. Their original bytes and
review are retained in the [tool packet](../../proof/tools/riscv64/graphics-6162901d6/index.json).

`manifest.json` records the fixed archive and members, not a mutable latest
release. `copyright` and `publisher-notice.md` retain primary licensing and the
publisher's component-license qualification, with line endings and trailing
whitespace normalized. The publisher's build-code MIT/Apache licenses are not
the firmware's license. Original source lengths and digests remain in the
manifest and retained packet.

This is reviewed input for the forthcoming explicit preparation entrance.
No acquisition command, selected product profile, graphical boot, local input,
or firmware execution is established by these files. The implementation must
verify the archive before extracting only the two named regular members,
verify both members, publish an immutable pair atomically under a preparation
lock, preserve licensing, and refuse an incomplete or corrupt cache. Boot and
doctor must verify local bytes without downloads; every boot must use its own
writable VARS copy and leave the pinned template unchanged. The existing
serial RISC-V route remains separately selected.
