# Proof dependency boundary

Fixed proof identities, topologies, and fixtures belong under `proof/`. Ordinary
product, Body, target, semantic, and architecture code must not import them as
runtime truth merely because they make a scenario convenient. Proof harnesses
may depend on the production code they exercise, and `cargo xtask` may depend on
proof packages to provide the repository-development entrance.

The machine-readable [dependency audit](proof-dependency-audit.toml) records all
current non-dev Cargo edges from outside `proof/` into `proof/`. Each edge is
classified by what the consumer currently uses and states its intended
resolution. This is current executable debt, not approval of those edges.

The inventory deliberately excludes `dev-dependencies`: package-local tests are
allowed to consume exact proof fixtures. It includes ordinary dependencies and
build-dependencies because both can make fixed proof facts part of a production
package or artifact. Dependencies between proof packages are also excluded.

The audit test derives the actual edge set from every tracked Cargo manifest and
requires exact agreement with the finite inventory. Removing an edge therefore
requires removing its audit entry in the same change; adding an edge requires an
explicit classification and review rather than silently widening the inversion.

Extraction work should move the smallest truthful reusable contract to its
architecture, semantic, mechanism, fabrication, or target owner. It must not
copy fixed Host, Boot, Line, board, transport, Plan, or scenario identity into a
new production location, and it must not promote simulated advertisements into
claims about physical target availability.
