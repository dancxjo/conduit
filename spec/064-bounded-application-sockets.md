# Bounded application socket boundaries

## Contracts

Conduit exposes four optional raw transport operations:

- `conduit.host/net/tcp/connect` opens one client stream session;
- `conduit.host/net/tcp/listen` binds one listener and accepts bounded stream
  sessions;
- `conduit.host/net/udp/connected` exchanges datagrams with one pinned peer;
- `conduit.host/net/udp/datagram` sends and receives individually addressed
  datagrams.

These are application socket boundaries, not network configuration. They never
perform name resolution, interface selection, address acquisition, routing,
firewall mutation, NAT, TLS, HTTP, or public-reachability discovery. The
displaced `net/tcp/socket` and `net/udp/socket` drafts are not aliases.

## Identity and authority

An address is a host-language-neutral value containing an opaque network
resource, sixteen address bytes, and a port. Exact compilation pins target,
bind, listener, peer, network resource, host and topology observations,
provider implementation and artifact, grants and leases, security facts, and
every finite limit. A stale observation or absent grant rejects admission
before bind, connect, listen, accept, send, or receive.

Checking, describing, inspecting, and resolving perform no socket mutation.
A listener grant permits only the planned bind and listen operation; it neither
changes a firewall nor claims local, routed, or public reachability. Required
secure transport is unsupported here and must select an explicit higher TLS
provider rather than degrading to plaintext.

## TCP

TCP data ports carry bounded `std/bytes` chunks. Chunk boundaries are pressure
and scheduling facts, never message boundaries. Each session has an exact
identity. Connect, listen, accept, partial read, partial write, write
half-close, read EOF, refusal, reset, deadline, cancellation, provider loss,
and cleanup remain distinct lifecycle evidence.

The plan bounds concurrent sessions, backlog and accepts, pending operations,
send and receive bytes, chunk and queue bytes, timers, work, evidence, deadline,
and cleanup. Cancellation does not reconnect or retry. Once cancellation,
reset, deadline, or provider loss becomes terminal, cleanup completes within
the planned cleanup bound.

## UDP

UDP ports carry typed datagrams containing address metadata and bounded bytes.
Connected UDP pins one peer. Unconnected UDP requires an explicit destination
per outbound datagram and reports the explicit source per inbound datagram.
Datagram boundaries are preserved.

The plan pins an MTU and whether fragmentation is permitted, and bounds message
bytes, queued messages and bytes, pending operations, timers, work, evidence,
deadline, and cleanup. Oversize rejection, loss, duplication, and reordering
are explicit outcomes or evidence; none is silently converted into TCP-like
delivery or ordering.

## Providers and conformance

The allocator-free deterministic provider uses caller-owned buffers and covers
connect, listen/accept, half-close/EOF, finite send and receive, refusal, reset,
deadline, cancellation before and after commit, buffer and work exhaustion,
provider loss, UDP oversize, loss, duplicate, reorder, and cleanup. Its fault
controls describe transport observations and do not grant authority.

The Linux hosted provider uses only exact numeric loopback resources in its
checked fixtures. It proves normalized equivalence without DNS, firewall
mutation, or reachability claims. A browser or constrained host that lacks an
exact implementation reports unsupported; it does not substitute a teaching
simulation or an ambient network API.

HTTP servers, HTTP clients, DNS, and distributed cords may consume these
boundaries through their own typed semantic contracts. They must not duplicate
or erase the raw transport semantics.
