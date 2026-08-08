# Logical sessions and route attachments

Issue #500 separates a logical framed session from the exact carrier route
currently attached to it.

`SessionIdentity` is carrier-neutral. It binds the protocol, Plan, source and
sink fragments and active plays, Connection, exact logical host and boot pair,
value kind, and admitted item/byte bounds. Changing any of those facts changes
or invalidates the logical session.

`RouteAttachment` preserves the exact selected link binding, provider,
initialized provider instance, host/boot pair, link endpoint identities, and
link limits. It is admitted only through a currently `Ready` `LinkBinding` whose
immutable `BoundLink` is already in the connection's sealed candidate set. Its
limits must cover the logical session bounds. Carrier adapters transport frames;
they do not create Plans, logical identities, or attachment policy.

Session wire version 2 encodes the carrier-neutral identity in every frame and
the exact route attachment in `Hello`. Version 1 bytes are not reinterpreted by
the version 2 decoder. `SessionMachine` continues to own ordering, pressure,
delivery, cancellation, failure, terminal state, and sequence truth. This slice
does not select routes or define failover/resumption behavior.
