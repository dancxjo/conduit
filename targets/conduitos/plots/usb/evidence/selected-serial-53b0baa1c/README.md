# Selected CDC serial product observation

Both product kernels were built from clean commit `53b0baa1ced0ce52bae39513e0caf91df3f3c394`. Both execute the same selected checked `serial/duplex-bytes` Back and package SHA `d1621050418b243909457672c65c587246b6b526b104b5b23c1674622839da38`.

Each run returns 32 ordered canonical serial results: 16 transmitted 12-byte buffers and 16 received 17-byte buffers (64 requested, short=true), then acknowledges retirement. Independent USBredir wire validation checks successful configuration, exact 115200 8N1 line coding, five Source controls, ordered endpoints and exact transmitted/received octets. Firmware enumeration is checked separately by architecture.

The receipts, canonical serial logs, independent device wire logs, exact package/root and verifier sources are retained here. The initial failed checker receipts remain in local scratch; subsequent fresh runs passed after correcting USBredir configuration-message and firmware-enumeration assumptions. The AArch64 transcript also passed the strengthened line-coding verifier; this separate revalidation is recorded explicitly.

This is freestanding emulator evidence, not physical compatibility or full issue acceptance. Unsupported-profile admission before class Play, complete fault/pressure/cancellation matrix, hosted CI and USB Audio/ECM remain open. Native mechanisms do not contain CDC class policy.
