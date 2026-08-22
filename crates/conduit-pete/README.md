# Pete Conduit realizations

This crate realizes ordinary Conduit robotics, sound, and indicator meaning on
Pete's iRobot Create hardware. Create Open Interface is a bounded device
protocol over an exact UART Base; it is not a Conduit Line and it does not own
planning, scheduling, or authority.

Repository proofs enter through `cargo xtask pete`. The std Host supports
live observation plus bounded speaker, indicator, and reduced-safety drive
verticals. Each entrance requires exact Host, Boot, Base, and robot identities
and retains machine evidence without promoting it into Pico W or human proof.

The historical Netherwick project's Brainstem revision and its responsibility migration ledger
remain recorded in `docs/architecture/pete-brainstem-migration.md`. The old
describe-only fixture runtime and product-facing demo are intentionally absent.

Pico W realization, physical safety HIL, and the byte-identical two-Host
capstone remain open under issue #1521.
