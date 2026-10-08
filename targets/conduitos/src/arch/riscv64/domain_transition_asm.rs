//! User entry and supervisor-owned trap frames; no user stack is trusted.
core::arch::global_asm!(
    r#"
.option push
.option norelax
.option arch, +d
.section .bss.conduitos_riscv64_domain_context,"aw",@nobits
.balign 16
conduitos_riscv64_domain_context:
    .zero 112
.section .text.conduitos_riscv64_domain,"ax",@progbits
.macro SAVE_FP base
    fsd f0, 0(\base)
    fsd f1, 8(\base)
    fsd f2, 16(\base)
    fsd f3, 24(\base)
    fsd f4, 32(\base)
    fsd f5, 40(\base)
    fsd f6, 48(\base)
    fsd f7, 56(\base)
    fsd f8, 64(\base)
    fsd f9, 72(\base)
    fsd f10, 80(\base)
    fsd f11, 88(\base)
    fsd f12, 96(\base)
    fsd f13, 104(\base)
    fsd f14, 112(\base)
    fsd f15, 120(\base)
    fsd f16, 128(\base)
    fsd f17, 136(\base)
    fsd f18, 144(\base)
    fsd f19, 152(\base)
    fsd f20, 160(\base)
    fsd f21, 168(\base)
    fsd f22, 176(\base)
    fsd f23, 184(\base)
    fsd f24, 192(\base)
    fsd f25, 200(\base)
    fsd f26, 208(\base)
    fsd f27, 216(\base)
    fsd f28, 224(\base)
    fsd f29, 232(\base)
    fsd f30, 240(\base)
    fsd f31, 248(\base)
    frcsr t1
    sd t1, 256(\base)
.endm
.macro LOAD_FP base
    fld f0, 0(\base)
    fld f1, 8(\base)
    fld f2, 16(\base)
    fld f3, 24(\base)
    fld f4, 32(\base)
    fld f5, 40(\base)
    fld f6, 48(\base)
    fld f7, 56(\base)
    fld f8, 64(\base)
    fld f9, 72(\base)
    fld f10, 80(\base)
    fld f11, 88(\base)
    fld f12, 96(\base)
    fld f13, 104(\base)
    fld f14, 112(\base)
    fld f15, 120(\base)
    fld f16, 128(\base)
    fld f17, 136(\base)
    fld f18, 144(\base)
    fld f19, 152(\base)
    fld f20, 160(\base)
    fld f21, 168(\base)
    fld f22, 176(\base)
    fld f23, 184(\base)
    fld f24, 192(\base)
    fld f25, 200(\base)
    fld f26, 208(\base)
    fld f27, 216(\base)
    fld f28, 224(\base)
    fld f29, 232(\base)
    fld f30, 240(\base)
    fld f31, 248(\base)
    ld t1, 256(\base)
    fscsr t1
.endm
.macro VERIFY_FP base, failure
    addi sp, sp, -272
    mv t2, sp
    SAVE_FP t2
    li t3, 0
1:
    add t4, \base, t3
    ld t4, 0(t4)
    add t5, t2, t3
    ld t5, 0(t5)
    bne t4, t5, 2f
    addi t3, t3, 8
    li t4, 264
    bltu t3, t4, 1b
    addi sp, sp, 272
    j 3f
2:
    addi sp, sp, 272
    j \failure
3:
.endm
.global conduitos_riscv64_domain_enter
conduitos_riscv64_domain_enter:
    addi sp, sp, -128
    sd ra, 0(sp)
    sd gp, 8(sp)
    sd tp, 16(sp)
    sd s0, 24(sp)
    sd s1, 32(sp)
    sd s2, 40(sp)
    sd s3, 48(sp)
    sd s4, 56(sp)
    sd s5, 64(sp)
    sd s6, 72(sp)
    sd s7, 80(sp)
    sd s8, 88(sp)
    sd s9, 96(sp)
    sd s10, 104(sp)
    sd s11, 112(sp)
    la t6, conduitos_riscv64_domain_context
    sd sp, 0(t6)
    csrr t0, satp
    sd t0, 8(t6)
    csrr t0, stvec
    sd t0, 16(t6)
    csrr t0, sscratch
    sd t0, 24(t6)
    csrr t0, sstatus
    sd t0, 32(t6)
    csrr t0, sie
    sd t0, 40(t6)
    csrr t0, scounteren
    sd t0, 48(t6)
    csrr t0, sepc
    sd t0, 56(t6)
    csrr t0, scause
    sd t0, 64(t6)
    csrr t0, stval
    sd t0, 72(t6)
    sd gp, 96(t6)
    sd tp, 104(t6)
    ld t0, 40(a0)
    sd t0, 80(t6)
    SAVE_FP t0
    ld t0, 48(a0)
    sd t0, 88(t6)
    LOAD_FP t0
    la t0, conduitos_riscv64_domain_vector
    csrw stvec, t0
    ld t0, 32(a0)
    csrw sscratch, t0
    csrw scounteren, zero
    li t0, 32
    csrw sie, t0
    // UXL=64, FS=Dirty, SPIE=1; SPP/SUM/MXR/VS/UBE are clear.
    li t0, 0x200006020
    csrw sstatus, t0
    ld t0, 24(a0)
    csrw satp, t0
    sfence.vma zero, zero
    fence.i
    ld t0, 0(a0)
    csrw sepc, t0
    ld sp, 8(a0)
    ld a0, 16(a0)
    li x1, 0
    li x3, 0
    li x4, 0
    li x5, 0
    li x6, 0
    li x7, 0
    li x8, 0
    li x9, 0
    li x11, 0
    li x12, 0
    li x13, 0
    li x14, 0
    li x15, 0
    li x16, 0
    li x17, 0
    li x18, 0
    li x19, 0
    li x20, 0
    li x21, 0
    li x22, 0
    li x23, 0
    li x24, 0
    li x25, 0
    li x26, 0
    li x27, 0
    li x28, 0
    li x29, 0
    li x30, 0
    li x31, 0
    sret
.balign 4
conduitos_riscv64_domain_vector:
    csrrw sp, sscratch, sp
    addi sp, sp, -544
    sd t0, 40(sp)
    csrr t0, sstatus
    andi t0, t0, 256
    bnez t0, conduitos_riscv64_domain_root_fault
    sd x1, 8(sp)
    sd x3, 24(sp)
    sd x4, 32(sp)
    sd x6, 48(sp)
    sd x7, 56(sp)
    sd x8, 64(sp)
    sd x9, 72(sp)
    sd x10, 80(sp)
    sd x11, 88(sp)
    sd x12, 96(sp)
    sd x13, 104(sp)
    sd x14, 112(sp)
    sd x15, 120(sp)
    sd x16, 128(sp)
    sd x17, 136(sp)
    sd x18, 144(sp)
    sd x19, 152(sp)
    sd x20, 160(sp)
    sd x21, 168(sp)
    sd x22, 176(sp)
    sd x23, 184(sp)
    sd x24, 192(sp)
    sd x25, 200(sp)
    sd x26, 208(sp)
    sd x27, 216(sp)
    sd x28, 224(sp)
    sd x29, 232(sp)
    sd x30, 240(sp)
    sd x31, 248(sp)
    csrr t0, sscratch
    sd t0, 16(sp)
    csrw sscratch, zero
    csrr t0, sepc
    sd t0, 528(sp)
    csrr t0, sstatus
    sd t0, 536(sp)
    addi t0, sp, 256
    SAVE_FP t0
    la t6, conduitos_riscv64_domain_context
    ld gp, 96(t6)
    ld tp, 104(t6)
    ld t0, 80(t6)
    LOAD_FP t0
    .if {proof}
    VERIFY_FP t0, conduitos_riscv64_domain_irq_fp_failed
    .endif
    csrr t0, scause
    li t1, 0x8000000000000005
    beq t0, t1, conduitos_riscv64_domain_irq
    li t1, 8
    beq t0, t1, conduitos_riscv64_domain_gate
    li a0, 0x10e
    li a1, 1
    li t1, 1
    beq t0, t1, conduitos_riscv64_domain_return
    li t1, 5
    beq t0, t1, conduitos_riscv64_domain_return
    li t1, 7
    beq t0, t1, conduitos_riscv64_domain_return
    li t1, 12
    beq t0, t1, conduitos_riscv64_domain_return
    li t1, 13
    beq t0, t1, conduitos_riscv64_domain_return
    li t1, 15
    beq t0, t1, conduitos_riscv64_domain_return
    li a0, 0x106
    j conduitos_riscv64_domain_return
conduitos_riscv64_domain_gate:
    ld t0, 136(sp)
    bnez t0, conduitos_riscv64_domain_invalid_gate
    ld a0, 80(sp)
    li a1, 0
    j conduitos_riscv64_domain_return
conduitos_riscv64_domain_invalid_gate:
    li a0, 0
    li a1, 3
    j conduitos_riscv64_domain_return
conduitos_riscv64_domain_irq:
    call conduitos_riscv64_domain_irq_handler
    bnez a0, conduitos_riscv64_domain_expired
    addi t0, sp, 256
    LOAD_FP t0
    ld t0, 528(sp)
    csrw sepc, t0
    ld t0, 536(sp)
    csrw sstatus, t0
    ld t0, 16(sp)
    csrw sscratch, t0
    ld x1, 8(sp)
    ld x3, 24(sp)
    ld x4, 32(sp)
    ld x6, 48(sp)
    ld x7, 56(sp)
    ld x8, 64(sp)
    ld x9, 72(sp)
    ld x10, 80(sp)
    ld x11, 88(sp)
    ld x12, 96(sp)
    ld x13, 104(sp)
    ld x14, 112(sp)
    ld x15, 120(sp)
    ld x16, 128(sp)
    ld x17, 136(sp)
    ld x18, 144(sp)
    ld x19, 152(sp)
    ld x20, 160(sp)
    ld x21, 168(sp)
    ld x22, 176(sp)
    ld x23, 184(sp)
    ld x24, 192(sp)
    ld x25, 200(sp)
    ld x26, 208(sp)
    ld x27, 216(sp)
    ld x28, 224(sp)
    ld x29, 232(sp)
    ld x30, 240(sp)
    ld x31, 248(sp)
    ld t0, 40(sp)
    addi sp, sp, 544
    csrrw sp, sscratch, sp
    sret
conduitos_riscv64_domain_expired:
    li a0, 4
    li a1, 2
    j conduitos_riscv64_domain_return
conduitos_riscv64_domain_irq_fp_failed:
    li a0, 0
    li a1, 3
conduitos_riscv64_domain_return:
    mv a4, a0
    mv a5, a1
    la t6, conduitos_riscv64_domain_context
    ld t0, 80(t6)
    LOAD_FP t0
    .if {proof}
    VERIFY_FP t0, conduitos_riscv64_domain_return_fp_failed
    .endif
conduitos_riscv64_domain_restore_machine:
    ld t0, 8(t6)
    csrw satp, t0
    sfence.vma zero, zero
    ld t0, 16(t6)
    csrw stvec, t0
    ld t0, 24(t6)
    csrw sscratch, t0
    ld t0, 40(t6)
    csrw sie, t0
    ld t0, 48(t6)
    csrw scounteren, t0
    ld t0, 56(t6)
    csrw sepc, t0
    ld t0, 64(t6)
    csrw scause, t0
    ld t0, 72(t6)
    csrw stval, t0
    ld t0, 32(t6)
    csrw sstatus, t0
    ld sp, 0(t6)
    ld ra, 0(sp)
    ld gp, 8(sp)
    ld tp, 16(sp)
    ld s0, 24(sp)
    ld s1, 32(sp)
    ld s2, 40(sp)
    ld s3, 48(sp)
    ld s4, 56(sp)
    ld s5, 64(sp)
    ld s6, 72(sp)
    ld s7, 80(sp)
    ld s8, 88(sp)
    ld s9, 96(sp)
    ld s10, 104(sp)
    ld s11, 112(sp)
    addi sp, sp, 128
    mv a0, a4
    mv a1, a5
    ret
conduitos_riscv64_domain_return_fp_failed:
    li a4, 0
    li a5, 3
    j conduitos_riscv64_domain_restore_machine
conduitos_riscv64_domain_root_fault:
    csrci sstatus, 2
1:  wfi
    j 1b
.option pop
"#, proof = const cfg!(feature = "ordinary-domain-proof") as u8,
);
