# Bounded literal storage in prepared structured expressions

Prepared structured children now retain Boolean/octet literal bytes through the
existing literal codec. Dynamic evaluators are boxed instead of reserving the
full evaluator union in every child slot. Nominal Types, predicates, arithmetic,
projections and conditional branches keep their ordinary evaluation path. Codec
refusal falls back without changing the original refusal phase.

This is a bounded storage improvement in the shared evaluator, not a compact
structured instruction tape. Recursive dynamic evaluators, structured field
names/type prefixes and intermediate outputs still exist. It introduces no
TM1637-specific packet synthesis, semantic-type change or hardware authority.

The 64-bit CPU model rounds live allocations to 32-byte units. A literal-padded
record falls from 8,064 to 2,592 retained bytes; the actual authored TM1637
packet expression falls from 26,912 to 18,688. Baseline and final logs are
retained separately; regression ceilings were added after the baseline run.
These are evaluator storage measurements, not MCU or whole-kernel bounds.

All 23 focused evaluator tests pass, including 256 input-byte reference cases,
512 packet cases spanning every brightness byte and both enabled states, lazy
failure followed by successful reuse, immutable expressions and scratch/payload
bounds. Recurring evaluation allocates nothing. Strict scoped library/test
Clippy passes. The display-output public proof includes the new test target.
No new MCU heap capture or firmware flash is claimed for this final change.
