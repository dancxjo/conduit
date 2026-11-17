# Exact owner two-facet test diagnosis

Primary read-only trace of owner-two-facet-proof.log, no competing Source mutation/build:

Failure at runtime_face/tests.rs393 is `InvalidPlotIdentity("every external Fore binding requires one exact queue limit")`.

Test line385 calls `source.expand_entry_for_authoring()` on `plots/ambient-home/aspects.conduit`. The public loader's `CanonicalSource::authored_entry_name` (plot_source.rs78) intentionally chooses syntax.plots.last(). That file's last declared Plot is **ambient-aspects**, the caller-supplied open Fore helper. The intended clock-driven closed parent is **ambient-observed-aspects**, declared first.

Use existing `source.expand_named_for_authoring("ambient-observed-aspects")` for this explicit owner execution test. Keep the original empty external boundaries for the closed Plot. Do NOT weaken the one-exact-budget check or invent an external producer/budget to make the wrong entry run. Assert expanded.name in the fixture so this remains obvious. This is a focused diagnosis, not a passing test claim.

Related deployment caution: resident.conduit's closed parent ambient-resident is also declared before its helper Plots. Actual Source selection must explicitly name ambient-resident. An ordinary default entry would select the final open helper. Preserve the authored explicit entry in the deployment/checking request; do not use default expansion and assume it names the closed instrument.

Root disk cleanup: removed735 inactive primary-owned frozenRoot release rlibs,4671610718 payload bytes, after actual compiler-command/FD consumer check. Executables/generated inputs/logs/proofs/source/credentials preserved. Root free1.4→5.8GiB; no foreign cache or active Ambient/Queue artifact touched.
