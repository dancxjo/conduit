# Visual evidence

**[Explore the current ConduitOS visual journey →](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)**

The [current-product truth surface](https://dancxjo.github.io/conduit/current-product.html)
names the exact accepted release and publication behind this journey, states
whether they lag development, and links the latest proof receipts.

Thirty-two real QEMU screenshots show boot, current host offers, body and play lifecycle, a
hot-plugged line, Tour, and Patchbay interaction. Each checkpoint explains the
action, visible result, concepts, and asserted behavior, with links to the exact
capture provenance. This is the published accepted journey, not a fresh capture
from this documentation review. It demonstrates emulator execution; physical
hardware has a separate proof boundary.

## Reproduce the ConduitOS journey

Publication verifies a complete, correlated 1280 by 800 RGBA8 journey from the
exact Crèche-produced Spore, then includes it in the accepted Pages carrier.

The journey page presents all thirty-two real screenshots inline in transition
order. Each checkpoint separately explains what a user can see, the action that
led there, the semantic behavior the harness proved, and the Conduit concepts in
view. Its image also links to a focused provenance page with exact manifest
correlation.

Refresh the source artifacts by running the ordinary repository-development
entrance:

```sh
cargo xtask conduitos journey-proof
```

That proof replaces `target/conduitos/x86_64/journey-frames/manifest.json` and
its checkpoint PNGs during one real QEMU journey. Publication runs
`tools/ci/stage-conduitos-pages-evidence.mjs` against those same artifacts; it
refuses an incomplete journey, a missing, duplicated, or renamed checkpoint,
unexpected dimensions or pixel format, and PNG bytes that do not match the
manifest. There is no separately maintained documentation screenshot set.

The published walkthrough is available at the
[current ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/).
The gallery also retains the same narrated walkthrough and focused provenance
pages under the commit-addressed journey index. Emulator pixels remain
documentary evidence and never imply physical hardware acceptance.

## Evidence manifests

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
the proof step, pinned browser/rendering environment, presentation revision, plan,
active play, manifestation/renderer identities, and the semantic disposition that
was asserted before capture. Wall-clock time is deliberately absent from evidence
identity.

A successful suite with all required declarations produces `complete`. A failing
or interrupted suite may retain files, but its manifest is
`diagnostic-incomplete`. Missing required evidence also writes an incomplete
manifest and fails evidence validation. Consumers must publish only `complete`
manifests. The manifest format and validation belong to `xtask`; CI may transport
the resulting directory but does not define its meaning.

The manifest contract originated in #821; the first Patchbay captures were
added through #822.

Audio exhibits use the same envelope without pretending that sound is a
screenshot. `cargo xtask host journey-hears-speaks` declares `audio` outputs
for its input PCM, listenable input WAV, and synthesized output WAV, alongside
machine-readable recognition, response, and same-play receipts. Complete
verification requires that exact six-output set, audio media types, RIFF/WAVE
framing for both listenable files, and shared plan/play provenance. Publication
and CI transport remain separate acceptance gates; a local complete manifest
does not itself make an exhibit current or accepted.

Provider-backed audio is deliberately a host-local rung on `forebrain` and
`victus`, not a reason to copy multi-gigabyte model stores into ordinary CI.
Create a private JSON profile on each machine and run:

```sh
cargo xtask host journey-hears-speaks-local --locked \
  --profile /absolute/path/hears-speaks-provider.json \
  --output target/journeys/hears-speaks
```

The profile schema is `conduit.journey/hears-speaks-local-provider@1`. It names
the exact host, already-installed Whisper executable and model, input PCM,
already-local Ollama model, admitted memory, and already-installed Piper
executable, voice, config, and optional library directory. Each executable,
model, voice, and config entry has a corresponding SHA-256 field; the Ollama
entry uses `ollama_model_content_identity`. The entrance accepts only
`forebrain` or `victus`, verifies every declared digest before starting, and
uses provider discovery that refuses an absent Ollama model. It contains no
download or fallback path. This follows Tongues' separation between small
checked-in contracts and locally installed, checksum-identified model assets;
the license of a provider executable never implies the license of its weights.

The retained `receipt.json` records the discovered Whisper executable/model,
Ollama runtime/model, and Piper executable/voice/config identities. Thus the
same six-output evidence format proves which locally installed provider bundle
actually ran without publishing that bundle. Use a separately reviewed local
profile on each host; copying a profile between machines is intentionally
refused by its exact `host` field.

After a release head has passed promotion, an operator may admit the small
evidence directory without uploading any provider assets:

```sh
tar -C target/journeys/hears-speaks -czf hears-speaks.tar.gz \
  input.pcm input.wav recognition.json response.json output.wav receipt.json manifest.json
sha256sum hears-speaks.tar.gz
gh release create "journey-evidence/$ACCEPTED_SOURCE_SHA" \
  hears-speaks.tar.gz --target "$ACCEPTED_SOURCE_SHA" \
  --title "Hears and Speaks evidence for $ACCEPTED_SOURCE_SHA"
gh workflow run admit-hears-speaks.yml \
  -f accepted_source_sha="$ACCEPTED_SOURCE_SHA" \
  -f promotion_run_id="$PROMOTION_RUN_ID" \
  -f evidence_sha256="$ARCHIVE_SHA256"
```

The trusted default-branch workflow accepts only a successful promotion head
whose tree equals current `main`, downloads that run's already-sealed complete
Pages carrier, verifies the archive digest and exact evidence manifest, then
reseals and deploys the full site with the audio gallery added. A later commit
cannot reuse the archive because the manifest, release tag, promotion run, and
accepted tree must all agree. The archive contains only the bounded outputs;
Whisper, Ollama, Piper, and their model stores remain local to the machine that
ran them.

The bounded gallery publisher accepts a verified audio exhibit only through
`cargo xtask evidence gallery --hears-speaks-evidence-root <directory>` and
only when its manifest is bound to the same accepted commit as the other
gallery inputs. It copies the exact declared files into both commit-addressed
and `current/hears-speaks/` pages with native browser audio controls. Omitting
that input clears a stale current audio exhibit instead of silently carrying
it across releases.

The One form, Two fronts sibling uses a separate four-output manifest. Its
supported entrance is `cargo xtask evidence one-form-two-fronts`, which feeds
one deterministic front-door presentation into the native software renderer
and the pinned Chromium DOM/SVG renderer. It retains `native.png`,
`native.json`, `browser.png`, and `browser.json`; complete verification requires
exact renderer and manifestation provenance plus the same presentation identity,
revision, and semantic basis across both receipts. The renderer plans, plays,
Manifestations, and pixels remain deliberately distinct.

The gallery accepts that sibling only through
`--two-fronts-evidence-root <directory>` bound to the same accepted commit as
the other inputs. Its side-by-side page states that neither pixel equality,
physical-display output, nor human perception is established. Omitting the
input clears stale `current/one-form-two-fronts/` content.

Little Life uses a separate six-output manifest rather than consuming a
ConduitOS capture slot. `cargo xtask evidence little-life` retains PNGs at
generations 0, 1, 8, and 32, the complete 32-generation scalar-field terminal
transcript, and the ordinary plan/play execution report. Generation zero is
the deterministic Orbium seed lowered through the semantic gray8 bitmap
contract. The later PNGs are derived from the exact cells emitted by the
installed std scalar-field terminal presentation. They do not claim a native
graphical manifestation, physical display, or human perception.

## Canonical Patchbay camera

The documentation renderer is only the `chromium` project in
`proof/browser/patchbay-html.playwright.config.mjs`. It uses Playwright 1.62.0, matching the
pinned `mcr.microsoft.com/playwright:v1.62.0-noble` CI image, a 1366 by 768 CSS
pixel viewport, device scale factor 1, `en-US`, `UTC`, dark color scheme,
reduced motion, and the named DejaVu Sans font supplied by that pinned image.
The proof asserts that the font is loaded before capture. The current
configuration runs Chromium with one worker and zero retries.

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

## Accepted Patchbay views

These stable links advance only when the trusted main-only publisher accepts a
new exact commit. Each image links to its provenance page. The prose states the
same essential result because the images document a rendering; they do not
define semantic success.

The overview correlates the checked form graph with the same authoritative
structure exposed by Patchbay.

[![Current accepted Conduit Patchbay overview](https://dancxjo.github.io/conduit/current/patchbay/overview.png)](https://dancxjo.github.io/conduit/current/patchbay/overview/)

After keyboard selection succeeds through an admitted interaction play, the
selected gear is correlated across the structural list, graph, and inspector.

[![Current accepted Patchbay state with one gear selected and correlated in the inspector](https://dancxjo.github.io/conduit/current/patchbay/selected-gear.png)](https://dancxjo.github.io/conduit/current/patchbay/selected-gear/)

After an ordinary control invocation succeeds, Patchbay exposes the correlated
Interaction plan, play, and disposition alongside the resulting presentation.

[![Current accepted Patchbay state after an interaction play succeeds](https://dancxjo.github.io/conduit/current/patchbay/interaction.png)](https://dancxjo.github.io/conduit/current/patchbay/interaction/)

After renderer delivery is lost, the UI reports disconnection while retaining
the last accepted presentation revision and exact plan rather than inventing a
new success or erasing the known state.

[![Current accepted Patchbay state retaining its exact plan after renderer delivery loss](https://dancxjo.github.io/conduit/current/patchbay/disconnected.png)](https://dancxjo.github.io/conduit/current/patchbay/disconnected/)

`cargo xtask evidence docs-verify` rejects missing, duplicated, immutable-commit,
or ephemeral-artifact references. At publication, the same command additionally
requires each stable image to match the exact current commit bytes and requires
its page to expose that commit's provenance before Pages can deploy.

## Human review, not pixel authority

`proof/browser/patchbay-html.spec.mjs` produces five canonical images, four shown above only after the
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

`cargo xtask conduitos prove --arch x86-64 --evidence-root <directory>` can
emit one bounded UTF-8 console transcript after the existing x86_64 proof has
validated its boot sign, kernel sign, Observatory snapshot, exact semantic
presentation, and terminal QEMU debug exit. The ordinary proof remains the
acceptance authority; capture is not triggered by a sleep or an image timer.

The manifest classifies this artifact as `console-transcript` and records the
exact commit, x86_64 architecture and accepted P5 rung, `freestanding-emulator`
proof class, QEMU executable/version and finite machine profile, firmware,
host/boot, plan/play, kernel artifact identity/digest, semantic trigger, output
digest, and 256 KiB transcript ceiling. Its physical-evidence field is
explicitly false. No width or height is invented for a console transcript.

The console-transcript evidence set contains exactly one required transcript.
Verification rejects missing semantic markers, an incomplete terminal line,
wrong proof/rung/machine facts, a physical claim, digest drift, extra files, or
an oversized output. CI invokes the same `cargo xtask` entrance and retains the
exact-head directory as an Actions artifact.

The static gallery accepts this evidence only when its manifest commit equals
the simultaneously verified Patchbay evidence commit. When supplied to the
gallery command, it writes separate current and commit-addressed ConduitOS
pages whose heading warns that the transcript is emulator evidence, not
physical-hardware evidence. This is the earlier console-only proof surface;
its zero-framebuffer profile does not describe the graphical journey above.
