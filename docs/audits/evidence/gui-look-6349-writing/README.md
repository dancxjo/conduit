# Memory Lantern writing surfaces

Source `8003d90161fb913d073d67666dec1d8be2d6e870` corrects an actual Workspace editor inheriting the oversized signal display's 56px monospace text. The native textarea now consumes the shared Host/Look font, surface, ink and radius tokens at a user-root minimum, with explicit platform forced colors. Workspace supplies a bounded 48rem writing measure. The picker/review text and title clamp also use root-relative sizes.

The retained `proof/browser/workspace-writing-look.mjs` runs against an explicitly staged product. Actual Birth/Wake and Unicode editing retain the editor instance, revision, selection and Body/Plan/Play across Editorial, Engineering and Phosphor **chrome preferences**. These are not canonical resident Look-selection receipts. At 390px, a 32px root produces actual 32px editor text and 26px picker text, with visible focus, platform colors, another acknowledged authoritative edit, no document horizontal overflow and zero browser errors. The textarea scrolls its long content normally; the tall narrow page is not claimed to fit one viewport.

All five existing editor contract tests and the Look wire checks pass. `browser.log`, `report.json` and four screenshots retain the actual run. The staging script uses current source assets verified byte-for-byte in the report, but explicitly reuses the recorded Workspace WASM/catalog from the earlier activity proof. This slice does not rebuild Rust/WASM or establish new native/CI acceptance. The runtime Quantity fault #6387 and cross-component canonical Look selection remain open.

Normal source internal reproduction: stage with `targets/browser/tools/stage-browser-workspace.sh` using the retained runtime/release assets/catalog, then `node proof/browser/workspace-writing-look.mjs STAGED_PRODUCT EVIDENCE_DIRECTORY`. These are internal proof conveniences; supported product/workspace entrances remain unchanged.

Maintenance: audited inactive task-owned incremental directories reclaimed 312,901,632 bytes. Active/unique evidence and source were retained; the cleanup audit is included.
