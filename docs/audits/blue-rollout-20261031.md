# Blue ESP32 rollout, 31 October 2026

This is a bounded physical rollout receipt for Blue on Victus, using shared
integration source `ac3ac678ea5b3ab649eebe92235d54113a63f2f1`. It does not establish
release acceptance or results for the other instruments or Raspberry Pis.
The [machine-readable receipt](blue-rollout-20261031.json) contains the public
artifact hashes and measured bounds. Raw flash, credentials, signing material,
checkpoint payloads, observation records, and local recovery tools remain private.

## Construction and preservation

The supported `cargo xtask prove environment flat-network-appliance-image`
entrance completed with Blue's retained construction birth Sign and original
owner public key. The retained biography matched its original Body. Runtime
packaging and linked local-planner checks passed. The application is 3,245,088
bytes in the existing 3,371,008-byte application partition; the linked CPU stack
reservation is 37,412 bytes against the unchanged 36,864-byte floor. A linked
reservation is not a maximum call-chain stack proof.

A fresh 4 MiB backup authenticated the original Body and Root before writing.
The current production observation-profile decoder authenticated the newest
checkpoint and its Clean cursor. That cursor was byte-equal to the previously
independently accepted retained cursor. The actual next-Boot checkpoint path
passed against a copied NOR region: the immediately pre-write snapshot was
generation 889 at 29,115 encoded bytes; copied successor writes reached 30,376
bytes within the 32,608-byte payload capacity, leaving 2,232 bytes.

Only the application partition was written. A complete 4 MiB readback before
normal boot verified the exact new application and byte equality of every
non-application byte, including credentials, observation epochs, network
recovery evidence, and Body state. Neither data partitions nor the original
Body were replaced. The previous physically working application was retained
privately for conditional application-only rollback; rollback must validate
the then-current checkpoint rather than rewind data or observation history.

## Physical execution

The new application resumed the same original Body, reported restored checkpoint
generation 891, and reconnected to saved Wi-Fi. During a 180-second serial
capture it produced two fresh retained observation records, advanced 50 Mask
acknowledgements, and reported a fresh Face. Sampler and Mask fault fields were
clear; no fatal marker was observed. Two live checkpoint timing events reported
success. Controller acknowledgements do not independently establish visible
physical display output.

Three observation-custody requests were refused with `observed-offer-not-frozen`.
No custody ACK, erase, or offload was performed by this recovery lane. This
receipt does not establish a new observer return, invitation, or membership
transaction. Forebrain retained ownership of shared admission and recorder work.
White, the Pico devices, and their hardware profiles were not changed here.

## Limits and ownership

The existing 26,000-byte policy remains unchanged. It gates optional
capacity-returned renewal/interruption, not startup or installation. Passing
one bounded startup does not establish long-duration renewal. The pre-write
copied successor margin was narrow; the encoded extent after the live startup
checkpoint writes was not reread. No second forced restart was performed.
Further restart and owner transactions require fresh capacity qualification;
checkpoint growth needs the shared biography-custody path, not a raised limit.

The original pin, credentials, buffered records, and qualified artifacts remain
in private custody. The UART lease was released to forebrain after the capture
terminated. No shared runtime, admission, protocol, or hardware-profile source
was changed by this device rollout. See the [environmental Body](../../bodies/environment/README.md)
and [documentation proof policy](../documentation-policy.md) for the distinction
between this dated evidence and general current capability.
