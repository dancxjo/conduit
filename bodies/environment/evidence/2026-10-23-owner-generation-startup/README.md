# Owner-generation image startup refusal

The supported owner-run proof flashed the clean source/image recorded in
`image-receipt.json`, but Root never produced an admission offer. The bounded
startup capture contains a ProCpu stack-guard panic during canonical type-node
decoding. No owner proposal, Body start or observation run occurred.

The failing ELF has a 28,684-byte CPU stack span (`_stack_end_cpu0=0x3ffd8ff4`,
`_stack_start_cpu0=0x3ffe0000`) and a 3,728-byte native entry poll frame. The
previous physically accepted retirement image had a 28,812-byte span. Its
success does not verify the larger retained native identity introduced later.
The linker still requires at least 28,672 bytes; the runtime guard was not
weakened. This negative capture is retained separately from any later repair.
