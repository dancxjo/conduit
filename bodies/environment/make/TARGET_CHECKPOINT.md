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

## Flash-backed Text experiment, 2026-10-25

A second physical run tested one substantial representation change: explicit
immutable configuration Text borrowed from linked flash, through the existing
Core offer, checked Source, and planner interfaces. Owned and borrowed Text
retained identical meaning, debug output, JSON and postcard bytes; ordinary
wire decoding stayed owned. Two representation tests and four existing verified
metadata tests passed, and the environment image builder compiled. These checks
were prerequisites to the hardware experiment, not delivery evidence.

The same ESP32, Body, initialized Root/radio, native providers, 118,768-byte Root
heap, 73,728-byte SDK heap and 41,268-byte stack reservation were retained.

| Target phase | Root used | Root free | Result |
| --- | ---: | ---: | --- |
| Initialized Root, Body and radio | 73,956 | 44,812 | Same baseline |
| Five native offers | 81,600 | 37,168 | Same baseline |
| All thirteen software offers | 106,764 | 12,004 | First blocker removed |
| All fifteen individually decoded Source Gears, then dropped | 106,764 | 12,004 | All passed |
| Indexed selection | — | — | Allocation of 1,120 bytes failed |

The last reported Root peak was 109,676 bytes; it is not the final failing peak
or a complete appliance bound. Before selection the free bank extents were
`[0, 3812, 8192]`. The panic backtrace identifies serde sequence growth during
selection; it does not identify the precise failing collection. Selection,
planning, execution and a physical native sample remain unproved. No native
sample reached the display.

The experiment was set aside rather than published as a partial Core Text API
migration. Its source patch and new source files are retained outside Git in
`~/.cache/conduit/borrowed-text-target-experiment.patch` and
`borrowed-text-target-experiment-new-source.tar`; the local stash is named
`borrowed-text-target-experiment-20261025`. Do not silently reapply it or promote
its component tests to appliance delivery. The hardware decision is now explicit:
retain this exact MCU and specialize preparation further, or use more RAM.

Experimental ELF SHA-256:
`9fd382c7a9cfbde2043cb20d3a5cbd8023510c02b0c8ad28f6e551a503006692`.
Raw capture SHA-256:
`53b385944ccc9caa024cf03d85fed07826ea4c205d044675ea06f636c4fbed5a`.
The full bench image was restored and independently rechecked against device
MD5 `4f8db50b72e16cfc70e19e41bdeeabc5`.

## Exact outer offer reservation, 2026-10-25

A third physical run added exact reservation for the thirteen software offers
before appending them to the five native offers. This saved 3,024 retained bytes
against the second run. After all offers and all individual Gear probes, Root
used 103,740 bytes, leaving 15,028, with free bank extents `[0, 6836, 8192]`.
Indexed selection still failed, now requesting 2,240 bytes in serde sequence
growth. The exact collection remains unproved. The last reported peak was
106,652; it is not the failing peak or a complete appliance bound. Neither
planning nor native execution was reached. No native sample reached the display.

The build took 5m44s; stack reservation remained 41,268 bytes. The experiment is
preserved in the local stash `exact-offers-target-experiment-20261025` and the
external `~/.cache/conduit/exact-offers-target-experiment.patch` plus
`exact-offers-target-experiment-new-source.tar`, not shipped as an API migration.
ELF SHA-256:
`7ba4802c0a057b28d1c23a1f04254fd68ba5029a9d1a549ef531bda224970232`.
Capture SHA-256:
`49eae30e96b5c79695246c0b4190800b86a742afd43e098701fb23a43501c023`.
The full recovery image was restored and its device MD5 reverified as
`4f8db50b72e16cfc70e19e41bdeeabc5`. The next decision remains the target's RAM
requirement versus a larger compiler/runtime change, not more component APIs.
