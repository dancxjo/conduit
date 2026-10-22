# Ordinary native product without a graphics provider

Refs #5608 and #5994. The existing supported canonical live-image proof entrance
adds an independent scenario:

```
cargo +stable xtask make conduitos creche-controls-proof --scenario no-graphics
```

A viewport request conflicts with this scenario and refuses before building.
The entrance retains the ordinary product-host image builder and validates exact
clean Source identity, image hash and resolved Make binding. It boots that
unchanged product binary with no VGA device, ordinary xHCI keyboard/pointer,
no network and an external debug-exit witness. There is no product proof feature,
faulting display target, injected interaction or alternate Face renderer.

The bounded collector requires the actual USB-configured stage, paired native
display-setup started/refused observations, exactly one typed `display-absent`
Sign and exact refusal exit 35. It rejects duplicate/wrong/truncated records,
wrong exits and any Face, scanout or source-change publication. Source, image and
transcript custody stay explicit. Display failure occurs before the ordinary
Host/Boot/Show publication, so this scenario never invents those identities.
A successful result proves honest terminal startup refusal; it does not prove a
graphical substitute or recovery after unplugging a previously working display.

A preliminary [original diagnostic](evidence/native-missing-graphics-diagnostic-20261022/)
boots the previously preserved ordinary product Source `bc3ff641a` and returns
exit 35 with typed `display-absent`, no Show/scanout/source effects. Its product
manifest/hash and raw serial/stderr are retained. That internal diagnostic is
not relabelled as enactment of the new supported entrance or a later Source.

The new collector's focused validator passes, including wrong-exit, duplicate,
malformed/truncated, missing-span and forged Show/source-effect cases. Workspace
formatting and whitespace checks pass. Fresh clean-Source supported proof is pending. Existing CPU/fault-fixture recovery and ordinary graphical journey
packets keep their separate proof classes. Physical/human evidence, runtime
provider disconnection, scene-pressure fallback, combined-head CI and normal
merge remain separately required where owned.
