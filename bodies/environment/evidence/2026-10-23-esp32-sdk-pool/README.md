# ESP32 native SDK pool evidence

The replacement ESP32-WROOM revision 3.1/4 MB ran the native Root image identified
in the receipt. Its total heap backing remains 184 KiB: 64 KiB for Root and a
separate 120 KiB for the SDK C allocation boundary. Both use the pinned
`esp-alloc` implementation; SDK allocation has no fallback into Root storage.

Boot assertions exercise allocation, calloc zeroing, prefix-preserving realloc,
overflow refusal, arena exhaustion and freeing. They check that live SDK
allocation leaves Root usage unchanged, and that all test allocations are
released before RTOS/radio startup. A boot failure cannot reach the UART offer
loop.

Four complete offers followed actual DHCP/HTTP service readiness. All reported
33,408 SDK bytes used out of 122,880, and 5,200 Root bytes used out of 65,536.
Body and native Plan installation remained false. No credentials were submitted
and no join operation was invoked. The prior shared-heap image and its private
submission proof remain separate evidence.

The temperature/RH image was restored afterward, retiring this diagnostic Boot.
The task's `environment-sdk-private-heap` outputs retain the ELF, source patch,
allocator source, offers and serial/flash/restoration logs. Runtime image
attestation remains null. This proves physical allocation-boundary checks and
startup, not full selected-provider admission, station joining, fallback, mDNS,
Host/Body construction, browser recording or overnight endurance.

The public build entrance is `cargo xtask prove environment root-esp32`.
