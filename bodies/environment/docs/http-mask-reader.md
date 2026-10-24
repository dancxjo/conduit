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
Native Root continues to use its bootstrap route, so this change does not claim
that native HTTP has been activated. Its actual non-Clone reader, route admission,
withdrawal and complete service backing still need to be connected explicitly.

`cargo xtask prove environment http` covers the exchange and Mask contracts.
The explicit-provider tests use the real prepared semantic Face and actual
request parser/partial-I/O fixture, with zero recurring Rust allocations. They
also check bootstrap refusal, missing reader custody, unrelated subject properties,
unsealed input and immutable Face preservation. HTTP tests establish CPU behavior;
network sockets, browser membership and physical deployment remain separate proofs.
