# Later typography intent compatibility seam

Read-only review while stage08 font mechanism is active. No implementation.

Current closed laws: VisualExpression deny_unknown_fields/version1 (visual_expression.rs); VisualExpressionInfo.version1 in types.conduit; outer LookInfo.version1 and visual identity equals Look identity in look.conduit; component Fore conduit.face/component-mask@1 embeds the exact structured expression type (component_recipe.rs).

Later #5628 may add finite Engineering/Editorial typography intent through the existing canonical expression. This requires explicit expression wire/type versioning, component Fore revision and outer Look wire review. Preserve v1 golden fixtures as historical or unsupported cases; add current-version fixtures and browser pins. Older providers must refuse typed eligibility or report explicit canonical fallback, never ignore the axis. No parallel Look reader/runtime.

Host-owned font mapping must resolve a finite supported typography intent to one admitted role/profile mapping shared by measure and paint. Four current Rust files construct literal VisualExpression values; also check canonical LookInfo constructors. Main surfaces: face types.conduit/look.conduit, visual_expression{,_info}.rs, look{,_selection}.rs, component_recipe.rs, three authored component files, native/browser offers/resolvers, native typography+scene threading, browser binding and Look golden fixtures. Existing #5628/#5992 scope permits appearance-only extension if semantics/authority remain unchanged.
