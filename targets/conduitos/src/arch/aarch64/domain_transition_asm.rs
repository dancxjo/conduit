//! AArch64 register frames and exception vectors for the EL0 boundary.
core::arch::global_asm!(
    r#"
.arch_extension pauth
.section .bss.conduitos_aarch64_domain_context,"aw",%nobits
.balign 16
conduitos_aarch64_domain_context:
    .zero 192
.section .text.conduitos_aarch64_domain,"ax",%progbits
.macro SAVE_FP base
    stp q0, q1, [\base, #0]
    stp q2, q3, [\base, #32]
    stp q4, q5, [\base, #64]
    stp q6, q7, [\base, #96]
    stp q8, q9, [\base, #128]
    stp q10, q11, [\base, #160]
    stp q12, q13, [\base, #192]
    stp q14, q15, [\base, #224]
    stp q16, q17, [\base, #256]
    stp q18, q19, [\base, #288]
    stp q20, q21, [\base, #320]
    stp q22, q23, [\base, #352]
    stp q24, q25, [\base, #384]
    stp q26, q27, [\base, #416]
    stp q28, q29, [\base, #448]
    stp q30, q31, [\base, #480]
    mrs x9, fpcr
    mrs x10, fpsr
    str x9, [\base, #512]
    str x10, [\base, #520]
.endm
.macro LOAD_FP base
    ldp q0, q1, [\base, #0]
    ldp q2, q3, [\base, #32]
    ldp q4, q5, [\base, #64]
    ldp q6, q7, [\base, #96]
    ldp q8, q9, [\base, #128]
    ldp q10, q11, [\base, #160]
    ldp q12, q13, [\base, #192]
    ldp q14, q15, [\base, #224]
    ldp q16, q17, [\base, #256]
    ldp q18, q19, [\base, #288]
    ldp q20, q21, [\base, #320]
    ldp q22, q23, [\base, #352]
    ldp q24, q25, [\base, #384]
    ldp q26, q27, [\base, #416]
    ldp q28, q29, [\base, #448]
    ldp q30, q31, [\base, #480]
    ldr x9, [\base, #512]
    ldr x10, [\base, #520]
    msr fpcr, x9
    msr fpsr, x10
.endm
.macro SAVE_KEY low, high, offset
    mrs x9, \low
    mrs x10, \high
    stp x9, x10, [x16, #\offset]
    msr \low, xzr
    msr \high, xzr
.endm
.macro VERIFY_FP base, failure
    // Observe the restored registers before any Rust code can alter them.
    sub sp, sp, #528
    mov x21, sp
    SAVE_FP x21
    mov x11, xzr
1:
    ldr x9, [\base, x11]
    ldr x10, [x21, x11]
    cmp x9, x10
    b.ne 2f
    add x11, x11, #8
    cmp x11, #528
    b.lo 1b
    add sp, sp, #528
    b 3f
2:
    add sp, sp, #528
    b \failure
3:
.endm
.macro LOAD_KEY low, high, offset
    ldp x9, x10, [x16, #\offset]
    msr \low, x9
    msr \high, x10
.endm
.global conduitos_aarch64_domain_enter
conduitos_aarch64_domain_enter:
    stp x19, x20, [sp, #-96]!
    stp x21, x22, [sp, #16]
    stp x23, x24, [sp, #32]
    stp x25, x26, [sp, #48]
    stp x27, x28, [sp, #64]
    stp x29, x30, [sp, #80]
    adrp x16, conduitos_aarch64_domain_context
    add x16, x16, :lo12:conduitos_aarch64_domain_context
    mov x17, x0
    mov x9, sp
    str x9, [x16]
    mrs x9, ttbr0_el1
    str x9, [x16, #8]
    mrs x9, ttbr1_el1
    str x9, [x16, #16]
    mrs x9, tcr_el1
    str x9, [x16, #24]
    mrs x9, cpacr_el1
    str x9, [x16, #32]
    mrs x9, sctlr_el1
    str x9, [x16, #40]
    mrs x9, cntkctl_el1
    str x9, [x16, #48]
    mrs x9, tpidr_el0
    str x9, [x16, #56]
    mrs x9, tpidrro_el0
    str x9, [x16, #64]
    ldp x19, x20, [x17, #48]
    stp x19, x20, [x16, #72]
    SAVE_FP x19
    ldr x11, [x17, #64]
    str x11, [x16, #88]
    tbz x11, #0, 1f
    SAVE_KEY APIAKeyLo_EL1, APIAKeyHi_EL1, 96
    SAVE_KEY APIBKeyLo_EL1, APIBKeyHi_EL1, 112
    SAVE_KEY APDAKeyLo_EL1, APDAKeyHi_EL1, 128
    SAVE_KEY APDBKeyLo_EL1, APDBKeyHi_EL1, 144
    SAVE_KEY APGAKeyLo_EL1, APGAKeyHi_EL1, 160
1:
    tbz x11, #1, 2f
    mrs x9, pmuserenr_el0
    str x9, [x16, #176]
    msr pmuserenr_el0, xzr
2:
    msr cntkctl_el1, xzr
    msr tpidr_el0, xzr
    msr tpidrro_el0, xzr
    ldr x9, [x16, #32]
    ldr x10, =0x03330000
    bic x9, x9, x10
    orr x9, x9, #0x300000
    msr cpacr_el1, x9
    ldr x9, [x16, #40]
    ldr x10, =0xcd05e200
    bic x9, x9, x10
    msr sctlr_el1, x9
    isb
    LOAD_FP x20
    ldp x9, x10, [x17, #24]
    ldr x11, [x17, #40]
    dsb ishst
    msr ttbr0_el1, x9
    msr ttbr1_el1, x10
    msr tcr_el1, x11
    isb
    tlbi vmalle1
    dsb ish
    isb
    ldr x9, [x17, #8]
    msr spsel, #1
    msr sp_el0, x9
    ldr x9, [x17]
    msr elr_el1, x9
    msr spsr_el1, xzr
    ldr x0, [x17, #16]
    mov x1, xzr
    mov x2, xzr
    mov x3, xzr
    mov x4, xzr
    mov x5, xzr
    mov x6, xzr
    mov x7, xzr
    mov x8, xzr
    mov x9, xzr
    mov x10, xzr
    mov x11, xzr
    mov x12, xzr
    mov x13, xzr
    mov x14, xzr
    mov x15, xzr
    mov x16, xzr
    mov x17, xzr
    mov x18, xzr
    mov x19, xzr
    mov x20, xzr
    mov x21, xzr
    mov x22, xzr
    mov x23, xzr
    mov x24, xzr
    mov x25, xzr
    mov x26, xzr
    mov x27, xzr
    mov x28, xzr
    mov x29, xzr
    mov x30, xzr
    eret
.balign 2048
.global conduitos_aarch64_domain_vectors
conduitos_aarch64_domain_vectors:
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_irq_entry
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_irq_entry
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_sync
    .balign 128
    b conduitos_aarch64_domain_irq_entry
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
    b conduitos_aarch64_domain_root_fault
    .balign 128
conduitos_aarch64_domain_sync:
    mrs x9, spsr_el1
    tst x9, #15
    b.ne conduitos_aarch64_domain_root_fault
    mrs x9, esr_el1
    lsr x10, x9, #26
    cmp x10, #0x15
    b.ne 3f
    tst x9, #0xffff
    b.ne 4f
    mov x1, xzr
    b conduitos_aarch64_domain_return
4:
    mov x0, xzr
    mov x1, #3
    b conduitos_aarch64_domain_return
3:
    mov x1, #1
    mov x0, #0x106
    cmp x10, #0x20
    b.eq 5f
    cmp x10, #0x24
    b.eq 5f
    cmp x10, #0x18
    b.eq 6f
    cmp x10, #0x3c
    b.eq 6f
    b conduitos_aarch64_domain_return
5:
    mov x0, #0x10e
    b conduitos_aarch64_domain_return
6:
    mov x0, #0x10d
    b conduitos_aarch64_domain_return
conduitos_aarch64_domain_irq_entry:
    sub sp, sp, #800
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]
    mrs x9, elr_el1
    mrs x10, spsr_el1
    str x9, [sp, #784]
    str x10, [sp, #792]
    add x20, sp, #256
    SAVE_FP x20
    mrs x0, spsr_el1
    tst x0, #15
    cset x0, eq
    cbz x0, 7f
    adrp x16, conduitos_aarch64_domain_context
    add x16, x16, :lo12:conduitos_aarch64_domain_context
    ldr x20, [x16, #72]
    LOAD_FP x20
    .if {proof}
    VERIFY_FP x20, conduitos_aarch64_domain_irq_fp_failed
    .endif
7:
    bl conduitos_aarch64_domain_irq
    cbnz x0, 8f
    add x20, sp, #256
    LOAD_FP x20
    ldr x9, [sp, #784]
    ldr x10, [sp, #792]
    msr elr_el1, x9
    msr spsr_el1, x10
    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30, [sp, #240]
    add sp, sp, #800
    eret
8:
    mov x0, #4
    mov x1, #2
    b conduitos_aarch64_domain_return
conduitos_aarch64_domain_irq_fp_failed:
    mov x0, #3
    mov x1, #3
conduitos_aarch64_domain_return:
    msr daifset, #15
    mov x14, x0
    mov x15, x1
    adrp x16, conduitos_aarch64_domain_context
    add x16, x16, :lo12:conduitos_aarch64_domain_context
    ldr x20, [x16, #72]
    LOAD_FP x20
    .if {proof}
    VERIFY_FP x20, conduitos_aarch64_domain_fp_failed
    .endif
conduitos_aarch64_domain_restore_machine:
    ldr x11, [x16, #88]
    tbz x11, #0, 9f
    LOAD_KEY APIAKeyLo_EL1, APIAKeyHi_EL1, 96
    LOAD_KEY APIBKeyLo_EL1, APIBKeyHi_EL1, 112
    LOAD_KEY APDAKeyLo_EL1, APDAKeyHi_EL1, 128
    LOAD_KEY APDBKeyLo_EL1, APDBKeyHi_EL1, 144
    LOAD_KEY APGAKeyLo_EL1, APGAKeyHi_EL1, 160
9:
    tbz x11, #1, 10f
    ldr x9, [x16, #176]
    msr pmuserenr_el0, x9
10:
    ldr x9, [x16, #32]
    msr cpacr_el1, x9
    ldr x9, [x16, #40]
    msr sctlr_el1, x9
    ldr x9, [x16, #48]
    msr cntkctl_el1, x9
    ldr x9, [x16, #56]
    msr tpidr_el0, x9
    ldr x9, [x16, #64]
    msr tpidrro_el0, x9
    dsb ishst
    ldp x9, x10, [x16, #8]
    ldr x11, [x16, #24]
    msr ttbr0_el1, x9
    msr ttbr1_el1, x10
    msr tcr_el1, x11
    isb
    tlbi vmalle1
    dsb ish
    isb
    ldr x9, [x16]
    msr sp_el0, x9
    msr spsel, #0
    ldp x19, x20, [sp, #0]
    ldp x21, x22, [sp, #16]
    ldp x23, x24, [sp, #32]
    ldp x25, x26, [sp, #48]
    ldp x27, x28, [sp, #64]
    ldp x29, x30, [sp, #80]
    add sp, sp, #96
    mov x0, x14
    mov x1, x15
    ret
conduitos_aarch64_domain_fp_failed:
    mov x14, #3
    mov x15, #3
    b conduitos_aarch64_domain_restore_machine
conduitos_aarch64_domain_root_fault:
    msr daifset, #15
11: wfi
    b 11b
"#,
    proof = const cfg!(feature = "ordinary-domain-proof") as u8,
);
