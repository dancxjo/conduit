# Required-item wire integration repair — 22 October 2026

Refs #6243, #5628 and #5994. The installed Patchbay cutover accidentally copied the future BrowserComponentRecipe::RequiredItem variant from the retained branch, before the matching Face recipe and product CLI conversions were delivered. Current dev's three CLI match sites only handle Flat/Grouped, so the premature public enum variant makes those matches non-exhaustive.

Remove only that variant and its serde rename. Current dev continues to advertise exactly its supported recipes. The complete required-item/Look v2 delta remains preserved on origin/codex/native-local-progress for coordinated delivery; no fallback reader or fabricated implementation is introduced.

Focused product CLI compilation (`conduit` package, locked/offline) passes after the repair; the retained log is beside this audit. This is a compilation claim, not full-workspace/CI/stable acceptance. The original problem was identified by direct enum/match source inspection; no original failing compilation was run or claimed. Whitespace checks pass. CI deliberately skipped tonight.
