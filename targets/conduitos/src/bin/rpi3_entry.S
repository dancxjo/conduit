.section .text.entry,"ax"
.global conduitos_rpi3_product_entry
conduitos_rpi3_product_entry:
    msr daifset, #0xf
    // Secondary cores must never clear BSS or share Root's stack.
    mrs x0, mpidr_el1
    and x1, x0, #0xffffff
    cbnz x1, conduitos_rpi3_park
    ubfx x1, x0, #32, #8
    cbnz x1, conduitos_rpi3_park
    mrs x0, CurrentEL
    cmp x0, #8
    b.eq conduitos_rpi3_el2
    cmp x0, #4
    b.ne conduitos_rpi3_park
    b conduitos_rpi3_el1
conduitos_rpi3_el2:
    mrs x0, sctlr_el2
    tst x0, #1
    b.ne conduitos_rpi3_park
    tst x0, #4
    b.ne conduitos_rpi3_park
    mov x0, #0x80000000
    msr hcr_el2, x0
    mov x0, #0x33ff
    msr cptr_el2, x0
    mov x0, #3
    msr cnthctl_el2, x0
    msr cntvoff_el2, xzr
    mov x0, #0x800
    movk x0, #0x30d0, lsl #16
    msr sctlr_el1, x0
    mov x0, #0x3c5
    msr spsr_el2, x0
    adr x0, conduitos_rpi3_el1
    msr elr_el2, x0
    isb
    eret
conduitos_rpi3_el1:
    // The firmware stub must supply translation/cache-disabled EL1 entry.
    mrs x0, sctlr_el1
    tst x0, #1
    b.ne conduitos_rpi3_park
    tst x0, #4
    b.ne conduitos_rpi3_park
    mov x0, #0x800
    movk x0, #0x30d0, lsl #16
    msr sctlr_el1, x0
    isb
    ic iallu
    dsb sy
    isb
    msr spsel, #1
    ldr x0, =__bss_start
    ldr x1, =__bss_end
1:
    cmp x0, x1
    b.hs 2f
    str xzr, [x0], #8
    b 1b
2:
    ldr x0, =__conduitos_boot_stack_end
    mov sp, x0
    bl conduitos_raspberry_pi_product_start
conduitos_rpi3_park:
    wfe
    b conduitos_rpi3_park
.section .bss.boot_stack,"aw",%nobits
.balign 16
.space 4194304
__conduitos_boot_stack_end:
