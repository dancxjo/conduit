# Sealed route candidates

Issue #499 replaces a remote connection's single nominal route with an exact,
finite, ordered set of functionally compatible routes. Planning policy chooses
that set from observed boot-scoped links. Providers only report link facts; they
do not choose, reorder, or expand the set.

`BoundLink` contains the immutable endpoint, provider-instance, authority,
credential-reference, and limit facts committed into Plan identity. Candidate
order is meaningful and identity-bound. Every candidate independently satisfies
the connection's admitted item and byte bounds.

`LinkObservation` contains mutable availability evidence. Readiness and the
currently selected candidate do not alter Plan identity. A runtime may attach
only a candidate already present in the sealed set; #499 does not add route
selection, failover, retry, resumption, or a second session state machine.

The existing `provider` and `link_binding` fields remain as a temporary
single-route/current-observation facade for the existing USB and WebSocket
session adapters. An empty candidate list is accepted only for legacy decoded
plans and means that exact single binding. New remote plans always seal at least
one `BoundLink`. Follow-up issues #500 and #501 own runtime selection and
transport integration.
