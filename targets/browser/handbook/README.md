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
short-lived `ws://127.0.0.1:PORT/conduit` window that the installed Body owner
explicitly authorizes for that Host. With the owner service running, use
`conduit body browser-window --state-dir … --expected-host-id … --new-host-verifying-key … --authorize-window`;
quote the copied JSON key array as one argument. The command prints the exact
Body ID and window URL to enter on the page. For a returning browser Host,
omit `--new-host-verifying-key` and retain its credential in browser storage.
The production SDK and WASM runtime prove
admission and display the owner's biography. The browser builds native controls
from the current Face's declared action names, bounded text inputs, and finite
choices; WASM checks the selected bytes before emitting a typed interaction.
The installed owner currently accepts the reviewed clock interval action while
that authorized window and browser presence remain current. Lull a running clock
first; the owner checks the current credential, Part, Face, Show, and typed
argument, changes its checked workset, then supplies a fresh Face. The next
start requires a replacement Plan. A closed window does not remain an action
route. This local window proves membership and presence, not a transferred
Plan, Play, or remotely executed Plot. A public
HTTPS deployment cannot use this loopback window to reach a different computer;
the ordinary static Handbook remains independent of it.

When the Linux owner was installed with selected speech equipment, **Read this
view aloud** asks that owner to read the browser's current acknowledged Show.
**Check reading** reports the owner's running or terminal outcome, and **Stop
reading** requests cancellation. The owner checks the current carrier, Face,
Show, Host Boot, and selected equipment before starting. Sound comes from the
Linux speaker; the browser does not synthesize it. This direct readout is a
prerequisite for a spoken Mask, not yet an owner-spoken Mask Show or proof that a
person heard the playback.

Open the Handbook, choose **A clock you can stop** or **Turn keystrokes into text**, and edit the Plot source. Live highlighting comes from the packaged Rust/WASM syntax projection. **Try in my Handbook** checks and installs the exact source in your local Body; an invalid edit reports its refusal. **Lull** ends execution, and **Wake** admits another run.

**Open in Patchbay** opens the resident Patchbay application. Select the example from its list to inspect the actual checked Plot and execution projection. DOM/SVG is the browser Mask realization of this Show, not a second semantic graph store.

Reload and chapter navigation recover your retained Body under a fresh Boot. A second browser profile gets its own Body. Open **Your body and browser** to inspect evidence, **Release this tab** for another tab, or **Start my Handbook over**. Starting over clears the application's Body and state while preserving the browser Host identity.

The template names reviewed resources; `birth.json` names initial resident Plots and lesson sources. Prose remains in `wiki/`. The application module orchestrates public SDK operations, while Rust owns checking, lifecycle, workset and planning. See [Static Body applications](../../../docs/contributing/static-body-applications.md) for packaging, identity isolation, resource boundaries and required acceptance evidence.
