# Native Patchbay topology projection

Issue #556 extends the accepted #555 shell with a read-only view over the
current Observatory boundary. `PatchbayTopology` accepts only validated
`ObservatorySnapshot` values and retains at most four projected reports in the
native composition. When another arrives, the oldest presentation report is
dropped and the cumulative drop count remains visible. The Observatory
snapshot's own Sign capacity and visible-gap count are shown separately.
Each accepted report is limited to 256 projected topology lines and 64 KiB of
neutral rendered report data before it enters history; an oversized report is
rejected without displacing the last valid view.

The current report is projected into finite text lines for hosts, exact boots,
operation offers and status, resources, links, and observations. Host state,
capability availability, link report state, and the link binding's mutable
availability observation remain separate fields. Exact base instance,
endpoint host/boot, and link identities remain inspectable. Multiple reports
for one `HostId` with different `BootId` values are separate rows; stale state
is rendered rather than coalesced behind an alias.

The neutral `HostRow` also carries the advertisement's exact portable planner
offers and limits. This prevents a presentation consumer from reaching around
Observatory to recover them or silently treating planning as application
privilege.

Sorting and filtering operate on a presentation copy. They cannot change the
retained `ObservatoryReport`, add an absent host/capability/link, mutate a Plan,
or become membership/runtime truth. Invalid snapshots are rejected before
retained state changes. The presentation has a hard 256-line limit and fails
instead of silently truncating report facts inside that bound.

`patchbay-native` renders those lines into a software pixel buffer using a
build-validated, fixed GNU Unifont subset behind a renderer-local
`embedded-graphics` drawing target. Glyph lookup is allocation-free and the
adapter clips every pixel against both declared dimensions and the actual
finite slice. `winit`, `softbuffer`, drawing coordinates, glyph identities,
and font rendering remain in the native adapter; `patchbay-model` has no
toolkit dependency. The
`--observatory-snapshot PATH` option reads one ordinary JSON snapshot artifact,
validates it through Observatory, and does not discover, control, or persist
the reported subjects.

Canonical checked-face equality from #522 remains the compatibility rule. The
view displays exact checked capability facts and availability but performs no
compatibility matching, planning, selection, realization, or authority action.
