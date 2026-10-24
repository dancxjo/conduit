# Reserve stack for actual first-class unit validation

Separating the native synchronous advance and observer frames preserved CPU
conformance but did not prevent the physical stack guard. Actual preparation,
Start-held and Release passed again; no publication was produced. The new
trace ends in UnitDefinition::encode/seal/new/decode. The finite physical
repeat therefore contradicts sufficiency of the prior frame-only correction.
The wrapper restored the bench and verified twelve fresh acknowledged display
updates across sequences 1–3. Full captures are outside Cargo targets under
outputs/environment-native-face-probe/1792840414.

The next profile reserves a 36 KiB stack instead of treating the 28 KiB Root
floor as sufficient for Face/unit work. Root's 112 KiB and SDK's 72 KiB heap
totals remain unchanged: 8 KiB of CPU0-only Root backing moves to unused
RTC-fast data RAM, and raw publication scratch moves to RTC-slow data RAM.
Boot verifies byte access throughout the RTC heap backing before admission;
SDK/DMA allocation stays in its original disjoint arena. This awake single-core
profile does not promise RTC retention, deep-sleep support or APP-CPU access.
The enlarged stack still requires actual sensor/Face execution proof.

Espressif documents single-core RTC-fast heap usage and its non-DMA/PRO-CPU
restrictions in https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/memory-types.html#rtc-fast-memory .
The pinned esp-hal ESP32 linker declares both RTC regions as 8 KiB; its linker
bounds remain enforced. No semantic unit validation is bypassed.
