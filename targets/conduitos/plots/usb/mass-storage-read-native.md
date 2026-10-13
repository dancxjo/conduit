# Native storage recovery

`mass-storage-read-native.conduit` connects ordinary checked block requests to
Root authorization, the Source-defined BOT provider, and native settlement.
The fixed first profile reads LUN0, LBA0, one 512-byte block, using sixteen immutable
Source CBWs with distinct tags 1 through 16. Root must independently admit storage and USB ownership.

Package without guest execution:

```sh
cargo +stable xtask make conduitos protocol-source \
  --source targets/conduitos/plots/usb/mass-storage-read-native.conduit \
  --specializations targets/conduitos/plots/usb/mass-storage-read-native-specializations.json \
  --entry bot-storage-one-read --output-dir work/bot-package
```

Phase decisions are balanced to remain within the portable expression depth
bound of 16. Explicit variant guards precede payload access. Storage byte bounds
retain exact native metadata through record and variant projections; larger USB
byte bounds require a Source length guard.

Checked packaging proves Source expansion only. A fresh storage-capable product
kernel, exact block comparison, retained-resource trace, and original native
retirement receipts remain required for ordinary guest acceptance.

The boot fixture accepts one input record per port. A finite Flow can carry its
first `canonical_bytes` value and up to fifteen `subsequent_values`, each using
the same exact checked port kind. The containing runtime advances sequence only
after admission succeeds, retains a value under pressure, and closes the input
after its final accepted value. It supplies no BOT command or retry policy.
Repeated-read acceptance still requires sixteen actual device transactions and
independent command-tag validation. Physical storage, the
adversarial fault matrix, and cross-architecture acceptance remain open.


## Reset recovery boundary

`mass-storage-reset-policy.conduit` owns the finite reset → clear IN halt →
clear OUT halt sequence for the pinned interface 0 / endpoints 0x81 and 0x02.
Only exact zero-data acknowledgments advance; a failed acknowledgment refuses
without replaying a command. Prepared Kernel tests enforce one control owner,
request order, one terminal result and no later effect after failure.
[Native standalone evidence](evidence/native-bot-reset-d2ab8e8a/validation.json)
retains successful exchanges and a clear-IN STALL refusal.

This is not yet interrupted-transaction reentry. Current native cancellation
acknowledges DisableSlot, revokes selected possession and seals the attachment's
host. Further control calls on that stopped host refuse. Clearing software flags
would not restore the disabled endpoint contexts or revoked authority. Recovery
of an interrupted configured attachment requires class-neutral, acknowledged
endpoint quiescence/rearm that preserves EP0, followed by the plotted reset
sequence and a fresh independently authorized read. Failed stop, stale attachment
or failed recovery must prevent reentry. Existing full-slot retirement remains
the safe terminal path until that lifecycle is implemented and proven.
