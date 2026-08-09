# Logical sessions and Line attachments

Issues #500 and #618 separate a logical framed session from the exact admitted
Line currently attached to it.

`SessionIdentity` is Line-neutral. It binds the protocol, Plan, source and sink
fragments and active Plays, Connection, exact host and boot pair, value kind,
and admitted item/byte bounds. Changing any of those facts changes or invalidates
the logical session.

`LineAttachment` preserves the selected `LineId`, lower binding and Base
identities, initialized Base instance, host/boot pair, endpoint identities, and
finite limits. It can be constructed only from an `AdmittedLine` already sealed
for the Cord. Current availability is a separate Sign and is not serialized as
Plan or attachment identity. Platform adapters transport frames; they do not
create Lines, Plans, logical identities, or selection policy.

Session Hello frames encode both the Line-neutral logical identity and the exact
Line attachment. `SessionMachine` continues to own ordering, pressure, delivery,
cancellation, failure, terminal state, and sequence truth.
