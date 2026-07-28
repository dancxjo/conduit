# Conformance fixture version 1 history

## Manifest revision 2 — 2026-07-28

- Added the independent `panel-source` suite and
  `conformance/c3/panel-grammar-v1.json` cases for issue #14.
- Existing version 1 requests and expected outputs are unchanged.
- Expected version: new `panel-source-v1` operation at grammar version 1; no
  previous operation result was replaced.
- Requirement IDs: `SRC-001` through `SRC-011`.
- Migration: runners may add the `conduit.c3` profile explicitly; they must
  report it as unsupported rather than silently skipping its cases.

## Manifest revision 1 — 2026-07-28

- Established `conduit.conformance/v1` and protocol version 1.
- Indexed the reviewed canonical, compatibility, type, port/config, flow,
  lifecycle, composite, authority, plan, and evidence artifacts from issues
  #3 through #12.
- Added deterministic byte, recursion, and discovery-order seeds.
- Expected version: initial version; no prior expected output or migration.
- Requirement IDs: `CNF-001` through `CNF-009`, plus the semantic requirement
  IDs recorded per suite in `manifest.json`.
- This is an initial semantic fixture version, not a correction.
