# Executed Native laws in the existing checked interpreter (#6406)

The explicit checked-Native profile retains one actual Source-law registry.
It validates complete original Native input before the body, checks primitive
Native frames and structured results before their parents can consume them,
and keeps the existing arithmetic evaluator. Every selected constructed value
retains its actual owner, including Native records/variants and nested Core
optional/tuple/collection values. Only selected conditional branches execute.
Structured tape and constant-child shortcuts are disabled for this profile;
legacy constructors retain their existing paths and arithmetic semantics.

Aggregate guard counts, canonical buffer capacities, scopes, predicates,
contracts and conservative Native-admission steps must fit explicit preparation
limits. Metadata Source limits remain unchanged. Each guard is bounded before
allocation and actual retained capacities are charged before installation;
one rejected candidate is the maximum transient preparation excess. Reported
canonical capacities exclude metadata, path/index storage and general heap.
The complete prepared interpreter retains terminal failure/cancellation across
all branches; failed Native admission evidence is not reset to permit reuse.

The focused run passes 26 tests, including nine new guard tests, five arithmetic
controls, five Native occurrence tests, two optional tests and five tape controls.
It checks unused invalid Native input, invalid temporary before scalar projection,
invalid later equality values, selected/dead branches, Native variants, Core
optionals, nominal primitive refinement, missing owners, aggregate pressure,
terminal cancellation, forged arithmetic hints, hosted Send and allocation-free
successful reuse/refusal. Formatting and strict Plot checks pass. The initial
fixture constructor error and the large-enum lint refusal (repaired by boxing
guards during preparation) are retained beside the final focused result.

The actual Std contract compile and four original G2 guarded preparations are
running separately. These results do not authenticate a law expression envelope,
produce its canonical Kind, install an automatic Std offer, or prove the four
ordinary G2 Plan/Play/Host Call journeys. Those remain required for #6406.
