# Original numeric resource ownership composition

`PreparedNumericResourceOwners` retains exact already-admitted original model,
Plan, literal Source, tensor resources, and upstream byte buffers. Before it
constructs either handle array, it inventories their exact requested capacity
and the complete supported Plan/model/resource payload and requires preparation,
retained-payload, and shared-allocation-count quotas. Unsupported Plan owner
classes refuse. Preflight allocates nothing and performs no Source re-admission,
model-to-tensor semantic correlation, Plan verification, or normalization.

All original handles/order remain available through borrowed accessors. Physical
payloads are deduplicated by Arc allocation identity: resource roots, descriptors,
model/tensor/buffer backing bytes. Equal bytes at another address remain distinct
owners and receive separate charges. Logical slices never substitute for the
whole retained backing allocation. Upstream input buffers remain retained rather
than disappearing from the receipt when another owner is prepared.

The receipt reports shared allocation count separately. Arc allocation headers,
allocator bookkeeping, the returned inline aggregate owner, and stack are NOT
included in payload; the actual immutable host driver must charge its reviewed
header/resource profile before claiming whole working admission. Upstream file
loading, model/Native adoption, checker/factory/Back preparation and their peaks
remain distinct. This aggregate must not duplicate their physical retained
payload charges when the same original Arcs are composed.

Actual complete original FARGAN ModelSignature family admission and full model
adoption, plus original archived958 Plan/literal Source, passed the test. Two
separately admitted tensor resource roots share one descriptor and the whole
original model backing. Duplicate original handles are retained. A separately
allocated equal-byte backing and equal-byte descriptor are charged distinctly.
The first aggregate requests exactly72B for its handle arrays and reports
50,865,377B retained payload/eight shared allocations. All three one-under quotas
refuse with zero allocations. Actual source/model/Plan/resource/buffer pointers
are preserved, and reservation/accessors are allocation-free. Four original
model/adoption/aggregate tests pass; AI library and tracked tests pass Clippy.
This remains component evidence, not full trained public execution or #5218.

The five remaining direct numeric factories (guard, U16 profile, Flow pair,
nominal weakening and closing structured pair) now expose the same concrete
`prepare_with_inventory` boundary as the earlier numeric factories. Original
selection/Source/placement guards and legacy trait methods are retained.
Their full construction reservations and all-dispatch runtime tests are separate
from this retained Back inventory checkpoint.
