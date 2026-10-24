# Todo action icon coverage

The shared pinned Lucide vocabulary now includes `plus` for exact `todo/add@1`
and `trash-2` for exact `todo/remove@1`. Existing `todo/complete@1` retains `check`.
The two keys are appended, preserving all previous wire tokens and native indices.
All 28 previous native silhouettes remain byte-identical. The 30-icon native
pack is 138240 bytes; existing scene and input limits are unchanged.

`renderer-preview.png` is a local browser renderer fixture, not a Body execution
receipt or a native screenshot. It shows the actual shared SVG decoration and
stylesheet. The browser test checks accessible names, exact keyboard invocation
payloads, and text-only fallback for an unknown custom intent.

`capture.mjs` preserves the local fixture collector; its paths assume this chat's worktree and scratch layout, rather than defining a new supported entrance.

The upstream SVGs are copied unchanged from Lucide commit
`b7b6ecf1316d0af64c97a6b0392abe5e816a8e30`; existing license notices apply.
The mapping generator and native raster generator both reproduce exactly.

Local checks: four shared vocabulary tests, 107 Chromium owner-component tests,
and three Face composition tests pass. Three native semantic-icon tests also pass, including bounded page layout. No CI, current-source QEMU, release publication or issue closure
is claimed by this evidence.
