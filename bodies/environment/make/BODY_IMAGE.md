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
measured native profile reserves 41,428 stack bytes and contains its exact
construction biography (its byte size depends on the generated name). Four CPU contract tests cover local installation and its
refusals. Local installation has also passed the two-Boot hardware check below;
Plot startup and AP invitation delivery remain unverified. Mutable biography persistence and reboot continuity also
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

Core's canonical field encoders now write to either stored bytes or a streaming
fingerprint sink. Plan, fragment and callable-Front fingerprints consume the
same field order and length prefixes, borrowing immutable computation and
semantic-contract payloads instead of duplicating them into hash scratch.
Frozen historical vectors cover UTF-8, embedded zeroes, structured payloads,
boundary lengths, and equal-length content changes.

The actual environmental sampling Plan's CPU measurement records **593 bytes**
of additional peak heap for Body-plan validation (previously 33,163), compared
with 142,584 bytes for a rich Plan clone alone. The CPU profile now enforces a
4 KiB validation scratch ceiling. These are 64-bit host measurements, not Xtensa
admission figures. The existing sensor-outcome kernel proof checks the measured
validation path and still passes all seven transcript scenarios.

The native Body owner now retains a private immutable `BodyPlanSeal` after
ordinary Plan validation, using the same lifecycle implementation as the rich
Plan owner. On the actual sampling-plan CPU measurement, seal capture and a
retained clone each peak at **948 bytes**. The seal preserves identity and
current Host/Boot/offer scope; it is local validation evidence, not serialized
execution material or Play authority. Capacity and unsupported nested execution
are explicit refusals. Tests compare rich/sealed proposal, startup, Play and
lull behavior, and reject forged or deserialized seals.

The rich Plan is still needed during construction and execution preparation.
These host measurements do not establish that Root, biography, sampling, timed
display, network SDK, planning scratch, and required lifecycle evidence coexist
within the MCU budget. Compact construction and complete preparation/steady-state
accounting remain required.

## Physical local-installation check

The ESP32 WROOM (revision 3.1, 4 MiB flash) booted the native image twice. Both
boots retained Body `9e927a828ada6bf173452a3c44e65866e84f4c189ef582bc34bed923dd317987`
and generated name **River J. Collins**, with different actual Boot identities.
Ordinary membership reached revision 2; its Host, Boot and offer generation
matched the initialized Root advertisement. Root owns the Body session, and its
management report derives `body_installed` from that retained owner. It reported
`true` while `play_started` remained `false`.

The tested ELF SHA-256 is
`89a39a5955cd594c0496495d48fd9c0fad586daeab6fd64089cdd4f4dd49949f`.
After local installation, Root reported 70,464 bytes used out of 118,768, with
an 80,436-byte peak. At the first management offer it reported 74,580 used and
a 90,344-byte peak; the separate radio SDK arena reported 33,408 used and a
33,436-byte peak. These are observations at startup, **not** worst-case
combined planning, execution, client-network, or invitation admission bounds.

The test preserved a checksum-verified full flash recovery image outside Git.
Afterward that operational firmware was restored; two live sample sequences,
AP readiness, and successful display writes were observed. The attached device
is currently running that temperature firmware, not this incomplete native
appliance. Raw serial captures and recovery binaries are kept outside Git.
