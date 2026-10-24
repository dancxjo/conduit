# Native icons follow admitted text size

Source `dfeb14b59be128560593fd74fd8fb032580c81dd` scales existing semantic
Lucide icons from 16px to 20px at 125% and 32px at 200%, with a proportional
half-icon gap. Flat/Details, panel and mechanical layout all reserve the same
space before wrapping, intrinsic sizing and field-cluster admission. The prepared
row retains that size for drawing. Icon identities, assets, labels, actions and
command cost are unchanged; unsupported semantic mappings still retain text.

## Local checks

The focused regression fails on the old source (16px instead of 20px at 125%).
After the fix, 147 native scene cases pass, one existing opt-in case is ignored,
and the eighteen-replacement Look/draft/current-Show case passes. These host CPU
checks use the conduitos package opt-level=0 iteration override. Logs are retained
per run, including the negative baseline.

Normal product/guest builds pass the supported `creche-controls-proof --scenario
accessibility` at 1280×800 and 640×480. Both exit 0 with unchanged Source and a
clean tree. Each actual QEMU run checks the long Unicode draft, complete primary
forward/reverse keyboard cycles/pages in two 200%/forced-color Looks, calibrated
physical pointer argument focus, incomplete-input refusal and seven checked
Look/constraint replacements with fresh local Shows and no fallback. Product
profiles, command/payload/capture limits and pinned icon resources are unchanged.
Host proof-tool glue reuses its existing opt-level=0 build.

Per-width summaries index exact image/build/Source custody, frame budgets,
raw/archive SHA256 and all thirteen original PNG hashes/dimensions. Full receipts
and logs are lossless gzip archives. The older small-icon captures remain in
`../gui-look-6349-native-look-guidance/` as the before state.

## Boundaries

This improves ordinary semantic row icons; separate graph and checkbox drawing
are not changed. CPU Details/continuation cases are not a full QEMU Details
journey. Complete unavailable/resource-fallback coverage, full Body/Patchbay/Stop,
physical/human evidence and integration/publication remain open. No CI campaign
or issue closure is claimed.
