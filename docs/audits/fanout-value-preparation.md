# Reclaim refused, unpublished fan-out values

Development slice for #5231's fixed-storage and exact ownership requirements.
Quiescence, normal-close, and abnormal-terminal publication each stored a fresh
value and retained one reference per target. A later retain refusal returned
without releasing the original value or earlier retains. Remote ingress already
rolled back its retains, but did not preflight the u16 reference-count ceiling.

One private storage helper now prepares the complete new-value fan-out before
publication. It rejects zero references as an invalid budget and counts above
65,535 as reference overflow before storing anything. If retain refuses after
storage, it releases the original reference and every successful retain, then
returns the storage error. Quiescence, normal close, abnormal terminals, and
remote ingress use that helper. Existing queue/Sign admission and publication
order remain at the callers. No runtime state or allocation is added.

Three native internal scheduler regressions inject a reference limit of two into
a wrapper around the fixed value store, then attempt three-target publication.
All three reproduced one leaked allocated item before repair. After repair,
refusal leaves zero allocated items/bytes, unchanged Sign count, armed flags,
readiness, queue occupancy, and producer-closed flags. Removing the injected
limit allows retry with three exact references, followed by complete reclamation.
This is deliberate failure injection, not a claim that ordinary fixed storage
refuses at two references. Boundary tests independently exercise the real fixed
store at the u16 ceiling, refuse an oversized count without disturbing an
existing value, and retain/release exactly 65,535 references successfully.

The helper relies on ordinary storage error semantics: a refused retain has not
acquired a reference, and releasing each valid owned reference succeeds. It does
not make arbitrary faulty storage or Sign implementations transactional, nor
does it roll back mutations preceding these internal publication methods in a
larger Step. These tests prove the bounded new-value preparation boundary; they
do not establish whole-Play performance, physical proof, or parent acceptance.

Local validation: 16 focused fixed-only fan-out tests passed; alloc and
linear-ready-reference kernel suites each passed 195 tests. Formatting, strict
alloc all-target kernel Clippy, and diff whitespace checks passed. No CI was
dispatched or awaited.
