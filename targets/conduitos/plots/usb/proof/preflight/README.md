# Preflight evidence validators

These validators check independent observations. Their presence is not native
acceptance evidence; a fresh product kernel, exact package/Root identities,
retained serial transcript and paired external USB device transcript are required.

`check_discovery.py SERIAL PACKAGE supported|unsupported` requires one exact
package identity, retained pure Plan/class identities and original attachment,
one raw Boolean output and retirement. Supported discovery must precede class
Base/Play records and complete class retirement. Unsupported discovery must
retire false and refuse without any class Base/Play records.

`check_wire.py WIRE aarch64|x86_64 supported|unsupported` checks paired USBredir
requests/results, architecture-specific firmware configuration prefix, two native
discovery reads (9-byte header/full 67-byte configuration), and then either the
five Source controls and exact 16 duplex transfers, or no class transfers.
The profile and expected byte counts are specific to the pinned CDC example.

Omitting the verdict checks the earlier serial demonstration without preflight;
this compatibility mode does not certify profile admission. Native device
enumeration is excluded from the configuration-control sequence, and no physical
compatibility is inferred.

Checker development revalidated the two retained serial transcripts and used
explicitly synthetic phase fixtures: supported and unsupported sequences were
accepted; missing/extra controls and corrupted discovery records were refused.
Those fixture checks prove validator behavior, not new native execution.
