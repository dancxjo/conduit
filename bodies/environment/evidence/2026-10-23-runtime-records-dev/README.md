# Borrowed runtime tables on current dev

Issue #6313, following merged Core validator #6307. The runtime accepts a
canonical header plus borrowed record payloads through the same Core validator
and table builder as contiguous assignments. Current schema eligibility,
layout policy, routing, startup and Host Call checks remain unchanged.

The existing runtime kernel scenarios execute the borrowed-built tables after
the helper's descriptor/payload storage is dropped. The controls compare every
node, cord, route and startup position with the contiguous result, and compare
stale Boot, corrupted digest, re-sealed layout substitution and capacity
refusals. Table preparation and refusal controls allocate nothing.

All 22 owning runtime tests and strict all-target Clippy pass in an isolated
current-dev worktree and private target directory. No firmware, physical
execution, authority, Body installation or complete appliance admission is
claimed. Later retained-software/native installation work is outside this PR.
