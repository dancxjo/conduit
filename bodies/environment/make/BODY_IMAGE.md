# Body construction for the MCU image

The appliance is intended to arrive with its body built into the MCU image.
Its AP must report the running body's measurements and offers invitations for
other hosts to join later. Local sampling and seven-segment display do not
depend on a connected browser or Pi.

`PreparedBodyImage` uses the shared `PreparedBodyBirth` constructor from the
existing Crèche `BirthDraft` for its generated
friendly name and exact reviewed initial plot selection. `EnvironmentBodyImage::prepare`
checks the current appliance and display-output Sources and selects their real
checked identities, retaining both expanded identities for linking. Make supplies the
construction UUID and attributable birth Sign. The ordinary body biography
retains the name, so boot does not regenerate it. The name is metadata;
independent births retain distinct identities even when their names coincide.

The sampling material now includes `NativeSoftwareSourceRecipe`, emitted from
that same checked Source. It can initialize retained portable software and lend
its offers without reconstructing the rich Source graph on the MCU. The
ordinary assigned-table validator remains responsible for graph admission;
these descriptors establish no physical possession or effect authority.

The current producer links initial Lulled biography material into firmware
without claiming a future Host, Boot, capability or Play. At boot the trusted
image-install path records ordinary admission/presence events for the initialized
local Root and opens the linked Body as a Lulled member. Existing membership
cannot use this first-install entrance to replace a Boot. Startup, simultaneous
display/network memory admission, and AP invitation delivery remain integration work. This component alone is not that appliance.

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
It does **not** generate a flashable appliance image.

The image-construction entrance in this integration branch is:

```sh
cargo xtask make body environment-image --output target/environment-body
```

It obtains an attributable Sign from the initialized construction Host and
creates a distinct directory under the output path. That directory retains
`birth-sign.json` and `body-biography.json`. On a successful firmware link it
also receives `firmware.elf` and `build.json`, with the image digest and body
identity. The friendly name is generated once for that construction and retained
in the biography. Running the command again creates a new construction.

**This is still an incomplete native integration, not a deployment command.**
The linked-body firmware has passed the unchanged 36 KiB CPU stack guard.
The pinned compiler disables switch-to-lookup conversion for this image because
the ESP32 linker otherwise places those immutable tables in internal RAM. The
measured linked image reserves 41,428 stack bytes and contains its exact
1,973-byte biography. Four CPU contract tests cover local installation and its
refusals; physical local membership, Plot startup, and AP invitation delivery
remain unverified. Mutable biography persistence and reboot continuity also
remain integration work; a linked pristine snapshot cannot replace that history.
The existing `prove environment display-esp32` builds a display diagnostic only.

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

A verified build receipt and deployment procedure will be recorded here once
the complete entrance and boot behavior have been exercised. A separate
agent owns the Crèche plot; this draft consumes its existing library contract
and does not implement or replace that plot.

## Remaining plan ownership cost

Body-plan validation now borrows its nested Plot Plans and sorts only bounded
references. It keeps the existing seal identity and refusal behavior, including
canonicalizing an unsorted candidate. It no longer clones the rich graph merely
to revalidate it.

The actual environmental sampling Plan's CPU measurement records 33,163 bytes
of additional peak heap for Body-plan validation, compared with 142,584 bytes
for a rich Plan clone alone. These are 64-bit host measurements, not Xtensa
admission figures. The existing sensor-outcome kernel proof checks the measured
validation path and still passes all seven transcript scenarios.

The rich Plan itself remains owned and large. This change does not establish
that Root, biography, sampling, timed display, network SDK, planning scratch,
and required lifecycle evidence coexist within the MCU budget. Compact Plan
ownership and complete preparation/steady-state accounting remain required.
