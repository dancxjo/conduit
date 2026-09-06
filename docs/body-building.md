# Body building and Spores

Body building is a repository-development orchestration layer above accepted Host fabrication:

```text
checked *.body.conduit
  -> checked referenced *.host.conduit
  -> existing Host configuration -> PROFILE -> BUILD -> IMAGE
  -> Body binding
  -> Spore package
  -> optional supported deployment adapter
```

It is not a planner, runtime, package manager, or source of live Body presence. A build can record an intended prejoined Part or a finite self-joining invitation identity. It never creates a Host ID, Boot ID, current offer, Line, Plan, Play, presence, or runtime authority. A deployment receipt likewise records only prepared artifact transfer and explicitly denies boot, join, presence, and readiness claims.

## Entrances

Use the checked example with:

```text
cargo xtask body check bodies/pete/profiles/pete-r1.body.conduit
cargo xtask body show bodies/pete/profiles/pete-r1.body.conduit
cargo xtask body build bodies/pete/profiles/pete-r1.body.conduit
cargo xtask body build bodies/pete/profiles/pete-r1.body.conduit --host brainstem
cargo xtask body deploy bodies/pete/profiles/pete-r1.body.conduit --host forebrain
```

`check` and `show` parse descriptors and reuse Host-configuration validation without invoking target builders. `build` emits `image.json`, `build-manifest.json`, and `spore-manifest.json` beneath one directory per selected Host. `--host` selects exactly one fabrication package. The checked example covers hosted native, Pico W, and browser targets, and both prejoined and self-joining bindings.

`body` is a canonical Conduit document role parsed by the same tokenizer,
declarations, structured values, spans, and diagnostics as `form` and `host`.
Each repeated `host` declaration references checked `*.host.conduit` source and
then enters the existing Host configuration path. Body construction has no
private parser and creates no Host, Boot, OFFER, OBSERVE, ADMIT, Line, Plan, or
Play truth. Canonical `*.body.conduit` documents are the only Body construction
source; repository loaders do not infer or import a second format.

The output kind in a Spore is an exact requested target packaging class. Deployment is available only when the selected fabrication package declares an adapter. Body build does not manufacture runtime Host, Boot, or physical-success truth.

## Architecture packages

Each package owns a target pattern, toolchain identity, build adapter identity, supported output kinds, optional deployment adapter, finite maxima, and Base-implementation-to-feature mapping. Selected checked Bases deterministically derive the recorded feature closure. For example, Pico `serial/text -> pico/usb-cdc@1` selects only `line-usb-cdc`; ESP32 Bluetooth adds `bluetooth` while the kernel-only specimen omits it.

The generic Body layer calls the existing `build_host_image` path and records which fabrication package and adapter were selected. Adding a target is localized to another package contribution rather than another target switch in Body orchestration. A dependency-graph test keeps the descriptor/Spore model crate free of target SDKs, browser build CLIs, and unrelated speech assets.
