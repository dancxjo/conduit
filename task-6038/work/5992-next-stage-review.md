# Next native typography stage: bounded review

Read-only preparation while #6162 runs. Do not start implementation until stage05/06 gates finish.

Mechanism: pinned Serif for Title24, Sans interface, existing Mono for Details clauses whose actual Property(index) provenance resolves to an Identity value. No guessed text classification. Validate generated profile ascent/descent/line height against selected and Sans-fallback ink plus rescue bearing/height; test 100/125/200 percent rounding and combining marks. Layout and raster must share exact metrics. Preserve existing budgets and headless exclusion.

Later #5628 integration: engineering/editorial is a finite typography intent in the existing VisualExpressionInfo, with Host-owned pinned ensemble resolution. Current v1 is closed (types.conduit version constraint and Rust deny_unknown_fields); review an explicit Fore/wire version transition, not serde-default optional expansion. Preserve historical fixtures and honest unsupported-version/provider refusal or fallback. Thread one admitted role/profile mapping through measurement and paint. Do not infer typography from palette/Look identity or create a separate Look engine. Existing #5992/#5628 scope permits this extension if Face semantics/authority remain unchanged.

Likely later owned seams: semantics/face types.conduit/look.conduit and visual_expression modules, authored mask roots and native/browser offers, native typography/layout/raster, Look fixtures and browser pins. This document proves no implementation or runtime behavior.
