# Conduit

Conduit is being rebuilt around one rule:

> Forms describe meaning. Hosts offer implementations. Plans make realization exact.

Current `main` contains a useful Rust `std` prototype, deterministic
browser-shaped/Pico-shaped conformance fixtures, and one actual browser-local
Rust/WASM Signal host. Two independent Chromium-page instances parse and plan
the unchanged Signal form, lower exact local fragments, and execute through
`conduit-kernel` before driving real timers and bounded DOM presentation. It
does **not** yet contain a general-purpose std execution engine, Pico firmware,
live WebSocket/UDP transport, three-platform proof, production BODY model, or
production Observatory.
[STATUS.md](STATUS.md) is the checked claim boundary.

## Project guide

Conduit separates durable direction from current proof and immediate sequencing:

- [The Conduit canon](docs/conduit-canon.md) preserves the vision, vocabulary,
  architectural invariants, future layers, and the status of dormant or
  superseded ideas.
- [AGENTS.md](AGENTS.md) is the working agreement for contributors and coding
  agents, including scope ownership, parallel-work rules, proof discipline, and
  PR expectations.
- [STATUS.md](STATUS.md) states only what current code and named checks prove.
- [Issue #361](https://github.com/dancxjo/conduit/issues/361) owns the forward
  salvage order.

The canon is a seed vault, not a demand to implement every good idea now. The
archive and reboot remain quarries: concepts return only through focused,
reviewed vertical slices with executable acceptance.

## Salvage sequence

The reboot exposed valuable identity, planning, pressure, evidence, and wire
sketches, but its broadcast operation protocol cannot express general typed
graphs. The forward salvage roadmap is tracked in
[#361](https://github.com/dancxjo/conduit/issues/361):

1. port-aware bounded kernel;
2. exact semantic/resource/authority/link planning;
3. a small lossless form language and explicit composite faces;
4. actual std/browser/Pico hosts and observed bounded links;
5. a genuinely executable small standard catalog;
6. BODY/PART/GEAR/ROLE/CAST/LINK/SOUL;
7. Observatory over real reports, then useful tasks through host operations.

The archived pre-reboot tree and the reboot are both source quarries. Focused
reuse is recorded in [docs/reuse-ledger.md](docs/reuse-ledger.md).

Kernel takeover [#389](https://github.com/dancxjo/conduit/issues/389) is
accepted. Exact local `PlanFragment`s for the installed profiles lower into
bounded numeric kernel tables and run through the hosted scheduler. Unsupported
std forms fail closed; production `StdHost` has no fallback operation pump.
Nested expansion identity [#398](https://github.com/dancxjo/conduit/issues/398)
and general named composite faces
[#399](https://github.com/dancxjo/conduit/issues/399) are accepted. The
browser-local kernel checkpoint under
[#350](https://github.com/dancxjo/conduit/issues/350) is accepted at main
`b7852eed1e784a27dcd78e700b2f89ddc01bc097`, workflow `31022565054`.
Cross-host simulation fixtures retain an explicitly named legacy compatibility
driver; they are not transport evidence.

## Current executable prototype

The Rust `std` path can:

- parse the small reboot `form 0` grammar;
- validate explicit placements and exact reboot plan fragments;
- execute finite `flow/pulse -> presentation/show` demonstrations;
- stream bounded stdout receipts;
- exercise deterministic item/byte pressure and cancellation fixtures.

Run the local std demonstrations with:

```bash
just demo-std
just demo-triple-local
```

The browser-shaped and Pico-shaped crates live under `fixtures/`. Their
frame/datagram relays are in-memory deterministic fixtures, not sockets.
The actual browser checkpoint lives under `hosts/browser-runtime` and
`hosts/browser/`. Rust owns parsing, planning, exact lowering, kernel execution,
bounds, lifecycle, and terminal truth; JavaScript is the thin real-timer/DOM
adapter. This is browser-local execution, not a std-to-browser socket.

## Forward kernel

`conduit-kernel` is the new `no_std`, port-aware execution contract. Its
S1 slice provides exact input/output port identity, correlated generic
host-operation actions, prebound numeric route/admission tables,
item/byte-bounded fixed and preallocated hosted storage, and the accepted
fixed-capacity scheduler. Installed local std profiles now use the fail-closed
numeric lowering seam for real timer/stdout execution, reversible identity
projection, exact resource reservation, and measured allocation-free
activation. The production browser Signal host uses the same scheduler and
shared lowering boundary, with sealed capacity stability rather than an
overstated browser allocation measurement. `HostRuntime` remains only in
explicitly named simulation/composite compatibility paths, not production
`StdHost` or the production browser host.

Architecture and current salvage boundaries:

- [Project canon](docs/conduit-canon.md)
- [Contributor and agent agreement](AGENTS.md)
- [Salvage status](STATUS.md)
- [Portable host architecture quarry](docs/architecture/portable-hosts.md)
- [Host specification quarry](docs/architecture/host-specification.md)
- [S1 kernel notes](docs/architecture/salvage-kernel-s1.md)
- [Kernel takeover gate](docs/architecture/kernel-takeover.md)
