# Native Root network fallback proof

`cargo xtask prove environment network-fallback` exercises the currently flashed
Root provisioning diagnostic through its real HTTP mailbox, selected production
kernel and retained ESP32 radio provider. It requires Python serial support,
access to `/dev/ttyUSB0`, a fresh uninstalled Root setup control and a host Wi-Fi
connection to `Conduit-Climate`. It does not flash firmware or install a Body.

For attended host setup, `cargo xtask setup environment-host-wifi` installs a
validated sudoers grant for only the existing climate profile UUID and Wi-Fi
interface. `cargo xtask prove environment host-wifi` then reconnects without an
interactive password. The helper neither reads credentials nor edits profiles.

The proof submits one ephemeral nonexistent-network fixture over HTTP, requires
its actual 202 acknowledgement, and sends no UART requests for 30 seconds while
Root advances autonomously. It then requires two stable reports in the same Boot:
one credential call, one failed join call, an association failure, no attachment
success receipt and acknowledged AP restoration. The observation window is finite
and no second join is requested. A pre-existing installed control refuses the
proof rather than being silently reset or retried. Artifacts are retained under
`target/environment-network-fallback/<timestamp>`.

The [physical before/after receipt](../evidence/2026-10-23-network-stack-fix/measurement.json)
records an acknowledged submission followed by a ProCpu stack-guard panic in the
old image. Its installer frame alone requested 43,360 bytes, exceeding the
21,612-byte stack section. The corrected exact three-gear profile accepts only
port ordinal zero and preallocates 128 local Sign entries using the existing
bounded hosted Sign log. It allocates no remote Sign backing and no execution
storage. Complete kernel-footprint reporting includes the retained Sign backing.

The corrected installer frame is 13,136 bytes and the stack section is 32,756
bytes. The same negative fixture completes through the actual radio with one
association rejection and AP restoration; the setup page returns HTTP 200 again.
Root and SDK high-water values cover this one negative case. They do not prove
all station workloads or the environmental appliance's complete memory closure.

This is Root provisioning Control, with actual current issuance and native
owners. It is distinct from climate Body birth, sensor Plan installation, a
successful station attachment, Body-derived mDNS, browser membership, a live
recorder Line or physical overnight recording. Those remain separate obligations.

## Consumed construction material

After installation transfers exact bindings to the kernel and selected providers,
Root releases the rich network Plan and lowering. A preparation refusal still
retains its refusal evidence. Ongoing provider ownership, current Play identity,
terminal failure and restoration evidence remain with the actual control owner.

The [physical Plan-release receipt](../evidence/2026-10-23-native-plan-release/measurement.json)
records the same failed-association/AP-restoration path with retained Root heap
falling from 31,756 to 15,136 bytes. The Root arena remains 64 KiB and the isolated
SDK arena remains 120 KiB. Root preparation peak is still 38,400 bytes. The
setup page returns HTTP 200 in the same Boot after one join attempt. This frees
16,620 retained bytes without claiming complete climate admission, station
success, Body installation or a recorder Line.
