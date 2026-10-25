# ESP32 delivery checkpoint — 2026-10-25

**The current owned-offer preparation path does not fit. No native sample or
physical Show was reached.** Component tests do not establish appliance delivery.

The diagnostic ran on ESP32 rev 3.1, MAC `b0:a7:32:28:61:78`, 4 MiB flash,
using published source `3fbb3845e45bd424b3762167094d672b1036991a` plus the
`target-memory-check` instrumentation. Body Saito Minato
(`54a620076fa6dd741de58a9cca1b0ece07c219a3c4fdbc01af127a8ad523b888`) was
installed under actual Boot `boot/83a83901236ebd099b8a1258989e929b`, membership
revision 2. It remained Lulled; no Plan/Play or producer invitation is claimed.

The Root/SDK partitions and 36,864-byte CPU stack floor were unchanged.
The linked image reserved 41,268 stack bytes. Root retained sampler software and
value storage, native sensor/timer/display providers, installed Body, network
setup and active AP/HTTP service. This is a combined **lower bound**, not the
complete appliance: Mask execution, final kernel and later planning allocations
were not yet live.

| Hardware point | Root used | Root free | Root peak | SDK used / reserved |
| --- | ---: | ---: | ---: | ---: |
| Initialized Root, Body and radio | 73,956 | 44,812 | 89,720 | 33,408 / 73,728 |
| Native advertisement retained | 81,600 | 37,168 | 89,720 | 33,408 / 73,728 |
| First three software offers retained | 84,604 | 34,164 | 89,720 | 33,408 / 73,728 |

Root reserved 118,768 bytes. The fourth software offer (index 3, node 4)
requested one 29,798-byte allocation and panicked. Free bytes by Root bank were
`[0, 25972, 8192]`; no bank could satisfy that allocation. Source Gear decoding,
selection, compact planning, lowering and actual sampling were never reached.

The exact field is
`CapabilityOffer.semantic_contract.configuration[0].default_value.Text`, for
expression kind
`conduitese/pure-expression/8c882c607787cf62f070e120915ce172eb713dad0c5876a0d97e5d846ab7cc0a`.
Symbolization reaches `alloc::slice::to_owned`, serde JSON string decoding and
Core `ConfigurationValue::deserialize`. This is a copy of immutable program
text already linked in flash, not sensor data.

All 13 software offers require **39,398 bytes of owned default Text alone**.
That already exceeds the 37,168 bytes available after native offers by **2,230
bytes**, before other strings, collections, allocator overhead or planning
scratch. Their total JSON string-value payload is 46,820 bytes; that broader
number includes enum spellings and is not asserted as an exact heap bound.
Thus merging free banks alone would not make this catalogue fit.

The next admissible attempt must make a substantial representation change:
keep immutable semantic contracts/programs borrowed from linked flash through
selection/planning, with the same exact identity/hash/configuration checks; or
use hardware with more RAM. Do not increase arena/stack guards or add more
component preparation APIs as a substitute for rerunning this hardware gate.
Removing this first blocker would still not prove the full appliance fits.

Diagnostic ELF SHA-256:
`a88281788edd31ae2aa749014905280157eca73d9c43da11800ffef75c14fa33`.
Raw capture SHA-256:
`8ab31c4b5d89083cb8a1b8a7999045f2310bfb7e959ee21d7ec61fac04d8b730`.
Images and captures remain external to Git. The existing full-flash recovery
was restored and its device MD5 rechecked as
`4f8db50b72e16cfc70e19e41bdeeabc5`. Recovery firmware explicitly reports
`body_installed:false`; its bench reading is not the missing native Body proof.
