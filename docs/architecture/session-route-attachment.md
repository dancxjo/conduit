# Logical sessions and line attachments

Issues #500 and #618 separate a logical framed session from the exact admitted
line currently attached to it.

`SessionIdentity` is line-neutral. It binds the protocol, plan, source and sink
fragments and active plays, Connection, exact host and boot pair, value kind,
and admitted item/byte bounds. Changing any of those facts changes or invalidates
the logical session.

`LineAttachment` preserves the selected `LineId`, lower binding and base
identities, initialized base instance, host/boot pair, endpoint identities, and
finite limits. It can be constructed only from an `AdmittedLine` already sealed
for the cord. Current availability is a separate sign and is not serialized as
plan or attachment identity. Platform adapters transport frames; they do not
create lines, plans, logical identities, or selection policy.

Session Hello frames encode both the line-neutral logical identity and the exact
line attachment. `SessionMachine` continues to own ordering, pressure, delivery,
cancellation, failure, terminal state, and sequence truth.
