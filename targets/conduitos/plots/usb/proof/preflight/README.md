# Preflight evidence validators

These validators check independent observations. Their presence is not native
acceptance evidence; a fresh product kernel, exact package/Root identities,
retained serial transcript and paired external USB device transcript are required.

`check_discovery.py SERIAL PACKAGE supported|unsupported --root ROOT` requires one exact
package identity, retained pure Plan/class identities and original attachment,
one raw Boolean output and retirement. Supported discovery must precede class
Base/Play records and complete class retirement. Unsupported discovery must
retire false and refuse without any class Base/Play records.
The Root option verifies retained serialized bytes and SHA, then compares parsed
content against the selected media. Null optional endpoints/preflight and a zero
additional Sign budget normalize to omitted defaults.

`check_wire.py WIRE aarch64|x86_64 supported|unsupported` checks paired USBredir
requests/results, architecture-specific firmware configuration prefix, two native
discovery reads (9-byte header/full 67-byte configuration), and then either the
five Source controls and exact 16 duplex transfers, or no class transfers.
The profile and expected byte counts are specific to the pinned CDC example.

Omitting the verdict checks the earlier serial demonstration without preflight;
this compatibility mode does not certify profile admission. Native device
enumeration is excluded from the configuration-control sequence, and no physical
compatibility is inferred.

`decode_discovery.rs` uses Core canonical validation to extract the nested actual
configuration completion, attachment epoch and selected endpoint facts. It compiled
against the current Core artifact used by the native ISO bridge tests. A Core-built
checker fixture decoded a nested short completion and absent/selected endpoints;
truncated bytes, duplicate observations and a wrong primitive Type were refused. Native
acceptance must compare its descriptor bytes to the independent wire validator's
`discovery_configuration_hex`, and its endpoint geometry/epoch to the media Root.

`decode_serial.rs SERIAL observation [RECEIVE_HEX]` validates all 32 ordered serial
results through Core, with exact 12-byte transmissions and expected short receive
octets for each 64-byte request. The wire validator accepts the same optional
receive hex after its verdict argument. The default retains the original 17-byte
reply; `ff0080` prepares a separate three-byte binary short-transfer proof. Both
decoders compiled, and the serial decoder revalidated the retained original native
serial logs; no new short-transfer run is claimed by adding this parameter.

Checker development revalidated the two retained serial transcripts and used
explicitly synthetic phase fixtures: supported and unsupported sequences were
accepted; missing/extra controls and corrupted discovery records were refused.
Those fixture checks prove validator behavior, not new native execution.
