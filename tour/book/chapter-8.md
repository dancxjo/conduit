# Birth, spores, and the Crèche

A durable Body needs a beginning. The Crèche owns that bootstrap path:

choose or open a Seed Form, birth the Body, give it first machinery, add physical or virtual Hosts, and graduate to ordinary operation.

The **Seed is the Body's initial Form and birth provenance. It is not the only Form the Body may ever run.**

```conduit birth
form morse-network {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
```

<!-- conduit-first-host -->

A **spore** is a Body-bound deployable artifact carried to the target and planted with that target's ordinary mechanism.

Examples include UF2 for RP2040/Pico, HEX for AVR, native ESP32 flash images, IMG for SD-card targets, ISO or other boot artifacts for ConduitOS PCs, and finite hosted packages such as ZIP for browser or native hosted targets.

<!-- conduit-physical-host -->

Writing or flashing an artifact, observing a Boot, and admitting a Host are distinct facts. Conduit keeps those distinctions explicit.

<!-- conduit-graduation -->

Graduation ends the temporary Crèche path, not the Body. Later, Patchbay or another compatible reader can project Seed history, current Forms, one Body-wide Wake/Plan/Play, machinery, and resource use.
