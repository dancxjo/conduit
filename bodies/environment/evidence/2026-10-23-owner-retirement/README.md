# Native worker retirement: physical evidence

The supported `cargo xtask prove environment owner-run-esp32` entrance completed
on the attached ESP32 with the DHT11 on GPIO27. Source and image attribution are
in `owner-root-image.json`; the admitted Boot key is retained in `admission.json`.
No private owner seed or network credential is included.

The retained ordinary Body owner verified the device Start-held receipt before
recording Start and separately releasing the worker. The production kernel
committed three canonical DHT observations at the checked 60,000 ms cadence;
all three became ordinary Body Faces. Each observation file's length and SHA-256
match its Boot-signed snapshot metadata.

After verified quiescence (command sequence 40), the owner recorded ordinary Lull
and sent a separate authenticated, exact-reservation Retire command (sequence
41). It verified the Boot-signed Retired receipt before retaining
`owner-retired.json`. Both native capabilities were revoked with zero in-flight
calls before transfer. The retired reply reports actual attachment generations
`[1, 1]` in Root custody, worker phase `retired`, zero execution-context bytes,
and no prepared kernel/context or live capability possession. The terminal
progress/publication status remains inspectable.

Final Root heap usage was 10,172 of 114,688 bytes, with a 105,164-byte peak during
this run. The one-use executor task remains reserved (its held future is 3,536
bytes); retirement does not claim to recycle that task, advance resource
generations, authorize a second Play, or restore ready offers automatically.
The device is retired and inactive after this proof. These files do not prove
an application Line, durable store, overnight recording, network transition,
browser membership, Pi boot client, or physical display delivery.

CPU conformance also checks refusal before cleanup without consuming custody,
held cancellation, running cancellation, drain and cleaned-up failures; native
provider transfer allocates nothing. Root-control tests and strict Clippy pass.
Related work: #6334 (provider retirement) and #6337 (worker/Root retirement).
