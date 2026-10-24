# Fresh native Crèche and graph captures — incomplete journey

Source `92598cefd9a14ff50371b63df41273b4b28294ac` builds the normal x86_64
product with `--locked`, boots in QEMU, and captures the actual Crèche and
Patchbay display. `summary.json` binds the Source, actual Root ELF and ISO hashes,
22 verified QMP PNGs, and lossless serial/QMP archives. These are freestanding
emulator captures, not a mockup, browser substitute, physical or human evidence.

## Review

[Editorial compact](journey-frames/creche-editorial-compact.png) · [Editorial spacious](journey-frames/creche-editorial-spacious.png) · [Phosphor compact](journey-frames/creche-engineering-compact.png) · [Phosphor spacious](journey-frames/creche-engineering-spacious.png) · [Actual Patchbay diagram](journey-frames/patchbay-diagram.png)

The legacy `creche-engineering-*` filenames denote the Phosphor palette in this
harness. **Every current preset actually requests Editorial typography.** They
are not evidence of an Engineering font switch. The public chat outputs use
Phosphor names to avoid repeating that misleading label.

The actual four local Look transitions preserve the complete same Face/revision,
checked component Fore, recipe and layout hashes. Each acknowledges a distinct
local scanout Show and chains its predecessor. No Body action is dispatched by
these appearance keys. `look-transitions.json` retains the actual records and
an independent equality/chain check. Compact scenes use 48 commands; spacious
first pages use 25, inside the unchanged 48-command ceiling. Complete Face
retention does not by itself prove every control reachable on every continuation
page or all large-text/narrow cases.

## Build repairs and their limits

The former ambiguous-compositor observation is reproduced as two complete,
byte-identical loaded images in the earlier retained kernel. This branch reuses
accepted `dev` commit `455aaf0a33a5dddf739935003e77b5da2c3788ac`: compare complete
validated image bytes, allow identical copies, and refuse distinct images.
Five focused extraction checks pass. Extraction also accepts the fresh actual
Root and finds its 29232-byte compositor image. That is structural byte custody,
not authenticated linker provenance, semantic Show proof or general confinement.

The first fresh build refused stale standalone guest lockfiles. Crèche, Mask and
Application lockfiles now retain the Face compression edge and pinned packages;
unrelated versions are unchanged. All three resolve under `--locked`.

The next build refused the Application guest's 2 MiB RX/RO image window.
Minimum-size release optimization lets the same implementation link within the
existing limit: aligned image end `0x005f9000`, below `0x00600000` (28672 bytes
remaining). The separate private-data template is 16 bytes in its existing page.
No image, heap, stack, work, command, text or frame limit was raised. Successful
linking does **not** establish runtime work-budget acceptance.

## Required negative evidence

**The full native journey fails.** Later Patchbay target preparation reports:

```
CONDUIT_APPLICATION_REFUSAL stage=entry sequence=41 request=Some(Commit) detail=Ok(Fault(WorkExhausted)) entries_interrupts_preemptions=Some((41, 129, 1))
CONDUIT_APPLICATION_COMMAND_REFUSAL operation=prepare-target slot=Some(2) target=Some(3) refusal=Application
```

The kernel reports `native-body-application-initialization-refused`; the harness
then times out waiting for `patchbay-action` and stops the guest. Its failed
capture manifest and logs are retained. The cause of the work-budget refusal
is not isolated here; no claim that the image optimization solves it is made.
The complete capture manifest and larger build logs are retained as lossless
archives; `summary.json` also indexes every PNG digest and capture context.
The official journey receipt, subsequent full compositor-lifecycle collection,
headless asset tail and full graphical-profile acceptance are absent. The
monitor did not stop an optional tail: the proof failed before that stage.

Current native narrow/200%/forced-color receipts, actual Engineering selection,
the remaining reference differences and the original parent gates remain open.
No CI, integration, stable publication or issue closure is claimed.
