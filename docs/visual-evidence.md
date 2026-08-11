# Proof-native visual evidence

Visual evidence is a human-inspectable presentation of an already-established
semantic proof. It is not an additional runtime and it does not turn a screenshot
into proof.

`cargo xtask prove browser-host` writes `manifest.json` beneath the deterministic
`target/conduit-evidence/browser-host/` root after the suite terminates. An explicit
`--evidence-root <directory>` may select another root for local or CI consumers.
Evidence declarations always name relative paths beneath that root; absolute paths,
parent traversal, symlinks escaping the root, duplicate identities or paths, more
than 64 outputs, and individual outputs larger than 16 MiB are refused.

The versioned `conduit.evidence-manifest/v1` envelope binds the exact Git commit,
proof and suite identities, completion disposition, declared output metadata, byte
length, and SHA-256 digest. Each output carries a scenario identity and may carry
the proof step, pinned browser/rendering environment, presentation revision, Plan,
active Play, manifestation/renderer identities, and the semantic disposition that
was asserted before capture. Wall-clock time is deliberately absent from evidence
identity.

A successful suite with all required declarations produces `complete`. A failing
or interrupted suite may retain files, but its manifest is
`diagnostic-incomplete`. Missing required evidence also writes an incomplete
manifest and fails evidence validation. Consumers must publish only `complete`
manifests. The manifest format and validation belong to `xtask`; CI may transport
the resulting directory but does not define its meaning.

Issue #821 establishes this contract without declaring any captures. Issue #822
owns the first deterministic Patchbay screenshot declarations and their browser
provenance.

## Canonical Patchbay camera

The documentation renderer is only the `chromium` project in
`patchbay-html.playwright.config.mjs`. It uses Playwright 1.62.0, matching the
pinned `mcr.microsoft.com/playwright:v1.62.0-noble` CI image, a 1440 by 1000 CSS
pixel viewport, device scale factor 1, `en-US`, `UTC`, dark color scheme,
reduced motion, and the named DejaVu Sans font supplied by that pinned image.
The proof asserts that the font is loaded before capture. Firefox and WebKit
continue to execute the semantic compatibility test but never write canonical
evidence.

The deterministic in-process Patchbay fixture supplies the rendered state.
`overview.png`, `selected-gear.png`, `interaction.png`, `high-contrast.png`,
and `disconnected.png` are taken in semantic order, only after the assertions
for each named state pass. Playwright disables animation and hides the caret at
the screenshot boundary; it performs no sleeps, masking, redaction, or image
post-processing. After each capture the Chromium test atomically refreshes the
bounded `captures.json` declarations. `xtask` imports those declarations,
requires all five after a successful proof, and writes their exact identities,
rendering inputs, semantic provenance, byte lengths, and SHA-256 digests into
the ordinary evidence manifest.
