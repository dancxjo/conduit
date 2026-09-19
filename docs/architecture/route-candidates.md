# plan-sealed lines

Issues #499 and #618 establish an exact, finite, ordered set of lines for every
remote cord. A line is the host-offered connectivity realization; a cord keeps
the semantic relationship and typed port identities independent of it.

`LineOffer` keeps `LineId`, the lower `LinkBinding`, the explicit
`LineContract`, and a current `LineAvailabilitySign` distinct. `AdmittedLine`
contains only immutable facts sealed into plan identity. Its contract states
scope, traffic shape, duplex, ordering, reliability, continuation, security,
and the binding's finite payload, frame, buffering, and in-flight limits.

Candidate order is plan identity. Each admitted line must independently cover
the cord bounds. Availability signs remain outside the plan and cannot add,
remove, reorder, or mutate admitted lines. Local cords have no selected or
admitted line; remote cords have one selected line and at least one admitted
line. There is no legacy single-binding facade.
