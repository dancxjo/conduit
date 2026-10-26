# Repository documentation sweep, 2026-10-25

**Base inventory:** `dev` at `b69ce6940223a00126a058564c66c78ef7bb359b`.
**Change:** draft [documentation PR #6497](https://github.com/dancxjo/conduit/pull/6497).
**Scope:** documentation/navigation and historical references, not runtime or
hardware acceptance.

## Inventory and method

The complete baseline Git tree contained **694 Markdown documents** spanning
approximately **4.09 MB** of text: 286 under `docs/audits/`, 18 under
`docs/history/`, and 390 elsewhere (including current guides, component
README files, target instructions, proof records, and the handbook).

The sweep enumerated the complete tree and read document content through
the connected GitHub repository. It checked inline Markdown destinations
against tracked files or directories, and checked the handbook's 144
`[[wiki links]]` against tracked wiki page names. One transient blob-read
failure was retried successfully. Key public-facing documents were examined
editorially against the current canon, source contracts, and proof boundaries.

**Limits:** this is not a claim that every sentence in 694 documents was
manually revalidated against current implementation. The link pass did not
test external websites, fragment anchors, reference-style link definitions,
or generated-site routing. The handbook build, full test suite, emulator,
and physical hardware were not run for this documentation-only review.

## Editorial corrections

- The README now leads with **one continuing computer**, distinguishing a
  Body's identity from Host/Boot and explaining Plot/Plan/Play without making
  the front page a glossary.
- Body continuity no longer implies perpetual execution or mandatory growth.
  Purpose obligations, fulfillment readiness, Plot completion, lull, and
  retirement have different owners. Readiness itself carries no lifecycle
  authority; the new [Body purpose and fulfillment](../architecture/body-purpose-and-fulfillment.md)
  page identifies the actual finite source and derivation.
- Handbook links leaving `wiki/` for repository-only documents use stable GitHub
  `dev` URLs; relative cross-directory Markdown links can become broken
  generated-site routes after publication.
- The [documentation index](../README.md) now routes by reader intent.
  [Documentation policy](../documentation-policy.md) separates present
  contract, development capability, roadmap, and historical proof.
  Design, performance, and audit collection indexes state their different roles.
- [STATUS](../../STATUS.md) places its capability orientation ahead of the
  long protected-execution evidence, which is preserved. The
  [roadmap](../roadmap.md) is explicitly a dated navigation snapshot and
  not a substitute for live issue state.

## Checked cross-reference exceptions and disposition

| Source | Finding | Disposition |
|---|---|---|
| [Face grammar](../architecture/face-grammar-conformance.md) | Three `presentation-waist` references had been superseded by `face-waist` | Update the source path and its two JSON links to the existing fixture directory |
| [Native Patchbay GUI history](../history/architecture/native-patchbay-gui.md) | Two 'current' interaction links used retired `products/patchbay` paths | Point to `plots/patchbay/workbench/model` and retain the August checkpoint scope |
| Five dated native/canonical [audit records](README.md) | Links to referenced `evidence/…` directories that are not in this Git tree | Keep the checkpoint claims; replace dead Markdown links with the exact historical path and explicit not-in-checkout notice |
| `wiki/Start-here.md` | `index.html` is a generated Handbook entrance, not a tracked repository file | Retain deliberately; it is not a repo-relative source link |
| `targets/conduitos/tools/riscv64-firmware/publisher-notice.md` | A copied upstream publisher notice links to local `LICENSE-APACHE` and `LICENSE-MIT` files not present beside it | Leave the third-party legal notice unchanged; authoritative upstream license URLs remain in the notice. Legal artifact remediation, if needed, belongs to the vendor/asset owner |

Historical acceptance receipts, benchmark logs, and negative physical
observations were not replaced with new claims. In particular,
[MCU draft PR #6457](https://github.com/dancxjo/conduit/pull/6457) is
in-flight work, not a feature silently promoted into `dev` by this sweep.

## Future maintenance boundary

A documentation PR should specify whether it changes a present instruction,
normative architectural intent, a dated audit, or historical context. In a
future tool-integrated pass, validate built handbook navigation and fragment
anchors, check referenced third-party assets/licenses with their owner, and
recheck time-sensitive target-specific statements against fresh exact
receipts. This inventory and link check are a finite editorial baseline, not
an assertion that documentation can never become stale again.
