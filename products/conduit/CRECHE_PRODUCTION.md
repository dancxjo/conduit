# Reviewed Crèche image production

`CrecheEncounterAdapter` runs the shipped ordinary Crèche Plot through an
installed Host operation. Its Face carries the actual producing Body, Wake,
Plot Plan and issued Body Play. The selected renderer must retain both its
acknowledged Show and the exact returned input correlation. Structural Show
validity alone admits neither display custody nor input.

The controller moves one opaque `CrecheReviewedHandoff` to the make owner.
External callers cannot replace its reviewed state while retaining its request.
`CrecheImageProduction::prepare` consumes that handoff and records one ordinary
birth. Per-Host failures keep the same job and birth for retry.

For each selected Host, the owner supplies an empty read/write output handle
and either reviewed release machinery, target-build receipt and readable media,
or explicit Linux source/toolchain/boot-package/workspace grants. Selecting
“Build” does not supply these grants. The Linux path uses the existing
`BuilderBase`, `StdBuilderAdapter` and `LinuxSourceBuilder`; it accepts no UI
command string. Reread artifacts must match the actual builder receipt.

The current binder supports the declared native PC prepared-birth consumer in
ISO media. It patches one pristine reserved slot and independently reads back
the frame and final file digest. Unsupported consumers refuse. SD manufacture
and native Linux package production remain separate work, not aliases for ISO
or serialized Host descriptions.

`finish` requires all selected products and rereads the entire batch. A refusal
returns the retained job. `withdraw_product` permits a fresh verified replacement
without rebirth and restores its byte allowance. Product files are private;
there is no public mutable-capable file getter. `complete_images` revalidates
bytes before accepting the opaque completion for the exact reviewed request.
Successful `complete_images` enters the active **Images** Face. It does not
finish the installed operation or transfer the products. The same acknowledged
Show and returned typed interaction checks continue to protect Save and Done.
Each selected Host has a Save action; Done is a separate canonical action.

Save requests an owner grant, rather than granting a pathname or device effect.
`pending_save` exposes the exact selected artifact receipt. The Host supplies an
already admitted empty read/write regular file and a finite byte bound to
`start_save`; file work runs outside kernel polling. `save_host` verifies the
source, copies in bounded cancellable chunks, flushes and synchronizes the file,
independently reads its bytes back, then verifies the source again. A failure
preserves the prepared batch and caller-owned partial file. Success retains a
private-construction `conduit.creche/saved-image-receipt@1` containing the exact
request, Body, Host entry, target, digest and byte count. It records saved bytes;
carrier realization, boot and join remain false. Saving again replaces that
Host's retained save receipt only after a new successful save.

Done can finish without saving every image. The Host explicitly services its
pending canonical Done through `start_finish_images`. Its worker revalidates the
whole batch and constructs bounded
`conduit.creche/images-completion-evidence@1`: the canonical prepared evidence,
its SHA-256, and actual retained save receipts. Only that completion lets poll
publish the exact `Prepared` evidence digest and permits `take_products` to
transfer the batch once. Done does not invent a save receipt or infer deployment.
After completed retirement, `take_retired_products` instead recovers retained
products without claiming successful Prepared completion.

Production and completion revalidation perform blocking I/O.
`start_make_worker` consumes the exact review internally and attaches one owned
worker outside kernel polling. Its closure must pass the supplied cancellation
flag into production grants and retain one job across retries. Retirement revokes
interaction, cancels and joins that worker without holding mailbox or renderer
locks, then drains presentation. Rejected completion retains opaque products and
a bounded reason for one-time retrieval, including after retirement. A worker
panic classifies failure and persistently refuses successful effect drain; the
Mask is still cleaned up. Blocked OS reads remain a cooperative limit.

The `native-creche` feature supplies `LocalCrecheNativeOwner`: one prepared Mask
and one caller-owned initialized scanout, bound to the actual issued Body Play.
It retains actual render/Show/input receipts and acknowledges focus redraws
before accepting further input. The UI loop must service its bounded retirement
handshake while the Body worker drains. Controller ACK/input checks report
custody pressure rather than blocking that loop.

The `native-creche-sdl` feature implements `LocalCrecheScanout` for the same actual
SDL `Scanout` from the standalone `conduit-linux-display` package at
[`targets/linux/display`](../../targets/linux/display/README.md). Retirement
revokes that initialized scanout synchronously on its owning thread. The package
is excluded from root workspace membership; the feature adds a Linux-only
optional dependency without duplicating the renderer.

The standard Host now has optional initialized Linux display readiness
attachment, with private lifetime observation, a finite Show resource and
transactional loss retraction after drain. `prepare_linux_creche_output` now bridges
that exact live Host attachment to the same initialized KMSDRM scanout and the
ordinary native Mask. It retains exclusive scanout custody, uses a fresh private
issuer key, and refuses a different or lost display instance. Preparation neither
issues Body Play nor presents or acknowledges a Show. This is a trusted
cooperative Host-driver boundary, not isolation from hostile Rust code.

`pump_linux` composes one UI turn with the actual retained SDL scanout and
selected Mask. It services retirement first, publishes the controller's actual
Face, drains keys queued under an older semantic Face, and acquires at most one
observation. A pending semantic return suppresses further key selection. Driver
inspection bounds are explicit; keys still pass through the ordinary Mask's
acknowledged Show/focus route. Quit drains presentation and leaves cancellation
and collection of the Body worker's actual outcome to the application owner.

The full native application entrance still needs start/stop composition of the
ordinary Body runner with this UI pump and real make/save grant selection. The Images model, save machinery
and owner APIs do not themselves acquire display custody, choose a file for the
user, deploy a carrier, or supply a complete launched application.

The focused software tests use canonical review, actual admitted fixture files,
streaming image binding, retained receipt custody and the installed Host owner.
They cover changed bytes, cancellation, source/build mismatches, one-birth retry,
multiple Hosts, active Images Save/Done, exact saved-byte readback, partial-save
cancellation, one-time completion, real native Mask rendering on an explicit
CPU output, redraw ACKs, typed input and worker cancellation/retirement. They do
not establish a positive source image build, native device output, SD boot or ordinary later-Host join.
