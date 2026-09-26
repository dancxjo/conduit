# `@conduit/browser`

`@conduit/browser` is a zero-dependency ESM package for ordinary browser
applications. It runs a previously built and reviewed BrowserBundle. A website
does not need Rust, a WebAssembly compiler, npm modules at runtime, or access to
a compiler service.

The package output contains one exact checked Profile and BrowserBundle under
`bundle/`. Installing a different Profile means producing a different package
from that Profile's BrowserBundle. `profileId` is an optional assertion against
that fixed image; it cannot widen the image or turn on code that it did not
select.

```js
import { Conduit } from "@conduit/browser";

const host = await Conduit.browser({ root: document.querySelector("#conduit") });
console.log(host.id, host.bootId, host.profileId, host.offers);
console.log(host.packageVersion, host.runtimeAbi); // diagnostics; these grant no authority
const snapshot = await host.refresh(); // current offers come from a new Boot observation
```

## Source, checked meaning, and Body work

Conduit keeps authorship separate from realization. A Form says what a
computation means; a Host says which implementations it currently offers; a
Plan can later choose exact realizations. Source text is therefore not a graph
of browser callbacks, and checking source does not silently acquire resources,
ask for permissions, or promise that a Plan can run.

The browser SDK sends ordinary Conduitese text to the checked Rust runtime.
JavaScript retains the exact text as a source value; it does not tokenize,
parse, normalize, or infer a second language. Rust returns the canonical source
document identity, each checked Form identity, and the Kind requirements it
derived. These are different facts: changing whitespace can change the source
identity while preserving a checked Form identity, and neither identity is a
Plan or a running Play.

```js
const clock = host.form(`
clock {
  every: time/every(1s)
  tick: presentation/tick
  every >> tick
}.
`);

const checked = await clock.check();
if (!checked.ok) {
  for (const diagnostic of checked.diagnostics) {
    console.error(diagnostic.code, diagnostic.message, diagnostic.span);
  }
} else {
  console.log(checked.sourceDocumentId, checked.forms[0].checkedFormId);
  console.log(checked.requirements.kinds);

  // Rust reviews exact requirements without creating a Plan or acquiring them.
  const review = await host.review(checked.forms);
  console.log(review.requirements.resources, review.requirements.capabilities);
  console.log(review.bodyPlanCreated, review.resourcesAcquired); // false, false

  const body = await host.birth({ name: "Clock", forms: checked.forms });
  await body.install(clock);
  const play = await body.wake();
  console.log(play.id, play.plan.planId, play.state);
  await body.lull();
}
```

`check()` is a semantic boundary, not a promise of execution. A successful
result means the canonical checker accepted the source against its checked
Kind vocabulary. The `kinds` list is useful before planning; `resources` and
`capabilities` are `null` at source-check time because their exact requirements
depend on expanded meaning and a proposed realization. `host.review()` asks
Rust to expand the selected workload against this exact Host's current offers.
It returns per-Host resource and capability requirements but creates no Body
Plan, acquires no resource, and obtains no authority. The SDK does not
substitute an empty list for information the checker has not established. Parser diagnostics
carry the canonical UTF-8 byte range and one-based line and column when the
grammar provides them. Other refusals remain structured with a stable code and
message; the SDK never scrapes prose to recover an error category.

`birth()` requires checked Forms and sends the exact source, source identity,
and checked Form identities to Rust. Rust admits the source interaction,
validates initial membership and the bounded workload, and returns the Body
receipt. The Body begins lulled: birth does not imply a plan, selected
implementation, permission, or active Play. `install()` and `remove()` operate
on that Body's workload. Each operation reads the current authoritative
workload revision and submits it as the expected revision; a stale update is a
typed refusal with the runtime evidence and revision that was used. Form
identity and workload revision are not browser-side mutable state.

## From workload to Play

`wake()` asks the Rust Workspace to propose a Plan for the Body's current
workset against the current Host offers. The proposal is still not execution.
The reviewed Browser Host adapter then acquires exactly the Plan's local
resources and supplies their observations to the Rust start boundary. Only the
runtime's admitted `BodyPlayIdentity` becomes the returned `BrowserPlay`; a
refused proposal or resource admission remains a typed refusal with its source
evidence. Once started, the same adapter dispatches kernel effects through the
existing browser timer, input, presentation, audio, and resource adapters.
Application code never implements a Gear or effect dispatcher.

`play.id` is the runtime's active Play identity, and `play.plan` exposes the
exact Body, Plan, and Wake identities that admitted it. `play.dispatch()` gives
access to the bounded adapter completion receipt. `body.lull()` closes the
current adapter and records the exact terminated Play against current Workspace
truth. A Plan proposal is not exposed as a Play, and a rejected start does not
produce a successful lifecycle receipt. Host, Boot, Body, Plan, Wake, and Play
identities therefore keep their separate meanings even on one page.

