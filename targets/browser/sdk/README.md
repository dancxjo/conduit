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

The BrowserHost surface in this package is intentionally a read-only identity
and current-offer projection. Forms, Body lifecycle operations, typed events,
effects, and recovery are separate API contracts that build on the same
admitted Host and Boot.
