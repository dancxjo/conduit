# Handbook recovery source provenance

Development slice for #5510 / #5453: the recovery picker exposes the exact
original packaged source even when an exact-source-scoped saved correction is
active. Corrections have their own `saved correction` entry rather than being
mislabelled `packaged`. Each entry retains independent unsent editor input;
startup drafts belong to the effective source, not the original package entry.
Rechecking/saving a correction still binds it to the original package bytes.
No Body or continuity record is changed by source inspection.

The absent correction entry reproduced before repair. Two focused pinned
Chromium fixtures pass with one worker and zero retries, through production
`startApplication`: unavailable Host, and refused inventory with a fixture
checker that permits saving a correction. They verify exact original/corrected
text, independent unsent drafts, no storage writes while inspecting and exact
original-source custody when explicitly saving. These use a fake Host/checker;
they do not establish Rust syntax acceptance, actual packaged execution or public
release identity. The historical reported quoted `1kHz` provenance, released
package/digest and public HTTPS corroboration remain unproved; #5510 stays open.

The source inspector now includes a read-only provenance panel with the admitted
application/package identity, selected source origin, packaged resource role and
digest, local-edit state and SHA-256 of the exact displayed UTF-8 source. Missing
manifest evidence remains explicitly null. Source hashing is asynchronous and
cannot block editing or Host startup; a generation fence prevents late digest
results from overwriting newer source evidence. Saving a rescue correction or
installing checked source updates the inspection basis.

Both focused Chromium fixtures pass with independent Node SHA-256 expectations
for package source and edited correction text, matching fixture manifest metadata
and explicit unknown metadata when the Host is unavailable. Explicit save also
clears the local-edit marker. These remain source-inspection fixtures with a fake
Host/checker; no public release/package corroboration is implied by the panel or
these local tests.

Recovery persistence follow-up for #5453: correction saves now serialize against
the latest successfully persisted correction set. A second lesson cannot replace
a sibling correction with the stale opening record, including when its check
finishes while the first write is pending. Failed writes do not update the local
saved set or poison later saves; completions for another selected source do not
replace the current source's check status.

The pending-write regression reproduced before repair. The two provenance/source
fixtures pass, as do two focused Chromium write-order fixtures with the first
write held across a sibling save, then either completed or rejected. One worker,
zero retries. The final record retains both successful corrections or only the
successful sibling after failure. These are production recovery UI with fixture
storage/checker tests; real IndexedDB failure, cross-tab persistence and packaged
runtime acceptance are separate evidence.

