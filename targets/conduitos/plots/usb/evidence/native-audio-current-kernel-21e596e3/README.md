Fresh reconciled x86_64 kernel Audio proof
========================================

The unchanged checked package and Root completed 72 packets containing exactly
13,824 PCM bytes. The independent public decoder checked terminal phase 3,
original PCM retention and retirement; verify_wire.py checked all wire bytes.
The shared check_play_receipts.py checked matching Body/Plan/Play identities,
zero allocation/reallocation attempts and acknowledged Stop.

Kernel source is the local reconciled commit recorded in build.json. This is
external-device emulator evidence; audible or physical output is unverified.
The image packaging receipt predates execution; receipt.json records execution.

Recheck wire bytes with:

    python3 verify_wire.py wire.jsonl original-pcm.bin

Recheck lifecycle with the repository proof/check_play_receipts.py on serial.log.
validation.json binds all retained files with SHA-256.
