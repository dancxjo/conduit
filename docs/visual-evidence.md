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

## Accepted current views

These stable links advance only when the trusted main-only publisher accepts a
new exact commit. Each image links to its provenance page. The prose states the
same essential result because the images document a rendering; they do not
define semantic success.

The overview correlates the checked Form graph with the same authoritative
structure exposed by Patchbay.

[![Current accepted Conduit Patchbay overview](https://dancxjo.github.io/conduit/current/patchbay/overview.png)](https://dancxjo.github.io/conduit/current/patchbay/overview/)

After keyboard selection succeeds through an admitted interaction Play, the
selected Gear is correlated across the structural list, graph, and inspector.

[![Current accepted Patchbay state with one Gear selected and correlated in the inspector](https://dancxjo.github.io/conduit/current/patchbay/selected-gear.png)](https://dancxjo.github.io/conduit/current/patchbay/selected-gear/)

After an ordinary control invocation succeeds, Patchbay exposes the correlated
Interaction Plan, Play, and disposition alongside the resulting presentation.

[![Current accepted Patchbay state after an interaction Play succeeds](https://dancxjo.github.io/conduit/current/patchbay/interaction.png)](https://dancxjo.github.io/conduit/current/patchbay/interaction/)

After renderer delivery is lost, the UI reports disconnection while retaining
the last accepted presentation revision and exact Plan rather than inventing a
new success or erasing the known state.

[![Current accepted Patchbay state retaining its exact Plan after renderer delivery loss](https://dancxjo.github.io/conduit/current/patchbay/disconnected.png)](https://dancxjo.github.io/conduit/current/patchbay/disconnected/)

`cargo xtask evidence docs-verify` rejects missing, duplicated, immutable-commit,
or ephemeral-artifact references. At publication, the same command additionally
requires each stable image to match the exact current commit bytes and requires
its page to expose that commit's provenance before Pages can deploy.

## Human review, not pixel authority

`patchbay-html.spec.mjs` produces the five canonical images above only after the
corresponding semantic browser assertions pass. The images remain available for
human inspection and exact-commit comparison, but their pixels are not an
acceptance gate. Font rasterization, antialiasing, or other presentation-only
differences therefore cannot overrule semantic proof or turn a successful state
transition into failure.

Intentional visual changes remain reviewable in the exact-head evidence artifact
and accepted-main gallery. The manifest binds every image to its producing commit,
camera, fixture, scenario, and asserted semantic disposition without treating a
previous raster as runtime truth.

## ConduitOS console evidence

## Accepted ConduitOS visual journey

These stable links are published only after the frozen promotion boots the exact
Crèche-produced spore and the main-only Pages carrier accepts its complete,
correlated 1280 by 800 RGBA8 visual journey.

[![Current accepted ConduitOS front door](https://dancxjo.github.io/conduit/current/conduitos/x86_64/front-door-ready.png)](https://dancxjo.github.io/conduit/current/conduitos/x86_64/front-door-ready/)

[![Current accepted ConduitOS opened Form](https://dancxjo.github.io/conduit/current/conduitos/x86_64/form-opened.png)](https://dancxjo.github.io/conduit/current/conduitos/x86_64/form-opened/)

[![Current accepted ConduitOS visible result](https://dancxjo.github.io/conduit/current/conduitos/x86_64/result-visible.png)](https://dancxjo.github.io/conduit/current/conduitos/x86_64/result-visible/)

[![Current accepted ConduitOS lulled result](https://dancxjo.github.io/conduit/current/conduitos/x86_64/lulled.png)](https://dancxjo.github.io/conduit/current/conduitos/x86_64/lulled/)

Each image links to a provenance page containing the exact accepted commit and
the manifest correlation context. The gallery also retains all eight named
checkpoints under the commit-addressed journey index. Emulator pixels remain
documentary evidence and never imply physical hardware acceptance.

`cargo xtask conduitos prove --arch x86-64 --evidence-root <directory>` can
emit one bounded UTF-8 console transcript after the existing x86_64 proof has
validated its boot Sign, kernel Sign, Observatory snapshot, exact semantic
presentation, and terminal QEMU debug exit. The ordinary proof remains the
acceptance authority; capture is not triggered by a sleep or an image timer.

The manifest classifies this artifact as `console-transcript` and records the
exact commit, x86_64 architecture and accepted P5 rung, `freestanding-emulator`
proof class, QEMU executable/version and finite machine profile, firmware,
Host/Boot, Plan/Play, kernel artifact identity/digest, semantic trigger, output
digest, and 256 KiB transcript ceiling. Its physical-evidence field is
explicitly false. No width or height is invented for a console transcript.

The complete x86_64 evidence set contains exactly one required transcript.
Verification rejects missing semantic markers, an incomplete terminal line,
wrong proof/rung/machine facts, a physical claim, digest drift, extra files, or
an oversized output. CI invokes the same `cargo xtask` entrance and retains the
exact-head directory as an Actions artifact.

The static gallery accepts this evidence only when its manifest commit equals
the simultaneously verified Patchbay evidence commit. When supplied to the
gallery command, it writes separate current and commit-addressed ConduitOS
pages whose heading warns that the transcript is emulator evidence, not
physical-hardware evidence. No framebuffer capture is claimed: the accepted
x86_64 proof currently reports zero framebuffers, so console evidence is the
truthful first specimen.
