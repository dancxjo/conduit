# USB architecture acceptance evidence (#5258)

The same checked BOT Source package `e09a9f5aa14c599d85fb7a5ea247c91c0aab5d2d3045efca55bd3f08c003134d` ran unchanged against real emulated xHCI and QEMU storage on x86_64 q35 and AArch64 virt-10.2. Both runs accepted the patterned 512-byte LBA0 block only after a matching successful CSW and retired Play. No Rust storage class parser was added.

| Architecture | Product USB evidence | Remaining disposition |
| --- | --- | --- |
| x86_64 q35 | BOT emulator with device; immutable receipt retained | Full class/fault acceptance incomplete |
| AArch64 virt-10.2, neoverse-n2 | Same BOT package, emulator with device; immutable receipt retained | Full class/fault acceptance incomplete |
| IA-32 PC | Unsupported | No admitted native product USB provider |
| RISC-V64 virt | Unsupported | No admitted native product USB provider |
| LoongArch64 virt | Unsupported | No admitted native product USB provider |

`architecture-matrix.json` gives explicit class dispositions. Receipts retain build/media/Source identities, complete serial observations including Boot/Plan/Play and admitted Base/resource identity, firmware/emulator hashes, decoder source, and the unchanged-package comparison. Paths identify original local artifacts; embedded content and hashes preserve the proof independently. Physical/HIL compatibility remains unverified. Compilation is never substituted for execution. These partial rows do not close #5253–#5258.

The unchanged repeated BOT package `e3ecbad3add0b585cdd5915dd22878424d8f4f57564bbf711c633e31b5f94be9` also completed sixteen serialized READ(10) transactions on both architectures. Exact ordinals 0..31, matching CSWs, block-after-CSW publication and acknowledged Play retirement are decoded in the additional receipts. No emulator allocation counter claim is made.
