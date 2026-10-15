# Normal PC USB protocol preparation

For #5585, repairing the accepted #5437 product path. The normal PC image
previously exhausted its declared 16 MiB arena before executing the checked
CDC class. Preflight expanded the class a second time despite Boot retaining
its exact prepared entry. It now borrows that entry after comparing both the
package digest and selected entry; mismatched preparations refuse.

Removing the duplicate is necessary but insufficient: a fresh 16 MiB guest
still exhausted the arena during class preparation. A 32 MiB candidate also
failed after successful descriptor preflight. Both failures are retained.
The canonical PC profile now declares the same finite 256 MiB preparation
arena already used by the checked protocol Source profiles. This increases
the normal PC memory requirement; it does not add automatic growth or loosen
USB transfer, authority or operation bounds. Full retired Play and Sign
retention remains in place.

At source `c77ea77fd3372d7b03bf62a8522970cbbb82a33d`, the canonical normal
PC live image built and actual QEMU 10.2.1 CDC execution passed. Independent
wire validation saw two discovery controls, five class controls and exactly
sixteen OUT/IN pairs with exact payloads. Play reported zero allocations and
reallocations, stopped and retired complete. A second guest with the same
valid device and a mismatched Root packet size refused after discovery with
zero class controls and zero endpoint transfers.

The five native Rust files are byte-identical to source
`b9349abfc038d84dc5534d739a0f89cd3c4c969a`, where 215 focused native,
controller, USB Base and preflight tests passed with strict Conduitos Clippy.
The narrowed candidate also passed four preflight tests and strict Clippy at
`b86f11741f966ece3403297dc0c2d32fd76bb363`. The current guest source includes
the subsequent accepted thermostat change. Exact current guest identities,
wire facts and hashes are in the adjacent evidence receipt.

Proof class is freestanding emulator, not physical compatibility. The current
BOT matrix passed normal bounded reads and six other transfer/fault cases,
but STALL terminal propagation failed with a Host-call refusal. Two external
cancellation cases passed; their verifier explicitly does not establish a
late native controller completion. #5586 and parent #5253 remain open.

## Ordinary product boot verification

Candidate CI run `38693431965`, PC target job `116287711163`, exposed a
second integration gap: the ordinary product verifier still launched 64 MiB
while the image correctly required its declared 256 MiB arena. The retained
serial Sign refused `runtime-arena-unavailable`; this was not an observation
timeout or transient runner failure.

All normal PC image entrances now use the shared 512 MiB emulator setting:
two-boot verification, interactive journey, visible demo, front-door proof,
repeated keyboard proof and Crèche-spore acceptance. The normal large-arena
Boot Sign identifies that configuration. Dedicated small proof profiles retain
their own settings. Product boot QMP uses a short temporary socket, so longer
workspace paths do not prevent the emulator from starting.

At source `85094419d8e52a160ed4c4318619215dea5754f1`, the canonical product
rebuilt, two independent normal boots produced fresh Host/Boot identities,
and the twelve-step actual QMP keyboard journey passed. The observed Sign
reports the 256 MiB arena and 512 MiB normal-product emulator profile. Fresh
CDC success and mismatched-Root refusal cases also passed at this source.
The arena test, four demo tests and three product-boot tests passed, as did
strict all-target xtask Clippy. Exact identities, current guest receipts and
hashes are in [evidence/usb-normal-product-pc-boot.json](evidence/usb-normal-product-pc-boot.json).
The earlier source-pinned observations above remain historical facts.
