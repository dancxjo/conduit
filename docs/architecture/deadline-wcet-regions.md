# Deadline and WCET regions

Conduit supports general finite computation and stricter deadline-bounded
regions in the same Form. The boundary is an admission rule, not a second
language or execution engine.

## Separate facts

These facts must remain distinct:

| Fact | Meaning | Sufficient for a deadline? |
| --- | --- | --- |
| finite semantic domain | the values a computation may represent are bounded | no |
| finite resource bound | storage, queue, or other admitted consumption is bounded | no |
| maximum operation count | the selected realization has a finite work bound | no, by itself |
| WCET basis | the selected realization has an exact worst-case time on this target | required |
| deadline region | a composition whose transitive timing/resource basis was admitted | result |

A bounded parser over 16 MiB can be ordinary valid Form work while remaining
ineligible for a 50 µs motor step if it has no compatible WCET basis. No
semantic-unboundedness concept is needed to describe that result.

## Admission and composition

The planner admits a deadline region only after every selected realization in
its transitive dependency list supplies a non-empty timing basis. It sums the
individual worst-case durations with checked arithmetic, sums their admitted
resource units, and refuses overflow, missing basis, resource excess, or a
total above the region deadline. Capacity and operation facts remain available
to ordinary finite-form analysis but are never substituted for WCET.

This is conservative composition: an unknown child makes the parent unknown.
An implementation may be valid in a general-purpose Form and still be
rejected in the region. Continuous finite-state control is eligible when each
step has the same compatible finite basis; an indefinite lifetime does not
turn that per-step proof into a whole-lifetime completion claim.

## Replanning

The admitted region records its exact identity, deadline, resource ceiling, and
timing-basis identities. A replacement must retain the region identity,
deadline, and resource ceiling and independently pass the same admission checks. A slower selected
realization therefore produces a machine-readable refusal; it cannot silently
replace the old Plan. The old admission remains immutable and usable for
diagnosis or explicit replacement planning.

The contract proves planner admission only. It is not a physical timing claim,
an average-runtime estimate, a simulation promotion, a scheduler guarantee, or
permission for a hidden host callback inside the region. A physical or
platform-specific claim needs its own exact target, measurement method, and
proof class.

The executable conformance surface is the `conduit-planner` WCET contract and
its unit tests. It deliberately does not add a global real-time mode, ban
large finite work outside a deadline region, or create a second runtime.
