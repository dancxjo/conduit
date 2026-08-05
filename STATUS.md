# Conduit salvage status

This file is the repository-level claim boundary. A green check proves only the
rows and commands named here; it does not promote a simulation into a platform
adapter or physical proof.

| Surface | Contract | Simulation | Executable hosted implementation | Actual browser adapter | Actual firmware | Live transport | Physical/HIL proof |
|---|---:|---:|---:|---:|---:|---:|---:|
| Exact plan and fragment identities | prototype | mutation-negative fixtures | yes, std runtime | no | no | no | no |
| Connection envelope wire format | allocating prototype | deterministic vectors | yes, in-memory/frame/datagram fixtures | no | no | no | no |
| Portable Signal | yes | multi-value fixtures | yes, std stdout/timer | no | no | no | no |
| Browser-shaped manifestation | partial | yes, `conduit-browser-sim` | test-only | no DOM adapter | no | no WebSocket | no |
| Pico-shaped manifestation | partial | yes, `conduit-pico-sim` | test-only | no | no BSP/image/driver | no UDP/TCP | no board run |
| Realm membership | retired prototype | deterministic table tests | no production body model | no | no | no | no |
| Observatory | report-schema prototype | synthetic fleet | synthetic command only | no | no | no | no |
| `conduit.std` | prototype contracts | one-value demonstrations | incomplete semantics | no | no | no | no |
| Copy a file | unsafe prototype disabled | tests removed from default tree | no admitted host operation | no chooser | no | no | no |

## Required CI claims

The `check` workflow requires:

- workspace formatting, Clippy, and tests;
- no-std checks for semantic, wire, and std-catalog contracts;
- exact-plan mutation-negative tests;
- deterministic wire and simulated-host conformance vectors;
- WASM compilation of the browser-shaped simulation;
- Thumb compilation of allocator-free contracts and the Pico-shaped simulation.

WASM compilation is not browser execution. Thumb compilation is not firmware
or board acceptance. Frame/datagram fixtures are not WebSocket or UDP sockets.

## Salvage stop line

S0 restores truth. S1 replaces the broadcast operation protocol with a
port-aware bounded kernel. No actual browser/Pico host, BODY, catalog expansion,
Observatory acceptance, or useful task advances before its prerequisite gate in
issues #349 and #361 is accepted.
