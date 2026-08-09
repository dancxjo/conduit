# Plan-sealed Lines

Issues #499 and #618 establish an exact, finite, ordered set of Lines for every
remote Cord. A Line is the Host-offered connectivity realization; a Cord keeps
the semantic relationship and typed Port identities independent of it.

`LineOffer` keeps `LineId`, the lower `LinkBinding`, the explicit
`LineContract`, and a current `LineAvailabilitySign` distinct. `AdmittedLine`
contains only immutable facts sealed into Plan identity. Its contract states
scope, traffic shape, duplex, ordering, reliability, continuation, security,
and the binding's finite payload, frame, buffering, and in-flight limits.

Candidate order is Plan identity. Each admitted Line must independently cover
the Cord bounds. Availability Signs remain outside the Plan and cannot add,
remove, reorder, or mutate admitted Lines. Local Cords have no selected or
admitted Line; remote Cords have one selected Line and at least one admitted
Line. There is no legacy single-binding facade.
