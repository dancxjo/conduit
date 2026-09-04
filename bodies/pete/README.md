# Pete application

Pete is one concrete Body/application built with Conduit. This crate composes
ordinary Conduit robotics, sound, presentation, and indicator meaning for
Pete's iRobot Create hardware; it is not generic architectural substrate.
Create Open Interface is a bounded device
protocol over an exact UART Base; it is not a Conduit Line and it does not own
planning, scheduling, or authority.

Ordinary application composition lives directly under `src/`. Historical
deterministic capstone specimens are fenced under `src/proof/`, compiled only
for this package's tests, and are not re-exported as Pete's reusable API.

Repository proofs enter through `cargo xtask pete`. The std Host supports
live observation plus bounded speaker, indicator, and reduced-safety drive
verticals. Each entrance requires exact Host, Boot, Base, and robot identities
and retains machine evidence without promoting it into Pico W or human proof.

The historical Netherwick project's Brainstem revision and its responsibility migration ledger
remain recorded in `docs/architecture/pete-brainstem-migration.md`. The old
describe-only fixture runtime and product-facing demo are intentionally absent.

Pico W realization and physical safety HIL remain separate proof classes.
