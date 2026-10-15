# Global assignment hard admission

For #5553, the existing global planner now has an explicit test matrix over
current Host affinity, offered Front compatibility, Boot/generation observations,
queue and resource ceilings, declared memory and typed compute reservation,
authority allowlists and missing exact grants. Connected ordinary Plan sealing
refuses stale source/sink Boots, unsupported transport, unavailable or mismatched
Lines and invalid payload/in-flight limits. Missing modeled costs remain unknown.

No production scheduling or ranking policy changed. Successful plans retain exact
current Host/Boot/generation, capability, implementation/artifact and port facts
and pass ordinary Plan verification. Single-Gear and connected searches retain
16/128 state ceilings. The existing greedy-trap, capacity pruning and bounded
search tests remain part of the full suite. The three existing connected transfer
test bodies are byte-identical after sharing their fixture.

At source `6f707913fe4e4a5f2d9afd8941efbd9e594c143b`, all 327 planner tests across 77 targets passed,
with one existing ignored manual timing measurement. Strict all-target Clippy and
formatting passed. Exact log hashes and bounds are retained in
[evidence/global-hard-admission.json](evidence/global-hard-admission.json).

This is checked Source and production-planner proof with explicitly modeled
resource/Line offers. It is not live two-Host inference, physical transport or
execution performance evidence. Parent #5230 retains its broader acceptance.
