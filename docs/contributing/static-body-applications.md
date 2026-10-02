# Static Body applications

This is the development implementation for [#4804](https://github.com/dancxjo/conduit/issues/4804). It is not an accepted-release claim. The browser acceptance suite is being validated; the checklist below describes its required proof, not completed results.

A static application supplies reviewed machinery and starting material. Opening it creates or recovers a Body belonging to that browser installation. It does not join a pre-existing global Body, and it needs no application server, account, relay, or cloud database.

## Build the Handbook

From the repository root:

```sh
cargo xtask make body static --help
cargo xtask make body static \
  --application targets/browser/handbook/handbook.application.template.json \
  --handbook --output target/handbook-static
```

Choose a new output directory; the command refuses an existing destination. Without `--release`, it builds a reviewed browser release. `--release PATH` reuses an existing reviewed release directory. The `--handbook` option renders the existing `wiki/` prose and attaches the application to chapter pages.

Copy the complete output directory to an ordinary static HTTP host. Use HTTPS for deployment or a trusted loopback origin for local development so the browser can provide durable storage and cooperative Web Locks. Do not open it as a `file:` URL. The output includes the application manifest, resources, SDK, reviewed BrowserBundle, runtime, profile and IMAGE identities, and initial Plot sources. No runtime Body, Boot, Plan, or Play identity is preassigned to visitors.

## Make another application

Use `conduit.browser/application-package-template@1`, the existing application package contract. The template declares `application_id`, `state_compatibility`, selected Host implementations, and bounded resources. Each resource has a role, kind, output `path`, and byte bound. An optional `source` selects a repository input relative to the template directory; it is a build input, not a deployed URL. Package generation computes the exact resource and package digests.

The application module exports `startApplication(application)`. It reads admitted content through `application.text(role)`, uses the bounded `application.storage`, and calls `application.browser({ root })` to admit the packaged Browser Host. That entrance supplies the exact application's continuity configuration to the SDK.

[Clock Lab](../../targets/browser/examples/clock-lab/application.template.json) is a second, smaller application:

```sh
cargo xtask make body static \
  --application targets/browser/examples/clock-lab/application.template.json \
  --output target/handbook-static-second
```

Clock Lab deliberately uses the Handbook's compatibility label with a different application identity. The application identity and compatibility identity together determine the storage namespace; the version controls compatibility. The exact package digest records provenance without making every compatible package update a new Body. Two applications on the same origin must never recover or overwrite each other's Body.

## Runtime and browser boundaries

Rust/WASM checks source and owns Body biography, resident workset, membership, planning, execution and authoritative projections. JavaScript coordinates public SDK calls and realizes browser effects. HTML, DOM and SVG are appropriate browser Mask resources; they do not determine Body identity or reconstruct semantic truth.

`Conduit.browser()` admits Host and Boot without a DOM root and paints no diagnostic panel. Current Body execution requires an application-owned connected surface, supplied for its selected browser effects. Host admission alone is not resource acquisition or permission to start a Play.

The Handbook source editor obtains live syntax tokens from the packaged Rust/WASM projection, including Unicode and incomplete edits. Highlighting is not successful checking: **Try in my Handbook** checks the exact edited source and reports a refusal if it cannot be admitted. It must not replace the example with a JavaScript simulation.

**Open in Patchbay** selects the resident Patchbay Plot. Choose the example in its resident list to inspect the authoritative graph. Browser SVG realizes that projection; documentary diagrams are not substitutes for current checked-Plot, Plan, Play, gear, port and cord identities. The presentation boundary remains Face → Mask → Show; the callable signature is a fore.

## Static document content policy

The producer inserts a Content Security Policy meta element at the beginning
of every emitted application and chapter document's head, before executable
resources and before application manifest digests are computed. Reviewed
SDK/release payloads are copied without rewriting their embedded documents.
An authored stricter policy is retained alongside the generated policy.

Scripts may load from the same origin or admitted blob modules, and
`wasm-unsafe-eval` permits the packaged WebAssembly runtime. Inline JavaScript
and JavaScript `eval` are not permitted. Connections are limited to the same
origin. Styles permit the existing inline rules and local/blob stylesheets;
images permit local, data and blob resources; fonts permit local/data resources.
Objects, frames, workers, form submissions and document base overrides are
blocked. Inline SVG documentation remains ordinary document markup.

This policy complements exact package admission and browser network acceptance.
It does not establish hostile-code confinement or replace the proof that the
application needs no backend: same-origin static fetches remain allowed, and
WebRTC attempts are separately denied by acceptance instrumentation.

## Continuity, ownership and reset

The first visit births a Lulled Body, retains it, and explicitly wakes it. Reload or chapter navigation admits a fresh Boot and recovers the same Body. Resuming execution admits fresh Plan/Play truth; a serialized Play is never restored as live execution. Missing state permits a new birth. Corrupt or incompatible retained state must produce a refusal, not a silent replacement.

A cooperative application-scoped Web Lock is acquired before reading or creating Body continuity. A second tab receives an ownership refusal. **Release this tab** settles execution and persistence before releasing the lock; **Try again** can then recover the retained Body. This is cooperative browser ownership, not hostile-code confinement. A lock is not released merely on `pagehide`, because the same document can return from the back-forward cache.

**Start my Handbook over** closes the Body and clears only this application's retained state. It preserves the durable Browser Host identity. Reopening births a different Body. Browser storage can be evicted; the UI reports durability status without claiming guaranteed persistence. An ephemeral profile must make no durability claim.

## Required acceptance evidence

The sequential Chromium suite is [static-body-application.spec.mjs](../../proof/browser/static-body-application.spec.mjs), using real packaged files and zero retries. Required evidence includes:

- First local birth, truthful Lulled starting state, and explicit wake.
- Same Body through chapter navigation, reload and persistent-profile restart; fresh Boot and execution identities.
- A different browser profile receiving different Host and Body identities.
- Two applications on one origin retaining independent Bodies.
- Competing-tab exclusion and recovery after explicit release.
- Reset producing a new Body while preserving Host identity and the other application.
- Real edited-source checking, live native highlighting, and resident Patchbay identities correlated with the running Body.
- No external executable fetch, WebSocket, WebRTC, or hidden application backend dependency.

Node contract tests and browser acceptance prove different boundaries. Passing package checks alone does not establish durable browser behavior, public deployment, or stable-release acceptance.
