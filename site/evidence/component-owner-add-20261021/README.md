# Selected component owner Add: incomplete live proof

This packet preserves two different source-bound attempts for #5645. It is
negative/partial evidence, not a completed Web Component or Look delivery.
The later evidence commit does not relabel either captured product Source.

The supported entrance now exposes the existing browser scenario:

```text
cargo +stable xtask prove todo-journey --look-replacement \
  --state-dir <fresh-private-installed-state> --conduit-bin <installed-owner> \
  --fresh-body-source plots/todo/checkpoint-once.conduit \
  --handbook-package <matching-sealed-handbook> \
  --pinned-playwright <repository-pinned-playwright-index.mjs> \
  --output <new-evidence-directory>
```

Each attempt used one fresh installation, matching sealed owner/runtime/UI
Source, pinned headless Chromium and no retry. Original raw captures and private
state remain retained locally. Public raw JSON and PNG copies are byte-exact;
the manifest records any stderr whitespace normalization with its original hash.
Installation state, full page HTML, transport credentials and binaries are omitted.

## Initial attempt

Source `e362a6b07947f8befc6de1e70e97dc0d1e2c2cd8` stopped after 8.501 seconds.
It captured two actual selected component Looks, then an obsolete collector
assertion required the later WASM `-5` stale-Show error. The current SDK had
correctly refused the retired Show earlier. No Add was attempted at this stage.

The subsequent collector change requires the exact typed `StaleShow` result and
labels its layer `sdk-before-wasm`. It retains full owner-Face and replacement
Show equality checks; it neither bypasses SDK admission nor claims WASM proof.

## Current attempt

Source `ab32b37aaaa0e70805630f6d4f911828e4e1face` stopped after 19.611 seconds.
Its `current/look-observations.json` and `look-replacement-evidence.json` retain:

- actual selected `implementation/browser-component-mask`, checked component,
  recipe, Fore and Mask Plot/Plan/Play identities;
- compact/flat, spacious/flat and spacious/grouped acknowledged occurrences,
  unchanged canonical owner meaning and computed geometry;
- typed retired-Show refusal at the SDK, unchanged owner Face and replacement
  Show, and the actual 200% text-size occurrence.

The ordinary trusted Add path then returned from activation but did not observe
the expected new Face/revision/item within the existing 12-second wait. There
were no page errors. The retained failed view has zero Items and
`interactions_admitted: false`; preceding Look and text-size views had it true.
These observations do not establish the cause, mutation count, or final outcome
of the attempted Add. The action was not repeated.

The current Linux/browser seals and Handbook manifest retain Source identity and
artifact digests. The matching products came from the supported cached build
entrances. The CLI selection tests passed 3/3; the typed-refusal change passed
38 SDK and 30 Look-evidence helper tests plus syntax/diff checks. These focused
checks do not replace the failed live action proof.

## Remaining gate

Diagnose and repair this selected-component action path without fabricating
owner state, suppressing the failure, extending timeouts without evidence,
repeating an uncertain action, bypassing current Show admission, or substituting
the successful ordinary generic route from #5630. A new clean matching-source
run must retain the actual committed item, checked selected implementation,
same-Face accessible generic reading, and source-owned action/refusal receipts.
No PR admission, issue closure, native/physical proof or publication is claimed.
