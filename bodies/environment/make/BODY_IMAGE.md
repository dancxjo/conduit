# Body construction for the MCU image

The first appliance is delivered with its body built into the MCU image.
Its AP reports the running body's measurements and offers invitations for
other hosts to join later. Local sampling and seven-segment display do not
depend on a connected browser or Pi.

`PreparedBodyImage` uses the existing Crèche `BirthDraft` for its generated
friendly name and exact reviewed initial plot selection. Make supplies the
construction UUID and attributable birth Sign. The ordinary body biography
retains the name, so boot does not regenerate it. The name is metadata;
independent births retain distinct identities even when their names coincide.

The current producer creates initial Lulled biography material without claiming
a current Host, Boot, capability or Play. Linking it into firmware, local Root
admission/start, simultaneous display/network memory admission and AP invitation
delivery remain integration work. This component alone is not that appliance.

Check the construction contract with:

```sh
cargo xtask prove environment body-image
```

## Running this draft

From a checkout of this branch, run:

```sh
cargo xtask prove environment body-image
```

This runs the build-side birth/biography contract tests and strict Clippy.
It does **not** generate a flashable appliance image. There is no complete
appliance image-generation command on this branch yet; the existing
`prove environment display-esp32` builds a display diagnostic only and must
not be presented as the body appliance.

The image-generation entrance must eventually perform these steps together:

1. Check the actual environmental sampling and display Plots and Masks.
2. Create and retain the construction UUID and attributable birth Sign;
   call `PreparedBodyImage::prepare` to reuse Crèche selection and naming.
3. Link the biography and checked construction material into the ESP32 image,
   keeping future Boot identities, resource possession and effect authority out
   of the build artifact.
4. On the physical MCU, initialize actual resources, admit the local host,
   bind its fresh Boot, plan and start the body through the ordinary kernel.
5. Serve current measurements and ordinary body invitations over the AP while
   sampling and displaying temperature/humidity without another host.

The final build command, output image path and deployment procedure will be
recorded here when that entrance exists and has been exercised. A separate
agent owns the Crèche plot; this draft consumes its existing library contract
and does not implement or replace that plot.
