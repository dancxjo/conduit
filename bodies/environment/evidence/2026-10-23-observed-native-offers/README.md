# Actual Root offers composed for host-side planning

The public `cargo xtask prove environment software-ready-esp32` entrance now
captures native/software/native reports over the attached Root UART and feeds
that invocation's capture through the checked Source recipe on the planning
host. It emits planning-advertisement.json and composition.json alongside the
physical readiness receipt. This invocation retained 15 offers: two actually
retained native providers and 13 initialized software implementations.

Composition checks unchanged complete native advertisement, exact Host/Boot/
offer generation, exact checked Source identity and cadence, uninstalled state,
exact Source-compiled software offers and array extent, and no duplicate ID
with native offers. The software ingress is bounded to 64 KiB and capture file
to 128 KiB. Stale scope, changed native inventory, altered Source/cadence,
installed state, malformed/unknown headers, missing/duplicate/forged software,
wrong extents and ID collision refuse distinctly. Historical report replay is
CPU conformance only; the live log and receipt record the physical capture.

The API does not authenticate an arbitrary file. Its caller owns authenticated
Root transport and currentness; retained files are historical evidence, not
ongoing availability or authority. No native provider is instantiated on the
planning host. No Root capability, Body membership, Plan installation, sensor
sampling, Face projection, station join or recorder is claimed here.

The firmware rebuilt with the updated host-only Make dependency and lock, but
was not flashed again. The receipt distinguishes the installed, earlier
verified ELF from this rebuilt artifact. The board remains in its verified
post-fallback Boot. These are development receipts, not dev/stable acceptance.

All 24 native construction tests and compilation of both generated recipes
pass. The focused composition test also requires the ordinary planner to return
AuthorityGrantMissing for this composed advertisement with no Root grants;
it does not accept an unrelated planning failure as authority proof. Make
all-target and xtask Clippy pass with warnings denied. The ESP32 release build
passes with its existing two unused timer-issuance warnings pending real install.
