# #5628 version boundary

Read-only at `ffcdb1a8ea7caaf9fbdaa4e077c8b04b620dd2b7`.

There is **no numeric equality rule** forcing `LookInfo.version == VisualExpressionInfo.version`. `look.conduit:57–77` embeds the concrete `VisualExpressionInfo` record through `LookVisualFacetInfo`; its only cross-field constraint equates identities. However, this is not an independently opaque/version-dispatched facet: changing `types.conduit:667–674` changes the outer Look’s transitive semantic/wire type. Keeping the outer numeric field at1 would not preserve its existing schema. `tests/look_browser_binding.rs:8–41` compares the full canonical `LookInfo::semantic_type()` bytes and combined Source digest with the browser binding; both must change.

`visual_expression.rs:32–54` rejects unknown fields and versions. `visual_expression_info.rs` constructs/decodes that exact checked record; `look_selection.rs:48–64` passes the embedded record through that decoder, not a multi-version dispatch. `component_recipe.rs:210–225` derives startup parameter kinds from the expression profile while advertising `conduit.face/component-mask@1`. Changing the required startup record changes that Fore’s admitted contract even though no code automatically increments its revision.

Smallest honest hard cut remains expression2 + refreshed outer Look schema/binding/golden + component Fore2. Bumping outer Look’s numeric version to2 clearly names the already-breaking aggregate schema; it is justified contract labeling, not a mechanically mandated version cascade. Avoid claiming unchanged outer-schema compatibility. An independently versioned facet would require an explicit retained variant/decoder design that the current concrete embedding does not provide.

Persisted-v1 evidence found: checked-in `phosphor-look-v1.hex` and exact-byte regression (`look_browser_binding.rs:43–54`). These establish historical wire identity, **not a requirement to keep accepting persisted v1 resources**. No compatibility mandate found in the inspected contracts.
