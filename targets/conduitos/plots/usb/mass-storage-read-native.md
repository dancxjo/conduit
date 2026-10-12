# Native storage recovery

`mass-storage-read-native.conduit` connects ordinary checked block requests to
Root authorization, the Source-defined BOT provider, and native settlement.
The fixed first profile reads LUN0, LBA0, one 512-byte block, using immutable
Source CBW bytes. Root must independently admit storage and USB ownership.

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
retirement receipts remain required for ordinary guest acceptance. Fixed tag 1
does not establish repeated-read stale-tag acceptance. Physical storage, the
adversarial fault matrix, and cross-architecture acceptance remain open.
