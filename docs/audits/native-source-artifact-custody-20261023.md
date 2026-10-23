# Checked Source capture custody

Issue [#6276](https://github.com/dancxjo/conduit/issues/6276) follows the Source
inspection delivered by #6274. Its successful Candidate's x86 product retained
the complete journey receipt but omitted the separate checked Source PNGs and
manifest. The ordinary 27-frame gallery was present. Capture hashes could not
recover the missing bytes.

The x86 packager now stages `patchbay-checked-source` before sealing the product.
Its manifest must equal the checked Source section of the verified journey;
that journey already shares exact source commit and image SHA256 with the
verified paired product evidence. The enclosing product receipt retains both
files under that source and image identity. The capture set keeps the existing
collector bound of 260 (twice its 128-page bound plus four). No runtime,
selection, graph, heap, image, or timing contract changes.

Every capture has a unique safe checkpoint and matching PNG basename. Staging
checks regular files, exact file inventory, byte lengths, SHA256, RGBA8 PNG
headers and declared dimensions. It refuses missing or substituted files,
manifest disagreement, duplicates, escaping names and symlinks. Complete input
validation precedes copying, and copied bytes are compared with the validated
input. Journeys without the Source section retain their existing behavior.

The focused packager controls use the seven real PNGs and manifest retained
from measured Source `f88b3c481b20b296195b4ac8920add42078fd6ca`. They demonstrate
custody and refusal behavior, not a new product run. The supported validation
entrance is `cargo +stable xtask ci pipeline preflight <dev-base>`; it discovers
the focused controls with the other pipeline contracts.

Closure still requires the new hosted artifact, independent PNG CRC, dimensions
and decoded pixel hash verification, representative screenshots, normal merge
and relevant accepted-source checks. This packager's header/file-hash checks do
not independently recompute decoded pixel hashes. Earlier retained images must
not be relabelled as fresh CI captures. Source execution, Edit authority,
physical hardware and human enactment remain separate claims.
