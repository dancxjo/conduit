# Compact Crèche rendezvous assertion evidence

Owning issue: #6271. This proof-only repair selects the existing authoritative
membership revision, admitted Part and matching Host offer identity/count inside
the page. It avoids transferring the complete canonical Host inventory through
Playwright. Every semantic assertion and the 20-second deadline remain intact.

The retained failing Integration 39901645153 trace contains a 4.62 MB final
evaluation result taking about 2.25 seconds. All admission assertions and the
rendezvous process success completed before the total-budget failure. This does
not diagnose the independent Host startup or Birth costs.

Local diagnostic: pinned Chromium, one worker, zero retries, one test passed
in 8.8 seconds. The command used the ordinary checked-in Playwright configuration
and `creche-rendezvous.spec.mjs`, with the existing `workspace-product` and native
CLI from the pagination-integration target. These products were not rebuilt from
this checkout: this is diagnostic proof, not fresh exact-source product acceptance.
Node syntax and whitespace checks pass. Combined browser Integration remains due.

The internal focused invocation was:

```sh
CONDUIT_CRECHE_RENDEZVOUS_PRODUCT=target/workspace-product \
CONDUIT_BROWSER_PROOF_SHARD=rendezvous-repair \
CONDUIT_BROWSER_HOST_PORT=4189 \
node proof/browser/node_modules/@playwright/test/cli.js test \
  --config proof/browser/playwright.config.mjs --project chromium \
  --workers 1 --retries 0 creche-rendezvous.spec.mjs
```

The supported full-product entrance remains `cargo +stable xtask prove browser-host`.
No full proof campaign was duplicated for this diagnostic.

Local log SHA256: `a3c512dc3b103b7d01565ef28a874e3a20e6572db17f79140f6d674da1985008`.

```text
[WebServer] (node:2230764) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
[WebServer] (Use `node --trace-warnings ...` to show where the warning was created)


Running 1 test using 1 worker

(node:2230775) Warning: The 'NO_COLOR' env is ignored due to the 'FORCE_COLOR' env being set.
(Use `node --trace-warnings ...` to show where the warning was created)


[1/1] [chromium] › proof/browser/creche-rendezvous.spec.mjs:19:1 › a rendezvous code admits one already-running raw Host through the web Crèche
  1 passed (8.8s)
```
