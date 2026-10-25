.section .text.jetson_nano_header,"ax"
.global conduitos_jetson_nano_image_header
conduitos_jetson_nano_image_header:
    b conduitos_jetson_nano_boot
    nop
    .quad 0x70080000
    .quad __conduitos_image_size
    .quad 2
    .quad 0
    .quad 0
    .quad 0
    .byte 0x41, 0x52, 0x4d, 0x64
    .word 0
conduitos_jetson_nano_boot:
    // No stack/BSS writes until the boot core and supported entry state pass.
    cbnz x1, conduitos_jetson_nano_park
    cbnz x2, conduitos_jetson_nano_park
    cbnz x3, conduitos_jetson_nano_park
    mov x20, x0
    mrs x4, mpidr_el1
    ubfx x5, x4, #0, #24
    cbnz x5, conduitos_jetson_nano_park
    ubfx x5, x4, #32, #8
    cbnz x5, conduitos_jetson_nano_park
    mrs x4, CurrentEL
    cmp x4, #8
    b.eq conduitos_jetson_nano_el2
    cmp x4, #4
    b.ne conduitos_jetson_nano_park
    b conduitos_jetson_nano_el1
conduitos_jetson_nano_el2:
    mrs x4, sctlr_el2
    tst x4, #1
    b.ne conduitos_jetson_nano_park
    tst x4, #4
    b.ne conduitos_jetson_nano_park
    tst x4, #0x2000000
    b.ne conduitos_jetson_nano_park
    msr daifset, #0xf
    mov x4, #0x80000000
    msr hcr_el2, x4
    // Cortex-A57 CPTR_EL2 bits [13:12] and [9:0] are RES1.
    mov x4, #0x33ff
    msr cptr_el2, x4
    mov x4, #3
    msr cnthctl_el2, x4
    msr cntvoff_el2, xzr
    mov x4, #0x800
    movk x4, #0x30d0, lsl #16
    msr sctlr_el1, x4
    mov x4, #0x3c5
    msr spsr_el2, x4
    adr x4, conduitos_jetson_nano_el1
    msr elr_el2, x4
    isb
    eret
conduitos_jetson_nano_el1:
    mrs x4, sctlr_el1
    tst x4, #1
    b.ne conduitos_jetson_nano_park
    tst x4, #4
    b.ne conduitos_jetson_nano_park
    tst x4, #0x3000000
    b.ne conduitos_jetson_nano_park
    msr daifset, #0xf
    msr spsel, #1
    // AArch64 firmware DTB contract: readable aligned blob in initialized RAM.
    tst x20, #7
    b.ne conduitos_jetson_nano_park
    mov x4, #0x80000000
    cmp x20, x4
    b.lo conduitos_jetson_nano_park
    mov x4, #0x180000000
    sub x4, x4, #40
    cmp x20, x4
    b.hi conduitos_jetson_nano_park
    ldr w5, [x20, #4]
    rev w5, w5
    cmp w5, #40
    b.lo conduitos_jetson_nano_park
    mov x4, #0x200000
    cmp x5, x4
    b.hi conduitos_jetson_nano_park
    add x5, x20, x5
    mov x4, #0x180000000
    cmp x5, x4
    b.hi conduitos_jetson_nano_park
    adrp x6, __conduitos_image_start
    add x6, x6, :lo12:__conduitos_image_start
    adrp x7, __conduitos_image_end
    add x7, x7, :lo12:__conduitos_image_end
    cmp x20, x7
    b.hs 1f
    cmp x5, x6
    b.hi conduitos_jetson_nano_park
1:
    adrp x4, __conduitos_boot_stack_top
    add x4, x4, :lo12:__conduitos_boot_stack_top
    mov sp, x4
    adrp x4, __conduitos_bss_start
    add x4, x4, :lo12:__conduitos_bss_start
    adrp x5, __conduitos_bss_end
    add x5, x5, :lo12:__conduitos_bss_end
2:
    cmp x4, x5
    b.hs 3f
    stp xzr, xzr, [x4], #16
    b 2b
3:
    mrs x4, cpacr_el1
    orr x4, x4, #0x300000
    msr cpacr_el1, x4
    isb
    mov x0, x20
    bl conduitos_aarch64_jetson_nano_start
conduitos_jetson_nano_park:
    wfe
    b conduitos_jetson_nano_park
