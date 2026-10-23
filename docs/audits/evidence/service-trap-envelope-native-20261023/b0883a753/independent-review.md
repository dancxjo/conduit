# Native trap-envelope independent review

PASS: source `b0883a7531d30d98f7704e29906027c7e7b7bfdf`, actual ELF/ISO/serial SHA joins and all 19 retained file hashes verified. Twenty exact raw rows match sidecar; flags preserved in all20; preempt terminal interval is null. All existing protection validators remain byte-identical and original collector predicates remain present. Actual26 ring3 hostile entries,8MiB retained+8MiB scratch restart-zero, x87#NM/Root+sibling survival, timer and compositor fixtures passed.

Raw TSC samples below are trial0/1/2, with no calibration or subtraction. Root restoration overlaps terminal return; do not add intervals.

| Case | Mode | Entry-state | Terminal-return | Root-restore | Entry-control | Return-control |
|---|---|---|---|---|---|---|
| noop | fresh | 4040/9744/6918 | 16764/28250/27480 | 16424/27508/26608 | 142/182/298 | 66/320/64 |
| noop | reused | 7570/3972/2418 | 26852/20772/19712 | 25794/20522/19464 | 322/122/62 | 316/108/104 |
| 8-byte-text | fresh | 6074/7858/7046 | 28798/30490/30960 | 28170/29608/30512 | 288/410/542 | 324/190/316 |
| 8-byte-text | reused | 7604/4906/2098 | 28022/21062/20172 | 27566/20840/19914 | 112/344/68 | 66/76/68 |
| 256-byte-text | fresh | 6588/7110/7234 | 27478/31114/29966 | 27092/29756/29644 | 340/408/416 | 96/452/94 |
| 256-byte-text | reused | 8974/5236/2140 | 28968/21852/19986 | 28432/21250/19770 | 334/280/70 | 64/202/76 |
| timer-preempt | fresh | 6506 | null | 38320 | 164 | 64 |
| privileged-fault | fresh | 7254 | 27740 | 27308 | 188 | 88 |

These are proof-only software envelopes. Hardware delivery, pure IRET, full IRQ preservation and physical latency are excluded. Clock overhead remains included; controls are not subtracted. Fresh/reused samples share one fixed-order emulator process. No normal-product timing or full #6183 completion is inferred. Optimization delivery, accepted-source correspondence and broader acceptance remain separate.