The initial SDK profile has a finite, reviewed local adapter set. A capability
that is absent, unselected, unavailable, permission-denied, or lost remains a
distinct Host/runtime outcome; the SDK does not infer readiness from bundled
code or prompt while planning. Distributed execution Lines and permission-
gated acquisition are only usable when the admitted Plan and selected Host
profile provide their exact support.

Forms intentionally contain no DOM selector, browser API, implementation ID,
permission decision, or resource handle. Those belong to later Host and Plan
decisions. This is what lets one checked meaning remain portable without
pretending each Host can realize it.

## Snapshots and evidence events

`body.snapshot()` independently refreshes the exact Workspace snapshot. Its
Body biography records, membership records, workload revision, realization,
and current Host offers remain Rust evidence; the SDK returns a recursively
frozen projection and keeps no second mutable Body store.

`body.events()` reads that same bounded snapshot and projects only retained
Body biography records with their exact sequence and Sign identity. It joins a
wake record to its indexed Rust wake event and returns the raw structured
event as evidence. Host-offer notifications include the observed before/after
offers and identify generation advances that were coalesced between polls.
They are change notifications, not additional Signs. If biography detail was
compacted while a consumer was away, the stream reports the exact compaction
boundary; it does not invent missing history.

Subscriptions poll at a bounded interval (50–5,000 ms), yield one event at a
time, and hold no unbounded event queue. Re-subscribing starts at current
evidence by default. Set `replay: true` to read the finite retained biography
window explicitly. Pass an `AbortSignal` to stop the iterator. An event can
only describe a Rust record already present in the snapshot, so a refused
transition never appears as a success event. Refusals remain attached to the
operation's typed error; lifecycle evidence remains inspectable through the
snapshot independently of subscriptions.

```js
const snapshot = await body.snapshot();
console.log(snapshot.evidence.body_id, snapshot.evidence.body.workload_revision);

const controller = new AbortController();
for await (const event of body.events({ signal: controller.signal })) {
  console.log(event.type, event.identity, event.evidence);
}
```

For a no-bundler static page, serve the complete package directory from the
same origin and import `browser-sdk.mjs` directly:

```html
<main id="conduit"></main>
<script type="module">
  import { Conduit } from "/vendor/conduit-browser/browser-sdk.mjs";
  const host = await Conduit.browser({ root: document.querySelector("#conduit") });
  document.querySelector("#status").textContent = `Host ${host.id} is ready`;
</script>
```

The loader admits the package and BrowserBundle manifests, checks their exact
file lists and digests, verifies the runtime ABI and profile-gated Boot, and
then initializes one real Host/Boot. It refuses cross-origin bundle roots,
redirects, unlisted assets, stale digests, ABI mismatch, and a requested Profile
that differs from the package's image. Module code is imported only from bytes
whose identities were admitted by those manifests. The application's root is
used as a presentation surface; it contributes no Host, Profile, or authority
fact.

An application bundler needs no Conduit compiler. If it does not preserve the
package's adjacent `bundle/` directory, publish that directory as static assets
and pass its same-origin URL with `bundleRoot`.

## Static hosting and CSP

Keep `bundle/` beside the package module when deploying a static site. The
browser fetches only same-origin files declared by the exact package and image
manifests. Do not rewrite asset URLs to an unreviewed CDN. The SDK creates
short-lived `blob:` module URLs only after verifying the corresponding module
digests, and it instantiates the reviewed Wasm bytes only after checking their
exact BrowserBundle binding and ABI.

The deployment's CSP must permit same-origin modules and Wasm compilation, plus
`blob:` module loading for these verified modules. Restrict network connections
to the application's origin for the initial Host entrance. Add other origins
only when a separately admitted Conduit Line requires them. Use HTTPS for
permission-gated browser capabilities; the runtime will report unsupported or
unavailable implementations without widening the selected Profile.

## Producing the package

The package producer takes an exact BrowserBundle directory containing
`browser-page.json`, `browser-bundle-release.json`,
`conduit-browser-image.json`, and the declared immutable assets. It verifies
the image and every content digest before copying those assets into the package.
The consuming application only installs or serves that resulting directory;
it does not invoke the producer or compile Conduit.

Repository maintainers can produce an npm-style package from a reviewed bundle
with `cargo xtask host browser-sdk-package --bundle <bundle-directory> --output
<new-package-directory>`. Then run `npm pack` in the output directory or serve
the complete directory from a same-origin static root.

To inspect what will be deployed, read `bundle/browser-sdk-package.json` for
the pinned package, Profile, IMAGE, and file identities; compare its `files`
entries with `bundle/browser-bundle-release.json`; then inspect
`bundle/conduit-browser-image.json` for the selected implementation set and
`browser-page.json` for the reviewed distribution ABI and module graph. The
loader repeats these identity and closure checks at runtime.

The BrowserHost surface carries exact Host and Boot identity, while its current
offers remain a refreshed projection of Boot evidence. Forms and Body workload
operations use that same admitted Browser runtime. Typed refusal classes retain
machine category, operation, and runtime evidence. Typed events, full Play
lifecycle, and reload recovery are layered on the same Host and Body contracts.
