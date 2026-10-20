# Bounded reluctant-replanning policy

**Partial #5568 implementation. The issue remains open.** The planner now has
a finite local gate over existing fresh replacement costs. Its decision is a
recommendation, not authority to install or execute a Plan. The measured product
observation and authorized replacement route remains to be connected and proved.

## Policy behavior

`ReluctantReplanPolicy` admits 2–16 minimum observations, a positive persistence
span, maximum window and gap, positive cooldown and net-savings threshold, and
1–1,024 forecast operations. It compares equal exact demands on separately
sealed previous/candidate Plans. Every sample must conservatively exceed the
threshold: `(old lower fulfillment - candidate upper fulfillment) * horizon -
upper sequential replacement overhead`. The overhead includes preparation,
transfer, residency, migration and continuity. These are finite estimated cost
coordinates, not a certified deadline or execution guarantee.

Every evaluation revalidates all five replacement stages and the actual live
provider residences, forecast calibration/freshness/current execution assumptions,
selected placement and unchanged checked demand. Observations use the same local
Host/Boot/clock basis, paired sample instant and strictly ordered distinct Signs.
Old windows, gaps and insufficient persistence cannot recommend a replacement.
A recommendation latches its previous Plan; even after cooldown it cannot be
repeated for that same old Plan by this owner. A newly accepted previous Plan
must also respect cooldown. Refused arithmetic or evidence leaves both latches
unchanged. New owner construction does not inherit historical continuity.

The decision retains the policy, previous recommendation, exact Plan identities,
all paired source attributions, conservative net values, five replacement sources,
upper overhead and arithmetic work. The caller owns returned historical records.
This gate retains two latches rather than a growing decision history, performs no
kernel work, mutates no Plan, grants no authority and replays no external effect.
Its context is deliberately local; the prerequisite replacement-cost profile
refuses unsupported cross-Host/Boot transfers.

## Current evidence and limits

Six fixed-clock tests cover persistent benefit and its exact conservative value,
noise, short bursts, gaps, duplicate/reordered/stale observations, exhausted
numeric work, finite capacity, unchanged Plans, recommendation latching and
cooldown across an accepted successor. The fixtures use real separately reserved
and published local providers and ordinary sealed Plans. Forecast numbers and
clock ticks are explicitly declared fixtures, not measured speedups or independent
physical Sign authentication. A returned recommendation alone does not establish
that the product authorized or ran a replacement.

The supported `cargo xtask prove resource-frame` entrance includes those six and
five existing replacement-cost tests. At integrated source
`d5525ef5ed5c7dbfa147aae08cede4c19e32a577`, its 39 actual tests pass, including
existing production-kernel frame execution, publication/allocation, planning,
resource contracts and one resource projection test. Zero-test filtered binaries
are not counted as evidence. Strict scoped Clippy (including the timing example)
and focused formatting pass with a clean unchanged worktree. Implementation
source is `855fdc9e2004cd03d07a59a6a3940cf74a942692`; the integration also contains
accepted dev `5bd95bf04cb6f6347339f4e7d3c69489b6eaa02f`.

[Validation receipt and hashed raw logs](evidence/reluctant-replanning-policy-20261020/validation.json)
retain exact commands, source and counts. This is local conformance evidence,
not Candidate/Integration/main acceptance or a published release.

## Remaining original acceptance

Connect a measured product workload's fresh producer observations and the five
measured replacement costs to this gate. Preserve inspectable Sign/Plan/Host/Boot
and decision custody through the normal owner. Show a persistent large net
benefit enters ordinary fresh authorized replacement admission, while noisy load
does not repeatedly request replacement. Retain prior outcomes, execute no old
effects again, and independently verify the complete decisive and no-replan
records. Existing #6101 measurements and #6102 lifecycle proof are prerequisites,
not substitutes for that missing connection. #5568 and parent #6078 remain open.
