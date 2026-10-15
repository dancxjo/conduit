# Checked pure-chain fusion eligibility

For #5543, reuse the existing planner fusion selector and semantic
transformation derivation. This change extends their conformance proof; it does
not introduce a fusion runtime or claim faster execution.

Checked ordinary Source chains at depths 1, 8 and 32 retain exact checked Plot,
logical Gear and Cord contracts in the resulting Plan. Eight- and 32-Gear cases
select the cheaper eligible fusion observation. The singleton keeps its normal
realization because there is no internal boundary to combine. The 32-Gear case
uses 16 installed clamp and 16 installed scale instances; 32 clamp instances
refuse at the existing maximum of 16. Fixture work observations are explicit
planner inputs, not observed runtime timing.

The refusal corpus covers external effects, retained state, ambient time,
randomness and resources, suspension, variability and replay ineligibility;
Back Host Calls, resources and authority; finite-envelope failures; typed-port,
atomic-pressure, cancellation and evidence-preservation obligations; cross-Host
placement; and required observation and authority boundaries. Existing stale
offer-generation and missing semantic-fact tests remain intact. Their test bodies
are unchanged after extracting the shared fixture.

Required observation uses the existing explicit `FusionBoundary` seam, including
Watch-style independent observation requirements. This proof does not add a
Watch syntax or automatically discover observation attachments. The existing
selected-Plan fixture demonstrates that an observation requirement keeps the
ordinary unfused Cord and logical placements. Actual fused/unfused runtime
pressure, cancellation, failure attribution, performance and target availability
remain the sibling deliveries and parent #5397 acceptance.

The proof class is native checked Source and production planner/Plan lowering.
It is not runtime Play, browser/guest execution or physical-target evidence.
Code source `e8e4ee28aa7481f973cf96f36258af6c4b1e350e` passed 318 tests across
76 targets and strict all-target Clippy. One pre-existing manual timing
test is ignored. The [evidence receipt](evidence/fusion-eligibility.json) retains
commands, source-file and log hashes. Later documentation commits are not the
measured source.
