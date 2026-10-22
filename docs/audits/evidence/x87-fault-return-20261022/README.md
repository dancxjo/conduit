# x86_64 device-not-available fault return (#6229)

The handler lacked its jump to the common fault path. It pushed a synthetic error word and set vector 7, then fell through the divide-error handler, pushing a second word and replacing the vector with 0. The correction adds the missing jump and changes no grant, timing quantum or execution policy.

The actual CPL3 `fld1` control on clean `2a34102bd0908a9debe239250f6e26aae46b1584` observed origin 1/value 256 (vector 0); the supported proof refused. The corrected handler on `e60280f7409dbe201789d96059484d9531cdd378` observed origin 1/value 263 (vector 7), but the initial control incorrectly required the raw backend to revoke itself. That test-assumption failure remains preserved. Ordinary backend revocation belongs to the existing production `ProtectedRegion` lifecycle owner.

Clean `7be8b471f0818e93213f912c2d8a8b8d334bc3cf` passed the full supported `cargo +stable xtask make conduitos ordinary-domain-proof --locked`. Its control checks vector 7, fault state and refusal of further resume/backend access through `ProtectedRegion`, unchanged Root/sibling samples, and successful sibling execution. The fixture binding grants no effect capabilities and is not a semantic Plan admission claim. The complete ordinary Source, timer, memory and compositor fixture proofs run afterward.

The four positive receipts bind one actual ELF and original serial digest, QEMU q35/max/one CPU/256 MiB. Raw and compressed records, the two negative predecessors, exact source/image identities and locally retained actual ELF/ISO paths are in `manifest.json`. No bytes are normalized or relabeled. Positive evidence remains attributed to its measured source, not the later merge or documentation commit.

The branch was refreshed onto normally merged dev `d72396effb2436b7838b18547e5b48fb30f1393d`, producing `bed4142ee991962a64d34086189c1b39907b901c`. All five changed code/proof files are byte-identical to the executed positive source. On that refreshed clean source, formatting, strict all-target hosted ConduitOS Clippy and 19 focused CPU controls (12 ordinary Plan, two Morse, five memory/layout) passed. Candidate and relevant merged Integration remain separate gates.

This is emulator evidence for this exception return and existing fixtures. It does not establish broad FPU-state equivalence, physical or other-architecture execution, production compositor/microkernel completion, DMA/driver containment or stable publication.
