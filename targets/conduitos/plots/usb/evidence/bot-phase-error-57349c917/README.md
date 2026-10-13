# BOT phase-error product proof

The reviewed class policy is `mass-storage-read-reentry.conduit`; the native
provider remains a class-neutral USB transport. These four fresh product Host
runs were built at `57349c9170f9e10b6eedfccf733a5068d97e39ad`. The same checked
Source package and independently approved Root request cross x86-64 and AArch64.
`index.json` records exact file hashes, Source/Root digests and proof classes;
each case retains its kernel build record, packaged-image receipt, observed run,
USBredir wire and external device log. This is emulator proof, not physical proof.

Both `*-recovered` cases submit tag 1, receive DATA followed by a status-2 CSW
whose residue is `0xdeadbeef`, acknowledge reset, clear IN and clear OUT, settle
that original request as command-failed, and submit a separately authorized tag-2
read. The fresh 512-byte DATA differs from the failed command's DATA and is the
exact completed result. Semantic ordinals are 0/0 on the independent storage
placements; USB read ordinals are 0/1/2/3. Play records zero allocations and
reallocations, an acknowledged stop and normal retirement.

Both `*-failed-clear` cases receive the same phase error, acknowledge reset and
then fail clear IN. No clear OUT, new CBW, storage result or later bulk effect
occurs. The guest acknowledges stop and retires without fabricating completion.

Verify from the repository root:

```sh
python3 targets/conduitos/proof/usb-bot/verify_reentry.py --serial targets/conduitos/plots/usb/evidence/bot-phase-error-57349c917/x86-recovered/serial.log --wire targets/conduitos/plots/usb/evidence/bot-phase-error-57349c917/x86-recovered/wire.jsonl --phase-error
python3 targets/conduitos/proof/usb-bot/verify_failed_reentry.py --serial targets/conduitos/plots/usb/evidence/bot-phase-error-57349c917/x86-failed-clear/serial.log --wire targets/conduitos/plots/usb/evidence/bot-phase-error-57349c917/x86-failed-clear/wire.jsonl --phase-error
```

Repeat with the `aarch64-recovered` and `aarch64-failed-clear` directories.

Product preparation uses the existing reviewed
`conduitos-protocol-source.host.conduit` and
`conduitos-aarch64-protocol-source.host.conduit` profiles, with their explicit
256 MiB preparation arenas. The ordinary 16 MiB published graphical profile
exhausted preparation before class transport on this larger checked graph;
those attempts are not counted as class execution proof. No decoder limit,
controller rule or checker acceptance was widened.

Reproduction enters through `cargo xtask make host build <profile> --output
<product>`, `cargo xtask make conduitos protocol-source`, `protocol-image` and
`protocol-run`. Build the exact recorded runtime revision, use the retained
specializations and Root request, and retain new output directories. Set
`BOT_DEVICE_MODE=read-reentry-phase-error` (or
`read-reentry-phase-error-stall-clear-in`), an independent `BOT_DEVICE_OUTPUT`,
and the reviewed `qemu_bot_wrapper.py` as the emulator. AArch64 additionally
requires explicit QEMU 10.2 and UEFI firmware paths, recorded by the run receipt.

The runtime commit predates this evidence commit; the changed class Source and
native policy are pinned by the recorded package and kernel hashes. Later CI,
integration and stable-publication acceptance remain separately required.

These refreshed runs include the merged dev dependencies in PR #5780. The
prior implementation-revision receipts remain available in the sibling
`bot-phase-error-1e974d91d` directory, with their original identities intact.