Storage-independent inspection (#5453): original packaged lessons, the selector
and the plain source editor now initialize before reading saved corrections.
Delayed storage no longer leaves an empty disabled rescue tool. Exact-source
corrections are applied after the read; lesson selection and per-lesson unsent
drafts entered during that wait survive both a successful read and read refusal.
Host startup still uses the checked effective sources and does not silently
reset continuity.

The delayed-read fixture reproduced the disabled editor before repair. Six
focused pinned Chromium fixtures pass with one worker and zero retries: the
existing provenance and save-order/failure cases plus two held-read cases that
exercise two lesson drafts and successful/refused storage. These exercise real
Handbook orchestration with fixture storage/Host, not packaged WASM or public
release acceptance.

Unsent lesson continuity (#5453): a choice made during the stored-preference
read takes precedence when startup finishes. Per-lesson drafts now remain local
across ordinary selector changes; checking refusal does not discard them. An
installed checked revision clears its corresponding draft only after the source
inventory transition succeeds. Lesson inspection does not select or wake another
Body plot.

The stored-preference overwrite reproduced before repair. Seven focused pinned
Chromium source fixtures pass with one worker and zero retries. The new fixture
SDK exercises delayed preference loading, two unsent drafts, repeated lesson
switching and a rejected source check, with exactly the initial Body select/wake
calls. It proves production editor orchestration against a fixture SDK, not Rust
syntax or actual Body/packaged runtime acceptance.

Source rescue export (#5453): `Download displayed source` saves the exact current
textarea text as a local `.conduit` file. It stays available independently of
Host startup, checker/Body refusal and runtime controls, and exports unsent edits
without writing application storage or resetting continuity. It complements the
separate original retained-record backup; it does not claim to export archived
history or original package bytes when the displayed source has been edited.

The absent export action reproduced before repair. Seven focused pinned Chromium
source fixtures pass with one worker and zero retries. The two provenance cases
use real browser downloads and verify exact multiline Unicode file contents and
filename, while retaining the zero-storage-write assertion. Host/checker remain
fixtures; actual packaged/public acceptance stays open.

Recovery lesson-identity follow-up (#5453): a successful syntax check must still
contain the selected lesson's Plot name before its source can be saved as that
lesson's correction. Renaming the Plot previously saved an unusable correction
under the original name. Recovery now reports `LessonPlotNameChanged` without
writing storage or replacing the editor draft; restoring the name permits save.

The regression reproduced the bad save before repair. Five focused pinned
Chromium fixtures pass with one worker and zero retries: rename refusal/repaired
save, both pending-write outcomes and both original/correction inspector modes.
Fixture checkers now return the Plot names they claim to check. These tests
isolate recovery UI and persistence; Rust syntax, actual Body lifecycle and
packaged/public acceptance remain separate proof obligations.

## Public source observation — 17 October 2026

[Retained receipt](evidence/handbook-public-source-2026-10-17.json) records
read-only HTTPS observation of the current public Handbook. The served pitch
lesson uses bare `1kHz` and `Hz`; its SHA-256 is
`20c581cac1ba8525577fb95975f38b98f4a5831bc8f43f844864f11e32db630e`,
matching both the application's resource declaration and the repository source
at declared runtime revision `572292edf60c6b9054385f351de9159f0ff0fe09`.
The currently served application package digest is
`sha256:7eb6ac585cf747e59bbccc03ced92feb87a7eebc588a02f7dcc9fd2a5e02b8c9`;
independent recomputation matches. Public distribution, release and IMAGE
manifest bytes match their application resource hashes. Their aggregate
digests and IMAGE identity recompute, distribution/IMAGE inventories agree,
and the release links that IMAGE and distribution. The receipt retains exact
URLs, observed hashes, identities, source text and verification scope.

This adds current public content corroboration to the earlier local fixtures.
It does not identify the source of the 11 October quoted-value report, inspect
the reporting browser's cache or retained Body, verify runtime/module payloads,
execute Chromium, or verify a release-train/accepted-main receipt. The current
public observation cannot establish what that browser received historically.
#5510 and its parent remain open. The old issue path `plots/convert-pitch.conduit`
is absent in this checkout; the current source is under
`targets/browser/handbook/plots/convert-pitch.conduit`.

## Public Chromium observation — 17 October 2026

[Browser receipt](evidence/handbook-public-chromium-2026-10-17.json) records a
fresh pinned Chromium 151.0.7922.34 context on the public Handbook. The observed
package digest is
`sha256:fdd92ab3401bb299ed8838c735653e24838ef4abbb3917f2b4b2af113dfba449`,
which differs from the earlier source-only observation; they are separate
package observations, not interchangeable receipts.

Native example selection and Try execute the public bare-quantity pitch lesson
and display `Exactly 1000 Hz`. Editing its source to quoted `"1kHz"` and trying
it yields outer `PlotCheckRefused` with diagnostic `CND-FRM-051`. The editor stays
enabled with the rejected draft. BodyId, selected checked Plot, PlanId, PlayId,
installed plots and the SHA-256 of the complete projected Body evidence remain
identical before/after the refusal. The current admitted output remains visible,
and there are no observed page errors. The receipt includes source, identities,
diagnostic and exact comparison digests.

The first probe expected the wrong outer code (`PlotSyntaxInvalid`) and failed
its assertion after observing the real diagnostic. After correcting that
expectation, one subsequent probe passed. This is a recorded manual browser
observation, not a zero-retry CI/acceptance-gate claim or evidence for unpromoted
`live` changes. Malformed packaged inventory and malformed retained recovery
were not injected. Historical reporting-browser custody and exact release-train/
accepted-main evidence remain unverified; #5453/#5510 remain open.

## Malformed packaged lesson in Chromium — 17 October 2026

[Receipt](evidence/handbook-malformed-package-chromium-2026-10-17.json) and
[opt-in probe](../../proof/browser/handbook-public-malformed-package.mjs) retain
one successful fresh Chromium run using the real HTTPS-delivered runtime. Only
the pitch lesson and its resource/package hashes are intercepted locally. The
public package digest is `sha256:fdd92ab3401bb299ed8838c735653e24838ef4abbb3917f2b4b2af113dfba449`;
the deliberately malformed fixture package is
`sha256:1fea41377bd98cb32bfe797e4bcdc9c0d8448ba76d5ad130c7f67bf16aa9a6ef`.

The quoted-quantity packaged lesson refuses startup while the source editor and
selector remain enabled. The exact malformed lesson is inspectable. Native
editing and Try save the checked canonical correction under its original-source
identity; `body-continuity` remains null before and after saving. Explicit Try
again reloads the same fixture package with the saved correction, admits a Body
and executes the lesson to `Exactly 1000 Hz`, with no observed page errors.
The probe requires an explicit receipt destination and performs no public writes.

This is actual packaged-runtime Chromium execution with a local admission-
consistent malformed source fixture, not evidence of a malformed public
deployment. It does not exercise malformed retained recovery, historical
reporting-browser custody, unpromoted `live` changes or release-train/accepted-
main gates. Those remaining obligations keep #5453/#5510 open.

## Malformed retained source in Chromium — 17 October 2026

[Receipt](evidence/handbook-retained-source-chromium-2026-10-17.json) and
[opt-in probe](../../proof/browser/handbook-public-retained-source.mjs) exercise
the real public package `sha256:fdd92ab3401bb299ed8838c735653e24838ef4abbb3917f2b4b2af113dfba449`
in an isolated fresh Chromium context. A Body is created and paused normally;
only that context's retained pitch source is replaced with quoted `"1kHz"`.
After releasing the tab and reopening, retained recovery refuses with
`CND-FRM-051`, while the editor and selector remain enabled. Exact retained and
canonical packaged sources are independently inspectable.

Native editing/checking accepts the canonical correction without rewriting the
retained continuity record. A native download produces the exact original
record (byte comparison with its JSON serialization); receipt hashes retain
the record/backup comparisons. New-Body replacement is disabled by default,
enabled only after checking consent and disabled again after unchecking. No
replacement is started, and continuity stays unchanged throughout inspection,
checking, export and consent toggling. No page errors were observed.

The initial probe stopped on an ambiguous disclosure text locator; the scoped
locator was corrected before the subsequent successful run. This manual
observation is not a zero-retry acceptance gate. Fault injection is synthetic
and confined to the test context, so it does not identify historical reporting-
browser custody or prove semantic migration, separately archived history backup,
unpromoted `live` changes or release-train/accepted-main gates. #5453/#5510 remain
open despite evidence at the packaged, retained and rejected-edit seams.
