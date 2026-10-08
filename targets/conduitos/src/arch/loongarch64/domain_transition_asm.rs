//! PLV3 entry never trusts the application stack on an exception.
core::arch::global_asm!(r#"
.section .text.conduitos_loongarch64_domain,"ax"
.global conduitos_loongarch64_domain_enter
conduitos_loongarch64_domain_enter:
    addi.d $sp, $sp, -512
    st.d $r1, $sp, 8
    st.d $r2, $sp, 16
    st.d $r3, $sp, 24
    st.d $r4, $sp, 32
    st.d $r5, $sp, 40
    st.d $r6, $sp, 48
    st.d $r7, $sp, 56
    st.d $r8, $sp, 64
    st.d $r9, $sp, 72
    st.d $r10, $sp, 80
    st.d $r11, $sp, 88
    st.d $r12, $sp, 96
    st.d $r13, $sp, 104
    st.d $r14, $sp, 112
    st.d $r15, $sp, 120
    st.d $r16, $sp, 128
    st.d $r17, $sp, 136
    st.d $r18, $sp, 144
    st.d $r19, $sp, 152
    st.d $r20, $sp, 160
    st.d $r21, $sp, 168
    st.d $r22, $sp, 176
    st.d $r23, $sp, 184
    st.d $r24, $sp, 192
    st.d $r25, $sp, 200
    st.d $r26, $sp, 208
    st.d $r27, $sp, 216
    st.d $r28, $sp, 224
    st.d $r29, $sp, 232
    st.d $r30, $sp, 240
    st.d $r31, $sp, 248
    csrrd $t0, 0
    st.d $t0, $sp, 256
    csrrd $t0, 1
    st.d $t0, $sp, 264
    csrrd $t0, 6
    st.d $t0, $sp, 272
    csrrd $t0, 12
    st.d $t0, $sp, 280
    csrrd $t0, 25
    st.d $t0, $sp, 288
    csrrd $t0, 26
    st.d $t0, $sp, 296
    csrrd $t0, 24
    st.d $t0, $sp, 304
    csrrd $t0, 3
    st.d $t0, $sp, 312
    csrrd $t0, 2
    st.d $t0, $sp, 320
    csrrd $t0, 48
    st.d $t0, $sp, 328
    csrrd $t0, 49
    st.d $t0, $sp, 336
    csrrd $t0, 4
    st.d $t0, $sp, 344
    csrrd $t0, 0x88
    st.d $t0, $sp, 408
    csrrd $t0, 0x32
    st.d $t0, $sp, 416
    csrrd $t0, 0x33
    st.d $t0, $sp, 424
    csrrd $t0, 0x34
    st.d $t0, $sp, 432
    st.d $a0, $sp, 352
    st.d $zero, $sp, 392
    ld.d $t0, $a0, 64
    st.d $t0, $sp, 400
    ld.d $t0, $a0, 48
    st.d $t0, $sp, 360
    bl conduitos_loongarch64_save_floating
    ld.d $t8, $sp, 352
    ld.d $t0, $t8, 56
    st.d $t0, $sp, 368
    bl conduitos_loongarch64_load_floating
    ld.d $t8, $sp, 352
    ld.d $t0, $t8, 72
    csrwr $t0, 0x88
    ld.d $t0, $t8, 40
    st.d $sp, $t0, -16
    csrwr $t0, 0x30
    la.pcrel $t0, conduitos_loongarch64_domain_vector
    csrwr $t0, 0x0c
    li.d $t0, 0x80
conduitos_loongarch64_domain_misc_probe:
    csrwr $t0, 3
    li.d $t0, 1
    st.d $t0, $sp, 392
    csrrd $t0, 3
    andi $t0, $t0, 0x80
    beqz $t0, conduitos_loongarch64_domain_unsupported
    li.d $t0, 0x800
    csrwr $t0, 4
    move $t0, $zero
    csrwr $t0, 0x18
    ld.d $t0, $t8, 24
    csrwr $t0, 0x19
    ld.d $t0, $t8, 32
    csrwr $t0, 0x1a
    dbar 0
    invtlb 0, $zero, $zero
    ibar 0
    ld.d $t0, $t8, 0
    csrwr $t0, 6
    li.d $t0, 7
    csrwr $t0, 1
    ld.d $sp, $t8, 8
    ld.d $a0, $t8, 16
    move $r1, $zero
    move $r2, $zero
    move $r5, $zero
    move $r6, $zero
    move $r7, $zero
    move $r8, $zero
    move $r9, $zero
    move $r10, $zero
    move $r11, $zero
    move $r12, $zero
    move $r13, $zero
    move $r14, $zero
    move $r15, $zero
    move $r16, $zero
    move $r17, $zero
    move $r18, $zero
    move $r19, $zero
    move $r20, $zero
    move $r21, $zero
    move $r22, $zero
    move $r23, $zero
    move $r24, $zero
    move $r25, $zero
    move $r26, $zero
    move $r27, $zero
    move $r28, $zero
    move $r29, $zero
    move $r30, $zero
    move $r31, $zero
    ertn
.balign 4096
conduitos_loongarch64_domain_vector:
    csrwr $t0, 0x31
    csrrd $t0, 1
    andi $t0, $t0, 3
    addi.d $t0, $t0, -3
    bnez $t0, conduitos_loongarch64_domain_root_exception
    csrrd $t0, 0x30
    st.d $sp, $t0, -296
    move $sp, $t0
    addi.d $sp, $sp, -320
    csrrd $t0, 0x31
    st.d $t0, $sp, 96
    st.d $r1, $sp, 8
    st.d $r2, $sp, 16
    st.d $r4, $sp, 32
    st.d $r5, $sp, 40
    st.d $r6, $sp, 48
    st.d $r7, $sp, 56
    st.d $r8, $sp, 64
    st.d $r9, $sp, 72
    st.d $r10, $sp, 80
    st.d $r11, $sp, 88
    st.d $r13, $sp, 104
    st.d $r14, $sp, 112
    st.d $r15, $sp, 120
    st.d $r16, $sp, 128
    st.d $r17, $sp, 136
    st.d $r18, $sp, 144
    st.d $r19, $sp, 152
    st.d $r20, $sp, 160
    st.d $r21, $sp, 168
    st.d $r22, $sp, 176
    st.d $r23, $sp, 184
    st.d $r24, $sp, 192
    st.d $r25, $sp, 200
    st.d $r26, $sp, 208
    st.d $r27, $sp, 216
    st.d $r28, $sp, 224
    st.d $r29, $sp, 232
    st.d $r30, $sp, 240
    st.d $r31, $sp, 248
    csrrd $t0, 6
    st.d $t0, $sp, 256
    csrrd $t0, 1
    st.d $t0, $sp, 264
    ld.d $t8, $sp, 304
    ld.d $t0, $t8, 368
    bl conduitos_loongarch64_save_floating
    ld.d $t8, $sp, 304
    ld.d $tp, $t8, 16
    ld.d $t0, $t8, 360
    bl conduitos_loongarch64_load_floating
    .if {proof}
    bl conduitos_loongarch64_verify_floating
    beqz $a0, conduitos_loongarch64_domain_invalid_gate
    .endif
    csrrd $t0, 5
    srli.d $t1, $t0, 16
    andi $t1, $t1, 63
    beqz $t1, conduitos_loongarch64_domain_irq
    li.d $t2, 11
    beq $t1, $t2, conduitos_loongarch64_domain_gate
    li.d $a1, 1
    li.d $a0, 0x10d
    li.d $t2, 14
    beq $t1, $t2, conduitos_loongarch64_domain_return
    li.d $a0, 0x10e
    li.d $t2, 10
    bltu $t1, $t2, conduitos_loongarch64_domain_return
    move $a0, $t1
    b conduitos_loongarch64_domain_return
conduitos_loongarch64_domain_gate:
    // BADI can change during a Root page-table refill. Decode only the
    // immutable admitted code copy, at the bounded saved application PC.
    ld.d $t0, $sp, 256
    li.d $t1, 0x400000
    sub.d $t0, $t0, $t1
    li.d $t1, 65536
    bgeu $t0, $t1, conduitos_loongarch64_domain_invalid_gate
    andi $t1, $t0, 3
    bnez $t1, conduitos_loongarch64_domain_invalid_gate
    ld.d $t8, $sp, 304
    ld.d $t1, $t8, 400
    add.d $t0, $t0, $t1
    ld.wu $t0, $t0, 0
    li.d $t1, 0x002b0000
    bne $t0, $t1, conduitos_loongarch64_domain_invalid_gate
    ld.d $a0, $sp, 32
    move $a1, $zero
    b conduitos_loongarch64_domain_return
conduitos_loongarch64_domain_irq:
    li.d $t1, 0x1fff
    and $t0, $t0, $t1
    li.d $t1, 0x800
    bne $t0, $t1, conduitos_loongarch64_domain_invalid_gate
    bl conduitos_loongarch64_domain_irq_handler
    bnez $a0, conduitos_loongarch64_domain_expired
    ld.d $t8, $sp, 304
    ld.d $t0, $t8, 368
    bl conduitos_loongarch64_load_floating
    ld.d $t0, $sp, 256
    csrwr $t0, 6
    ld.d $t0, $sp, 264
    csrwr $t0, 1
    ld.d $r1, $sp, 8
    ld.d $r2, $sp, 16
    ld.d $r4, $sp, 32
    ld.d $r5, $sp, 40
    ld.d $r6, $sp, 48
    ld.d $r7, $sp, 56
    ld.d $r8, $sp, 64
    ld.d $r9, $sp, 72
    ld.d $r10, $sp, 80
    ld.d $r11, $sp, 88
    ld.d $r12, $sp, 96
    ld.d $r13, $sp, 104
    ld.d $r14, $sp, 112
    ld.d $r15, $sp, 120
    ld.d $r16, $sp, 128
    ld.d $r17, $sp, 136
    ld.d $r18, $sp, 144
    ld.d $r19, $sp, 152
    ld.d $r20, $sp, 160
    ld.d $r21, $sp, 168
    ld.d $r22, $sp, 176
    ld.d $r23, $sp, 184
    ld.d $r24, $sp, 192
    ld.d $r25, $sp, 200
    ld.d $r26, $sp, 208
    ld.d $r27, $sp, 216
    ld.d $r28, $sp, 224
    ld.d $r29, $sp, 232
    ld.d $r30, $sp, 240
    ld.d $r31, $sp, 248
    ld.d $sp, $sp, 24
    ertn
conduitos_loongarch64_domain_expired:
    li.d $a0, 4
    li.d $a1, 2
    b conduitos_loongarch64_domain_return
conduitos_loongarch64_domain_invalid_gate:
    move $a0, $zero
    li.d $a1, 3
conduitos_loongarch64_domain_return:
    ld.d $t8, $sp, 304
    move $sp, $t8
conduitos_loongarch64_domain_root_return:
    st.d $a0, $sp, 376
    st.d $a1, $sp, 384
    ld.d $t0, $sp, 360
    bl conduitos_loongarch64_load_floating
    .if {proof}
    bl conduitos_loongarch64_verify_floating
    bnez $a0, 1f
    st.d $zero, $sp, 376
    li.d $t0, 3
    st.d $t0, $sp, 384
1:
    .endif
    ld.d $t0, $sp, 264
    csrwr $t0, 1
    ld.d $t0, $sp, 272
    csrwr $t0, 6
    ld.d $t0, $sp, 280
    csrwr $t0, 12
    ld.d $t0, $sp, 288
    csrwr $t0, 25
    ld.d $t0, $sp, 296
    csrwr $t0, 26
    ld.d $t0, $sp, 304
    csrwr $t0, 24
    ld.d $t0, $sp, 392
    beqz $t0, 3f
    ld.d $t0, $sp, 312
    csrwr $t0, 3
3:
    ld.d $t0, $sp, 320
    csrwr $t0, 2
    ld.d $t0, $sp, 328
    csrwr $t0, 48
    ld.d $t0, $sp, 336
    csrwr $t0, 49
    ld.d $t0, $sp, 344
    csrwr $t0, 4
    ld.d $t0, $sp, 408
    csrwr $t0, 0x88
    ld.d $t0, $sp, 416
    csrwr $t0, 0x32
    ld.d $t0, $sp, 424
    csrwr $t0, 0x33
    ld.d $t0, $sp, 432
    csrwr $t0, 0x34
    dbar 0
    invtlb 0, $zero, $zero
    ibar 0
    ld.d $t0, $sp, 256
    csrwr $t0, 0
    ld.d $r1, $sp, 8
    ld.d $r2, $sp, 16
    ld.d $r6, $sp, 48
    ld.d $r7, $sp, 56
    ld.d $r8, $sp, 64
    ld.d $r9, $sp, 72
    ld.d $r10, $sp, 80
    ld.d $r11, $sp, 88
    ld.d $r12, $sp, 96
    ld.d $r13, $sp, 104
    ld.d $r14, $sp, 112
    ld.d $r15, $sp, 120
    ld.d $r16, $sp, 128
    ld.d $r17, $sp, 136
    ld.d $r18, $sp, 144
    ld.d $r19, $sp, 152
    ld.d $r20, $sp, 160
    ld.d $r21, $sp, 168
    ld.d $r22, $sp, 176
    ld.d $r23, $sp, 184
    ld.d $r24, $sp, 192
    ld.d $r25, $sp, 200
    ld.d $r26, $sp, 208
    ld.d $r27, $sp, 216
    ld.d $r28, $sp, 224
    ld.d $r29, $sp, 232
    ld.d $r30, $sp, 240
    ld.d $r31, $sp, 248
    ld.d $a0, $sp, 376
    ld.d $a1, $sp, 384
    addi.d $sp, $sp, 512
    jirl $zero, $ra, 0
conduitos_loongarch64_domain_root_exception:
    // A reviewed pre-entry probe may discover an unimplemented writable CSR.
    // Only that exact Root PC and INE cause can return Unsupported. Every
    // other Root-origin exception fails closed without trusting a stack.
    csrrd $t0, 1
    andi $t0, $t0, 3
    bnez $t0, conduitos_loongarch64_domain_root_fault
    csrrd $t0, 5
    srli.d $t0, $t0, 16
    andi $t0, $t0, 63
    addi.d $t0, $t0, -13
    bnez $t0, conduitos_loongarch64_domain_root_fault
    csrrd $t0, 6
    la.pcrel $t1, conduitos_loongarch64_domain_misc_probe
    bne $t0, $t1, conduitos_loongarch64_domain_root_fault
conduitos_loongarch64_domain_unsupported:
    move $a0, $zero
    li.d $a1, 4
    b conduitos_loongarch64_domain_root_return
conduitos_loongarch64_domain_root_fault:
    csrwr $zero, 4
2:  idle 0
    b 2b
"#,proof=const cfg!(feature="ordinary-domain-proof") as u8);
