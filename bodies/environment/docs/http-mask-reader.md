# HTTP Mask over a retained Face

`face_response` renders an immutable, validated environmental Face into bounded
HTML or JSON. Fahrenheit and percent text are Mask representations; the Face
retains its first-class quantities, identity, revision and provenance. Property
lookup is scoped to the `environment` subject. Other Body subjects cannot shadow
its freshness or quantities. Invalid or unsealed Faces refuse explicitly.

`exchange_with_reading` accepts the Root-selected synchronous reading callback.
It requests one owned response after parsing the complete request and before
socket writes await. Partial writes do not re-read, re-render or retry. No Face
borrow crosses a socket await. Missing reader custody can return `None`, yielding
503 without falling back to a different publication. Root must independently
admit the reader, route, clock, response backing and connection work/deadline.
A supplied callback does not override the existing bootstrap route's reading
refusal or create Body membership, a Line, a sensor or radio authority.

The existing `exchange` entrance retains the attended bench's publication owner.
The experimental native Face profile explicitly connects its retained reader to
bounded Show custody and prepares response backing before serving. Other native
profiles retain the bootstrap reading refusal. See [native integration](native-http-mask.md)
for its finite scope; browser membership and combined network admission remain separate.

`cargo xtask prove environment http` covers the exchange and Mask contracts.
The explicit-provider tests use the real prepared semantic Face and actual
request parser/partial-I/O fixture, with zero recurring Rust allocations. They
also check bootstrap refusal, missing reader custody, unrelated subject properties,
unsealed input and immutable Face preservation. HTTP tests establish CPU behavior;
network sockets, browser membership and physical deployment remain separate proofs.

`PreparedHttpShow` resolves the Mask once into finite owned presentation material.
It retains bounded text and Face correlation, without retaining a Face borrow,
quantity decoder or semantic observation owner. Its caller must independently
withdraw this material when the basis, reader custody or admitted lifetime ends;
rendering age text does not refresh the Face or extend that lifetime. The CPU64
owner occupies 248 inline bytes and requires no heap allocation during preparation
or rendering. Native size and complete service backing remain separate proof.

Time labels follow the Face. A sampled coordinate appears as
`sampled_at_boot_ms`; an observed coordinate appears as `observed_at_boot_ms`
and does not become acquisition time. The other coordinate is null, while
`age_basis` names the selected reference. Without either coordinate, both times
and age are null. This does not fill the native recorder's acquisition-time gap.

`HttpShowMailbox` provides one bounded publication owner and separate Root
withdrawal custody for resolved Show material. Claiming it does not install a
route or confer Body, clock, Face or observation authority. Root independently
supplies the matching basis and a finite publication/lifetime budget. Each
accepted Show has an explicit half-open same-Boot lease. Missing, expired,
not-yet-valid or withdrawn material returns unavailable; expiry does not rewrite
Face freshness. Stale revisions and invalid leases preserve prior material and
quota. Withdrawal is permanent for this mailbox and fences late publication;
producer drop also withdraws it. No replacement producer or retry is implicit.
The CPU64 mailbox occupies 280 inline bytes and allocates no heap. The native Face profile connects this custody explicitly. Socket deadlines remain
bounded independently; full continuing appliance admission is not established.
