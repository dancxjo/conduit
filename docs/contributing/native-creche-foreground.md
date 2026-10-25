# Native Crèche foreground development

This is development code for #6468, not a shipped Linux package or physical
scanout acceptance claim. Linux x86-64 release manufacture enables
`native-creche-sdl`; ARM64 Linux release manufacture does not yet enable it.
Pi 3B+ ConduitOS SD media and physical boot remain separate unfinished work.

On a Linux x86-64 development Host, the supported manufacture entrance is:

```sh
cargo xtask setup linux-release
cargo xtask doctor linux-release
cargo xtask make host release --platform linux-x86_64 --output /absolute/release
```

The setup supplies native SDL2 development files. The resulting executable
requires the system SDL2 runtime on its destination; this bundle does not ship
that library. Native presentation also requires a working SDL2 KMSDRM driver,
display access and input permissions. A compile or SDL2 version probe does not
prove those runtime conditions. The full `--platform linux` set uses the same
native x86-64 manufacture and preserves its existing ARM64 package.

Install the sealed manifest without starting the background service before
foreground use:

```sh
/absolute/release/conduit-linux-x86_64 host service install /absolute/release/hosted-linux-x86_64.json --state-dir /absolute/installed-host --no-start
```

The product entrance is:

```sh
conduit host service run --state-dir /absolute/installed-host --creche /absolute/grants.json
```

Stop the installed service before taking foreground ownership. The command uses
its existing Host and Body locks, verifies running-versus-installed executable
bytes, prepares one current Boot, and resumes the retained owned Body. Bare,
joined-only, active and unsupported worksets refuse. Opening this encounter does
not birth a child Body. The Crèche Source is admitted to the existing Lulled
Body without replacing its primary Source, resident or membership; the proposal
includes every member. Other activation/input owners must exist rather than
silently dropping those members.

A Host grant document selects a checked child construction and actual file
custody. It is administration, not authored Plot meaning or UI-supplied paths:

```json
{
  "schema": "conduit.install/native-creche-grants@1",
  "construction": "/reviewed/new-body.body.conduit",
  "first_host": "first",
  "reviewed_images": {
    "first": {
      "resolved_image": "/reviewed/pc/resolved-image.json",
      "target_manifest": "/reviewed/pc/build-manifest.json",
      "artifact": "/reviewed/pc/conduitos-x86_64.iso"
    }
  },
  "source_builds": {},
  "staging_directory": "/owned/creche-operations",
  "save_directory": "/owned/exported-images",
  "maximum_total_bytes": 1073741824,
  "maximum_millis": 7200000,
  "width": 1280,
  "height": 800
}
```

Optional `offered_profiles` is a bounded list of absolute `.host.conduit` paths.
The launcher canonically checks these recipes and offers them alongside the
construction's existing profiles. They grant no image, toolchain, or deployment
authority; unsupported manufacture still refuses.

The directories must already exist and be explicitly selected. The example is
not an artifact download or a catalog trust decision. Supply reviewed artifacts
through the existing obtain/review workflow; binding independently validates the
selected machinery, target receipt, actual image bytes and readback. Current
media binding produces native-PC ConduitOS ISOs. Declaring another target does
not add its media implementation.

Without `starting_sources`, the encounter uses the current native candidate
catalogue and its own canonical checker. Unsupported candidates remain visibly
refused. Optional `starting_sources` contains bounded canonical Source paths;
checked Source outside the installed native profile does not become an available
starting Plot. This is finite eligibility, not destination admission or proof
that a different release contains identical Source. Destination Wake/Plan still
checks its actual installed machinery and Host.

For source manufacture, `source_builds` maps each declared Host entry to explicit
`source`, `expected_source_identity`, `permitted_local_roots`, `toolchain`,
`limine_archive`, `workspace`, `output_directory` and `bounds` grants. The source,
toolchain and bounds use the ordinary Builder contracts. Toolchains are installed
only, boot assets are the explicitly reviewed local archive, and this entrance
grants no network origins. A missing selected-Host source/image grant refuses
before birth. Runtime refusal retains the existing job's ordinary birth and any
completed media rather than birthing per Host.

Canonical review starts one off-thread make operation; Save uses a create-new RW
regular file, sync and independent readback. The main thread retains actual
KMSDRM input/presentation custody and services retirement while the Body worker
finishes. Done ends the temporary foreground Play, restoring its initially Lulled
state even if another Plot loops. Adapter acceptance and kernel ACK are separate:
only the actual report's acknowledged delivery plus verified canonical evidence
can support preparation success. The exact Body terminal disposition is retained,
including an operator-requested Lull after the encounter completed.

Each operation has a private UUID directory containing its birth, parent
operation, per-Host artifact receipts, granted save destinations, canonical
outcome evidence, remaining/rejected/retired product inventories and terminal
report. These records preserve evidence; they are not authority to resume a
failed job after process restart. Cancellation, cleanup or retention failure does
not become preparation success. Saving makes no carrier, destination boot or
membership claim. Local CPU/kernel and QEMU evidence do not establish Linux
physical presentation, Pi 3B+ support or later-Host join.
