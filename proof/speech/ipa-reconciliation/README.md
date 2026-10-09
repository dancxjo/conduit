# Canonical IPA constructor reconciliation

This packet records local proof for #5329 and #5260's selected four static Value
constructors in #5319. Parsing, complete inventory admission and encoding happen
before Play. The installed Back delivers one exact typed Value, retains it under
pressure and clears it on cancellation. No HostCall or audio playback is claimed.

`receipt.json` pins tested source `bee5007b966dfadda297061e5b2876eb56aae438`,
terminal commands and log hashes. The supported `cargo xtask check speech-ipa`
passes 1,121 Rust tests across 25 targets, then compiles the Speech semantic
bindings for `thumbv6m-none-eabi` without default features. These include generic
nominal/refinement regressions, Plot and Speech libraries, four actual installed
constructor journeys, independent Native/domain re-admission, all five native
speech offers, the complete std library and product human/JSON diagnostics.
Nine existing environment-dependent std tests are ignored; exact names and
requirements are retained in the receipt and suite log. None are IPA tests.

Separate terminal runs pass strict all-target Clippy for the five affected
packages, workspace Rust formatting and 11 CI history/ancestry regressions.
Clippy is not claimed for the full workspace. `fmt.log` is intentionally empty:
the command exited zero without output.

`source-manifest.json` hashes the frozen changed source, tests, manifests and
design documents before these evidence/documentation updates.
`native-identities.json` records all nine selected Native identities. The entire
generated Speech binding source has the same SHA256 as the original selected
#5319 candidate; generated Rust and build artifacts are not checked in.

After #5208 landed, the IPA stack was replayed onto its `dev` merge commit
`33b85c2c45ddf5c903ef554e3c7f96570e721acd`. The replay head in the receipt has
the exact same complete Git tree as the tested head. This subsequent packet and
documentation update changes no executable inputs. The preserved tested commit
and the two competing candidates remain recoverable; #5327's historical HostCall
proof is evidence for its own route, not interchangeable with this packet.

Required exhaustive CI on the submitted head, approved acceptance, accepted
integration and stable coverage remain open. This packet does not authorize
closing #5260 or supersede those gates. No `ph[…]`, `ph/…/`, `r/…/flags`,
Conduitese formatter or full Unicode IPA implementation is claimed.
