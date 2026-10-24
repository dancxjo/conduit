# Retire transferred output construction metadata

After compact output assignment transfers all three initialized operations,
their availability owner now releases its descriptor vector, offers and
configuration text before the scheduler box is allocated. The scope and offer
epoch remain inspectable, with no available operations. No driver, selected
kernel table, canonical value, Sign, native capability or grant is discarded.

Explicit retirement refuses without mutation while any initialized operation
remains. It allocates nothing, frees descriptor backing, preserves empty offer
inspection and returns `AlreadySelected` for another selection in the retired
scope. The original full preflight still precedes exclusive transfer; all
assigned output operations are moved exactly once before descriptor release.

All twelve Make `display_output` CPU tests pass, including partial-transfer
refusal, post-retirement scope/offer inspection, repeated selection, compact
assignment execution and physical-driver execution with CPU pin fixtures.
The hosted-store execution cases now retire metadata before running both valid
and refused Frames. Strict scoped Clippy passes. These cases remain owned by
`cargo xtask prove environment display-output`.

The ESP32 display-ready-root release image links with this retirement path.
That is compilation evidence only: no firmware was flashed, no new hardware
heap measurement was taken, and no combined sampling/Mask/output admission is
claimed. Existing memory guards, queue/Sign bounds and Root control reserve
are unchanged. This removes retained construction backing; it does not by
itself prove the combined appliance fits.
