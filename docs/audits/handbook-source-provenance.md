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
