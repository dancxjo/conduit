# Protected transition assembly extraction evidence

Measured clean source: `65869425ec2bc4b2a0f02d74d337c769a8e69ebd`. The supported ordinary-domain-proof session 10087 passed with actual x86_64 CPL3 execution, preserving the original 20 baseline samples, 14 setup records, five source/ELF/raw-trace-bound fixture receipts, and guest timer/control checks.

This is an extraction-only change: the original and extracted actual global_asm fragments compile to byte-identical 4056-byte x86-none objects (SHA-256 `22d540cc74777472420f9481f38e895f67774f2b0c4a1d49484635c660133426`). The exact original/extracted sources and objects, source manifest, result, first full supported native log, raw trace, five receipts, build/image metadata and identity verification are preserved losslessly here. Actual ELF and ISO remain in `work/6183-transition-native-artifacts/x86_64`, with hashes in the manifest.

The source manifest is a historical preflight snapshot: its running state precedes the retained terminal PASS. This evidence-only descendant changes no measured source object. No tests were repeated for packaging.

Profiles, budgets and deadlines were unchanged. This does not establish independent CR3/trap attribution, calibrated performance, production confinement, or normal responsive-journey speedup. No PR, Candidate, Integration or issue closure is claimed.
