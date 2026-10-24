# Structured tape conditional join repair

At unmodified production Source ebf989413, a new ordinary/prepared parity control fails: true branch output omits the following right=33 record field. The four existing tape tests pass. Exact negative Source85f0a4 and rawfailedlog/actualELF are retained.

Repair6adc488 prevents constant-byte coalescing across an existing jump destination. No runtime instruction or admission/bounds rule is added. Both branch values and repeated reuse match ordinary canonical bytes with zero allocations measured. Fifteen actual tests across four owning targets pass; strict Plot library/four-target Clippy passes. This does not claim the author's separate27-case campaign or fullCI acceptance.

Clean source/tree, fullargv/exits/logs, allpositiveELFhashes,4actualpositiveRunningELFs andnegativeELF retained. Recoverybranch andevidence only; existingPR owner remains unchanged. Fresh Candidate and applicable accepted Integration remain required after adoption. No MCU installation, physical output, authority or whole-pathno-growth claim.
