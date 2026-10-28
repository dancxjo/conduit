# Victus thermometer recovery

## Ownership and initial inventory, 28 October 2026

Victus owns the attached device ports and recovery. Forebrain owns White and
shared runtime, admission and protocol changes. This branch changes no White
profile or shared implementation. The user identifies the ESP32 as Blue and
the Pico as Yellow.

| Device | USB identity | Hardware/profile evidence | Initial network state |
|---|---|---|---|
| Blue | CP2102 `10c4:ea60`, serial `0001`, `ttyUSB0`; ESP32 revision 3.1, MAC `b0:a7:32:28:61:78` | Handoff: DHT11 GPIO27; TM1637 CLK18/DIO19, VCC 3.3 V | Maintenance image; normal saved-network recovery unverified |
| Yellow | `2e8a:000a`, serial `conduit-environment-usb`, `ttyACM0` | Live DHT11/TM1637 status: CLK7/DIO6, 50 us half-period, weak pull-ups; authored construction has DHT DATA3 | Radio ready, link down, no IP, attachment generation 0, peer not ready |

Yellow reports build
`c9c95121dd3f4c6c09b3db3238ba073fe05b697a6279875db213a915247805ac`
and configuration
`sha256:5ac1c36771db2cd151fc9c8985463026ed88ec4c1633e457d91bacd732dece9c`.
It explicitly reports `body_member=false`. Across separate USB status sessions,
with no client holding the port between them, samples advanced from 50 to 60
and acknowledged frames from 726 to 858. This is diagnostic local continuation,
not physical USB power removal, persistent history, or Body admission proof.

## Blue preservation and post-offload qualification

Before any flash write, the complete 4,194,304-byte flash was read with no
normal reset, retained privately and synchronized to disk. Its SHA-256 is
`ec6d76e006ef125dcc7cfc7e7b3aac0aede3e160a46ac47ff0e3025a28a823db`.
Both committed checkpoint digests verify: bank 0 is generation 631 with
30,360 encoded bytes; bank 1 is generation 632 with 4,609 bytes.

The production `private_preflight::private_pinned_body_has_exact_clean_network_cursor`
test ran exactly once against the fresh physical snapshot and independent
Root/Body pin and original Pending receipt. It passed, loading schema
`conduit.environment/root-restart@4`, generation 632 and 233 biography archives,
and authenticating the exact original Clean cursor. The observation/network
bytes and original dense bank exactly match the pre-offload backup. The strict
observation-marker check verifies all five latest markers as Reclaimed; the
network recovery region is blank.

All 260 files in forebrain's private recovery manifest passed size and SHA-256
checks. Raw records, pins, credentials, logs and backups remain private.
The existing global checkpoint-size guard is unchanged. No maintenance rerun,
fresh Body, Root-authority transfer to Yellow or erase-all was performed.

## Remaining acceptance

The selected schema-4 normal candidate is the existing source `0ac6b9a6c`, ELF
SHA-256 `aa3c2636850b83dad716b0d88afefc42587b1000e49c68f4310be369e8e49c1f`.
It is a physically unaccepted candidate, not current-dev firmware acceptance.
Its packaged app is 3,364,736 bytes within the existing 3,371,008-byte app slot.
Only that application range was written at `0x10000`. Complete post-write
4 MiB readback matched the application exactly. Bytes before `0x10000` and
from `0x347000` to flash end were identical to the original backup, including
credentials, evidence, observations, network journal and Body banks. Readback
SHA-256: `e5005b129ba5d8ab798978150c8ab4f1794ef7a0bf217d4cc667c4c0c4f183b3`.

The first normal Boot reported the same independently pinned Body and admitted
member, `restart_resumed=true`, checkpoint generation 634. With the recorder
stopped, it committed one observation by 85,785 ms and consumed 39 Mask Frames
by 131,032 ms, with no reported sampler or Mask failure.

Reopening pyserial unexpectedly reset the CP2102-connected board despite
preselected false DTR/RTS values. That is a discovered management-interface
hazard, not an intentional power-cut proof. The captured next Boot resumed
the same Body/member at generation 639. A direct file-descriptor capture
subsequently disabled HUPCL and made no modem-line calls. By 151,876 ms this
Boot had two committed observations and 46 consumed Mask Frames, no reported
sampler/Mask failures, and a fresh Face. A new Boot/continuation is proved;
recovered observation export/recorder acceptance is not.
By 197,073 ms, those counters reached three committed observations and 61 Mask
consumptions, still without reported sampler/Mask failures.

Blue did **not** regain saved Wi-Fi. Repeated attempts reached
`retry-backoff`, then `network-evidence-capacity` by 151,876 ms in the second
Boot, despite the initially blank network region. Station address remained
absent. This is a physical failure of the selected candidate, not a passing
reconnect run. It was handed to forebrain's shared-code owner.

A fresh complete post-test flash backup was retained and synchronized before
resuming normal firmware. SHA-256:
`f062e76f951005d366392e2864db68ba1d975fab440ad7ea31c705bfa8d3410e`.
Its checkpoint banks are generations 641/642, 8,291/8,602 bytes. The production
private Clean-cursor test again ran exactly once and passed for generation 642,
237 biography archives and the same independently pinned Root/Body and original
Clean cursor. All eight network pages pass NCE1 commit/CRC checks; each contains
17 events, two Host Calls requested/completed and one BackFailed event. This
inspection does not recover the underlying network failure reason or authorize
erasure. New observations and failure journals remain preserved in the backup.

Normal firmware was resumed after this backup. At 92,524 ms it again had one
committed observation, a fresh Face, 26 Mask consumptions and no reported
sampler/Mask failure; network evidence capacity remained refused. The user
initially saw `----` during startup, then explicitly confirmed alternating
values after the fresh observation. That is attended visible-output evidence
for this short run, not sustained electrical qualification. The final capture
closed with serial HUPCL disabled and no further requested reset; Blue was left
in normal local execution, and Yellow retained its Wi-Fi attachment.

The preserved credential journal has one valid revision-1 CNJ1 profile. Its
SSID appears in victus's cached scan. Using those preserved credentials and
the existing carrier protocol, Yellow actually attached to the saved network:
`link_up=true`, attachment generation 2, no radio error. During attachment its
diagnostic counters reached 71 samples and 1,019 acknowledged Frames, status
`ok`. This establishes host-requested attachment, not autonomous credential
persistence or Body membership. Yellow's firmware was not replaced.
After the USB client closed and reopened, Yellow still reported the same
attachment generation and address with `link_up=true`. No RF outage, power
removal or new-Boot credential persistence is inferred from that observation.

Authenticated observation recovery, selected-recorder acceptance, successful
Blue reconnect and sustained outage/restart acceptance remain unfinished.

Blue's recurring display wire failure remains unresolved. No supply or wiring
change was made. Direct 5 V conversion is not qualified; module pull-ups and
signal-level compatibility require physical inspection.

Yellow still needs preserved firmware/state, saved-network recovery, finite
durable observations and ordinary Host admission/assigned-Plot execution.
Transport attachment and diagnostic counters do not establish those contracts.
