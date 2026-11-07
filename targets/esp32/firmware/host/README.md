# ESP32 shared Host construction

The complete shared execution path passes optimized Xtensa type checking and
linking. Ordinary Make can qualify its application artifact; physical sensing,
display and carrier-interruption acceptance remain separate hardware checks.

The checked Blue construction owns the actual DHT11 GPIO, TM1637 GPIO, exclusive
UART carrier and finite monotonic timer slots. Its boot path verifies the
immutable Seed against the physical eFuse Host identity and original Make IMAGE.
Shared Root handles authenticated admission, retained return and preparation.
Source-derived software, resident protocol definitions, native offer images and
ordinary lowering layouts are generated from the captured workset.

The entry now calls the shared Source-selected sampling/Mask/output factory,
whole-workset admission, authenticated Start, native runners and Mask session.
These composition templates are shared with RP2040; only actual physical owners,
identity, entropy and carrier adapters are board-local. They specialize this
checked three-fragment Source profile, rather than claiming arbitrary Plot support.

The checked 96KiB heap occupies only HAL's secondary internal DRAM after its
reserved ROM data and stacks. Linker assertions check both physical regions,
exclusive heap occupancy, the checked total static limit and a primary stack
reserve of at least 16KiB. No PSRAM or additional arena is assumed.

Physical behavior still needs hardware qualification. Earlier electrical/Root
checks alone do not establish admitted sampling and display behavior.

Provisioning must first preserve the existing complete flash, credentials and
history and verify that the reserved bootstrap sector does not overlap live
retained state. The fabrication adapter emits an application-only image and a
separate private Seed carrier; it never writes flash itself.
