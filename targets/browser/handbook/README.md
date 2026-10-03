# Your local Conduit Handbook

This development application implements the Handbook client for [#4804](https://github.com/dancxjo/conduit/issues/4804). Browser acceptance is still being validated; this document does not claim accepted-release status.

Build a complete static directory from the repository root:

```sh
cargo xtask make body static \
  --application targets/browser/handbook/handbook.application.template.json \
  --handbook --output target/handbook-static
```

Publish the complete directory through a static HTTP host. HTTPS, or a trusted local loopback origin, supplies the browser APIs used for persistence and cooperative ownership. No shared Body service or live backend is needed.

The optional `?participate=owner` entrance admits this browser Host into an
already running Linux-owned Body on the same computer. It opens no local Body:
the page shows its exact Host ID and public verifying key, then accepts the
short-lived `ws://127.0.0.1:PORT/conduit` window that the foreground Body owner
explicitly authorizes for that Host. The production SDK and WASM runtime prove
admission and display the owner's biography. This local window proves membership
and presence, not a transferred Plan, Play, or remotely executed Plot. A public
HTTPS deployment cannot use this loopback window to reach a different computer;
the ordinary static Handbook remains independent of it.

Open the Handbook, choose **A clock you can stop** or **Turn keystrokes into text**, and edit the Plot source. Live highlighting comes from the packaged Rust/WASM syntax projection. **Try in my Handbook** checks and installs the exact source in your local Body; an invalid edit reports its refusal. **Lull** ends execution, and **Wake** admits another run.

**Open in Patchbay** opens the resident Patchbay application. Select the example from its list to inspect the actual checked Plot and execution projection. DOM/SVG is the browser Mask realization of this Show, not a second semantic graph store.

Reload and chapter navigation recover your retained Body under a fresh Boot. A second browser profile gets its own Body. Open **Your body and browser** to inspect evidence, **Release this tab** for another tab, or **Start my Handbook over**. Starting over clears the application's Body and state while preserving the browser Host identity.

The template names reviewed resources; `birth.json` names initial resident Plots and lesson sources. Prose remains in `wiki/`. The application module orchestrates public SDK operations, while Rust owns checking, lifecycle, workset and planning. See [Static Body applications](../../../docs/contributing/static-body-applications.md) for packaging, identity isolation, resource boundaries and required acceptance evidence.
