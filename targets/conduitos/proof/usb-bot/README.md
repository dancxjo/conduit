# Native BOT proof tools

These are bounded external USB DEVICE fixtures and independent receipt checks.
They do not provide Conduit Host offers or execute class policy. The reviewed
class policy remains in `plots/usb/mass-storage-read-native.conduit` and
`mass-storage-read-producer.conduit`.

`bot_device.py` supports the pinned read-only READ(10) profile and explicit fault
modes. USBredir framing is inherited from the retained CDC device fixture;
unsupported firmware SCSI probes stall. The fixture allows at most 512 messages,
512KiB of wire data, 2048-byte transfer payloads and sixteen supported commands.
The AArch64 firmware prefix is retained, not discarded from the transcript.

`run_matrix.py` accepts exact paths to the workspace, xtask, kernel, build record,
checked package, Root request, expected block and independently built decoder.
It creates new output directories, keeps all outcomes and exits nonzero on any
failed check. `qemu_bot_wrapper.py` owns its Unix socket and device process.
AArch64 additionally requires `--qemu` and `--aarch64-firmware`.

The independent decoder uses `verify_storage.rs` with the ordinary
`conduit_core::validate_canonical_structured_value` API. The included command adapter supports `native-storage-reuse` (sixteen exact blocks) and
`native-storage-terminal <tag>` (one exact original request/ordinal and terminal
result followed by retirement). Build it in a new work directory with
`python3 targets/conduitos/proof/usb-bot/build_decoder.py --workspace . --output work/bot-receipt-decoder`.
This creates an isolated proof crate using this workspace's canonical value validator;
it does not add a product package. The module is not a production driver. `verify_cancellation.py` independently checks
Root budget refusal, no block publication and original USBredir cancel identity.
A device response after cancel does not prove admission of a late native event.

The recorded runtime receipts under `plots/usb/evidence/native-bot-e9a277e4/`
are pinned to `e9a277e4cb38805033c781355600122b49876d5f`. Both product architectures
used the identical package and Root request. Later branch reconciliation added
CDC test files only; the receipts must not be relabeled as proof of a new kernel
identity. Retained receipt checks are historical evidence, not fresh CI execution.

Reset reentry, physical hardware, the full substrate lifecycle matrix and native
USB audio remain separate acceptance work. Successful terminal refusal without
retry does not establish reset recovery.
