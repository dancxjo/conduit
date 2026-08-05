# Conduit

Conduit is being rebuilt around one rule:

> Forms describe meaning. Hosts offer implementations. Plans make realization exact.

Current `main` contains a useful Rust `std` prototype and deterministic
browser-shaped/Pico-shaped conformance fixtures. It does **not** yet contain an
actual browser adapter, Pico firmware, live WebSocket/UDP transport,
three-platform proof, production BODY model, or production Observatory.
[STATUS.md](STATUS.md) is the checked claim boundary.

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

## Forward kernel

`conduit-kernel` is the new `no_std`, port-aware execution contract. Its
initial S1 slice provides exact input/output port identity, correlated generic
host-operation actions, prebound numeric route/admission tables, and
item/byte-bounded fixed and preallocated hosted storage. It is not yet the
complete scheduler or an adapter for the reboot semantic kinds.

Architecture and current salvage boundaries:

- [Salvage status](STATUS.md)
- [Portable host architecture quarry](docs/architecture/portable-hosts.md)
- [Host specification quarry](docs/architecture/host-specification.md)
- [S1 kernel notes](docs/architecture/salvage-kernel-s1.md)
