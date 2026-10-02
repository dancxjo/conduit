# History and retained evidence

These records explain earlier decisions and preserve proof for named
checkpoints. They are not current setup instructions or a list of unfinished
work. Use [current status](../../STATUS.md), the [roadmap](../roadmap.md), and
[architecture reference](../architecture/README.md) for present work.

Retained records may use `form` for an executable program and `code` for a
portable representation. Current source calls those `plot` and `form`,
respectively; see the [language migration audit](../architecture/plot-form-language-migration.md).
Historical syntax, identifiers, paths, and receipts remain evidence of their
named checkpoints, not aliases accepted by the current language.

## Acceptance and CI

- [Recorded acceptance milestones](accepted-milestones.md): exact receipts and checkpoint-specific limits
- [CI impact benchmarks](ci-impact-benchmark.md): measurements of their named runs
- [Retired candidate-evidence workflow](ci-candidate-evidence.md): why contributor-managed reconciliation was removed
- [Reuse ledger](../reuse-ledger.md): reviewed recovery and provenance

Current PR and release procedures have one home in the
[contributor CI guide](../contributing/ci.md).

## Language design provenance

- [Language settlement](language-settlement.md): the retained #4109 audit, amendment context, and recovered issue lineage

## Host and kernel design checkpoints

- [Portable hosts](architecture/portable-hosts.md)
- [Host specification](architecture/host-specification.md)
- [Reboot kernel M0](architecture/reboot-kernel-m0.md)
- [Salvage kernel S1](architecture/salvage-kernel-s1.md)
- [Salvage planning S2](architecture/salvage-planning-s2.md)
- [Kernel takeover](architecture/kernel-takeover.md)
- [Browser host S4](architecture/browser-host-s4.md)

## Patchbay implementation checkpoints

- [Native shell](architecture/native-patchbay-shell.md)
- [Topology projection](architecture/native-patchbay-topology.md)
- [Plot editor (historical “form editor”)](architecture/native-patchbay-form-editor.md)
- [Plan and play control](architecture/native-patchbay-control.md)
- [Primitive GUI](architecture/native-patchbay-gui.md)

## Physical records

- [Pete R23 carrier audit](hardware/pete-r23-carrier-audit.md): an attended diagnostic record, not today's attached-device state
- [Retained Latimer images](../evidence/physical/latimer/): physical evidence assets

The [Pete Brainstem migration](../architecture/pete-brainstem-migration.md)
also has current replacement and physical-proof obligations, so it remains
with the architecture contracts.

## Finding an older path

Checkpoint pages formerly under `docs/architecture/` now live under
`docs/history/architecture/`; the R23 audit lives under `hardware/`. The two CI
records moved from `docs/` into this directory. Their unique content is
retained, with relative links updated. Old issue links can be resolved against
[the pre-reorganization tree](https://github.com/dancxjo/conduit/tree/bea86efa70af4b62da81fc7eaf2541834024fde3/docs)
or Git history.

The redundant `integration-and-promotion.md` summary was retired in favor of
the contributor CI guide. Legal notices, third-party attribution, governance,
and evidence assets are not retired documentation.
