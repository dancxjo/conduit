# Workspace Mask service contract

This crate owns the single shared implementation of workspace row wrapping,
viewport navigation and scene layout. Root integration should reexport `layout`
for the host/reference adapter and call `scene::scene` there; do not maintain a
second copy of either algorithm.

The independently linked `conduitos-mask-service` retains `reader::Reader` in
private metadata. Its immutable image is not a Root-linked renderer helper.

## Root adapter

- Prepare an explicit `ServiceProfile::Mask` owner. First Update expects reader
  revision zero; use a nonzero Root-owned session namespace for its lifetime.
- Supply `data::Update` with the exact current readonly owner facts and canonical
  ApplicationView binary (at most 3,072 bytes). Scope includes Host/Boot, optional
  Body, Journey/outer revisions, foreground ResidentPlot, viewport extent and
  `data::view_digest(view)`. Root still validates display format and owner facts.
- Update replies with a new reader revision. It preserves viewport state unless
  the actual decoded view or foreground ResidentPlot changed. A new outer
  revision, extent, status or refusal alone does not reset navigation.
- Navigate accepts canonical three-byte KeyEvent encoding and exact expected
  reader revision. Releases, non-navigation keys and details mode do no work.
  Supported navigation preserves existing Changed behavior even at page bounds.
- Reset explicitly clears navigation. Selection returns a readonly Snapshot.
  Every query/mutation is fenced by the reader revision and outer transport's
  strict service instance/request sequence.
- Frame returns raw `[u32 metadata length][FrameReceipt JSON][GraphicsScene binary]`.
  Use `data::decode_frame`, compare the entire returned Scope/revision to the
  admitted request, check the selected node against the current Root view, and
  require every command's clip to equal the admitted viewport. Root must validate
  that a proposed selection names an in-bounds Ready Activate control using
  `layout::enabled_action`; it is not action or input-occurrence authority.
- All other command replies use `data::Reply` JSON. The fixed CMSK v1 wire is 48
  bytes within the 4 KiB exchange; contiguous upload/read chunks are at most
  16 KiB. Each complete input/output blob is at most 64 KiB, including metadata.
- Root retains effect admission, actual scanout, Face identity/basis, checked
  Mask/Show correlation and genuine input provenance. No guest receipt is a Show,
  physical visibility claim or capability. Terminal failures must latch the
  owner inert and release its service lease; never retry or use a Root renderer
  fallback. Replacement requires explicit fresh ownership/admission.

Closed layout: RX/RO image 0x400000..0x600000; exchange 0x601000;
transfer 0x603000..0x607000; stack 0x608000..0x708000; metadata
0x709000..0x809000. Its first metadata page is the bounded private RW/NX ELF data
initialization template. Retained state starts at 0x70a000. The allocator header
ends at 0x70e000; the remainder is a private finite heap. These are admission
ceilings, not measured peak usage or timing guarantees.

## General entrance controller

`control::Controller` retains the selected Host/Plot side, open-Plot flag,
exact-details mode and sixteen-page detail position before any Body, workspace
view or display extent exists. `Command::Control` has a separate expected control
revision and an exact Host/Boot/service-instance scope. Read and canonical key
commands return a finite `control::View`; lifecycle observation and explicit
inspect commands do not carry effect proposals. Root keeps its monotonic input
revision and all identities, lifecycle actions, Mask/Show and effect admission.

Native FrontDoor does not retain a writable copy of these controller fields.
The host reference invokes the same `control::apply` reducer. Workspace Update
requires the initialized same-scope controller and uses its details-mode flag,
rather than admitting a replacement mode from Root's fact DTO. General Face and
scene paths query the controller before rendering Arrival/Home/workspace branches;
terminal transport failure remains closed with no local controller fallback.
