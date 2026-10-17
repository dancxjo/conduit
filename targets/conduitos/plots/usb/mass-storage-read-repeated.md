# Retained read-only BOT demonstration

`mass-storage-read-repeated.conduit` is the exact Source extracted from the
retained `evidence/usb-bot-repeated-read-product-proof.json` package artifact.
The artifact's bytes were rehashed against its retained digest before extraction:
`e3ecbad3add0b585cdd5915dd22878424d8f4f57564bbf711c633e31b5f94be9`.
The historical package also contains specializations; this Source file alone
does not reproduce those package bytes or confer authority.

This preserves the already exercised sixteen serialized read-only READ(10)
transactions for reuse. It pins LUN0/LBA0, 512-byte blocks, QEMU's high-speed
interface profile and tag1, reused only after matching CSW acceptance. The
existing evidence covers x86_64/AArch64 emulator execution at its historical
kernel heads; extraction is not a new check, Plan, Play or current-head run.

Before #5257 closure, this demonstration still needs the reviewed generic
read-only block request/result seam and independent block-resource authority,
pre-class profile admission, complete contradictory/stale completion tests and
truthful stall/reset-recovery/loss/cancellation evidence. No writable command,
filesystem interpretation or retry of an uncertain write is authorized here.
