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
