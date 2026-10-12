# Domain-derived renderer inputs in development

`open-domain-face.json` is the exact Face projected by the ordinary contribution
fixture in `semantics/presentation/tests/face/open_domain_semantics.rs`. That
source test checks complete equality against the serialized specimen, including
Body, Face, context, source, Plan/Play basis, actions, disclosures, and open IDs.

This differs from the neighboring reconstruction accounts: it is an input to
renderer execution tests, not a claim that any native/browser Show or audible
speech has occurred. Its Body and execution setup are explicitly test fixtures.
Do not interpret a stored specimen as an installed owner or independent human
observation. Renderer tests must retain their own Show and realization evidence.
