# Actual native assignment and resident configuration footprint

The public software-ready-esp32 proof now projects the actual scoped appliance
Plan using a named finite local sensor profile: 15 nodes, 14 local cords,
one port per node, two native resources, no remote/resource cords, 48 KiB
assignment and 32 KiB declared runtime-state ceilings. These are construction
ceilings, not a completed Root memory/work admission.

The live Root reports produce a 46,411-byte standard native assignment.
The existing Core decoder validates its Host/Boot, inventory, digest and
resident computations on the planning host. The host retains the full Plan,
assignment and generated resident material in captured-construction.zip.
Native resource/capability/Body ownership remains untransferred on the device.

After the earlier network fallback, Root retains 86,536 of 114,688 heap bytes,
leaving 28,152. The whole 46,411-byte receive buffer cannot fit: its shortfall
alone is 18,259 bytes, before other installation work. Of the assignment,
39,552 bytes are seven computation configuration records already represented
by the trusted Source. The other records plus header occupy 6,859 bytes.
That remainder is a size observation, not an implemented compact wire format.
The declared runtime-state projection is 19,770 bytes; this is not total RAM
for Root, initialized Backs, native leases, kernel storage, Faces and recording.

Make now emits complete standard computation payloads directly from checked
Source before any Host/Boot/Plan exists. CPU conformance and the live projection
compare all seven payloads byte-for-byte with the ordinary native encoder.
The emitted Source module compiles in the managed generated-recipe proof.
These records prepare a future borrowed representation; no segmented decoder,
compact transfer, device assignment admission or storage bypass is present.
The existing independent resident-layout and computation checks remain intact.
The exported full resident recipe is trusted-build review material, not a
reason to trust a packet received by the device.

All 25 native construction tests pass, including a complete two-resource
projection, allocation-free CPU table-admission round trip, stale Boot refusal,
and refusal when a one-resource ceiling is supplied. Both managed generated
recipe modules compile. Strict Make all-target Clippy and the ESP32 release
build pass. The new Source record arrays are not yet consumed by device
admission, so the build retains two additional unused-material warnings.
The rebuilt image was not flashed; the receipt distinguishes its hash from
the running, previously verified policy image. Raw captures are archived;
repository build logs normalize whitespace without changing raw packet data.

Body installation, complete runtime admission, sensor/Face/Mask execution,
positive station/mDNS, recorder custody/transport and the physical overnight
run remain unfinished. This is development evidence, not dev/stable acceptance.
