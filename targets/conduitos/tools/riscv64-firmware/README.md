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

Explicit setup and local verification enter through:

```sh
cargo xtask make conduitos prepare-riscv64-firmware
cargo xtask make conduitos doctor-riscv64-firmware
```

Preparation needs `curl` and `tar` with xz support only for a cold cache. It
verifies the archive before extracting only the two named members to explicit
output files, verifies each member, preserves the notices, and publishes the
immutable pair atomically under a preparation lock. Warm preparation uses
verified local bytes without network access. An incomplete or corrupt cache
refuses without replacement; preserve it for diagnosis before moving it aside.
Doctor verifies local bytes without downloading.

These entrances prepare firmware only. They do not select a graphical product
profile or change the existing serial RISC-V route. Runtime integration must
use a private writable VARS copy for each boot and leave the pinned template
unchanged. Product boot, acknowledged Show and screenshot proof remain open.

## Retained local validation

At source `231bb012dd7653e926d3fa69e421b9541e1621c5`, the supported doctor
refused the missing cache, actual cold setup acquired and verified the fixed
archive, warm setup and doctor passed, and an independent audit verified both
read-only 32 MiB files and exact installed notices. Four focused refusal/cache
tests and strict all-target xtask Clippy passed. Corrected repository preflight
passed 345 tests; the original harness invocation omitted its required base
argument and is retained as a failed invocation. The [complete packet](../../proof/tools/riscv64/firmware-231bb012d/index.json)
keeps the commands, logs, digests and qualifications. This is firmware tool
preparation, not a graphical product boot or stable-release acceptance.
