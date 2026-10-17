//! Scalar F/D and all condition flags, shared by privileged IRQ and domain entry.
core::arch::global_asm!(
    r#"
.section .text.conduitos_loongarch64_floating,"ax"
.global conduitos_loongarch64_save_floating
conduitos_loongarch64_save_floating:
    fst.d $f0, $t0, 0
    fst.d $f1, $t0, 8
    fst.d $f2, $t0, 16
    fst.d $f3, $t0, 24
    fst.d $f4, $t0, 32
    fst.d $f5, $t0, 40
    fst.d $f6, $t0, 48
    fst.d $f7, $t0, 56
    fst.d $f8, $t0, 64
    fst.d $f9, $t0, 72
    fst.d $f10, $t0, 80
    fst.d $f11, $t0, 88
    fst.d $f12, $t0, 96
    fst.d $f13, $t0, 104
    fst.d $f14, $t0, 112
    fst.d $f15, $t0, 120
    fst.d $f16, $t0, 128
    fst.d $f17, $t0, 136
    fst.d $f18, $t0, 144
    fst.d $f19, $t0, 152
    fst.d $f20, $t0, 160
    fst.d $f21, $t0, 168
    fst.d $f22, $t0, 176
    fst.d $f23, $t0, 184
    fst.d $f24, $t0, 192
    fst.d $f25, $t0, 200
    fst.d $f26, $t0, 208
    fst.d $f27, $t0, 216
    fst.d $f28, $t0, 224
    fst.d $f29, $t0, 232
    fst.d $f30, $t0, 240
    fst.d $f31, $t0, 248
    movfcsr2gr $t1, $fcsr0
    st.d $t1, $t0, 256
    movcf2gr $t1, $fcc0
    st.b $t1, $t0, 264
    movcf2gr $t1, $fcc1
    st.b $t1, $t0, 265
    movcf2gr $t1, $fcc2
    st.b $t1, $t0, 266
    movcf2gr $t1, $fcc3
    st.b $t1, $t0, 267
    movcf2gr $t1, $fcc4
    st.b $t1, $t0, 268
    movcf2gr $t1, $fcc5
    st.b $t1, $t0, 269
    movcf2gr $t1, $fcc6
    st.b $t1, $t0, 270
    movcf2gr $t1, $fcc7
    st.b $t1, $t0, 271
    jirl $zero, $ra, 0
.global conduitos_loongarch64_load_floating
conduitos_loongarch64_load_floating:
    fld.d $f0, $t0, 0
    fld.d $f1, $t0, 8
    fld.d $f2, $t0, 16
    fld.d $f3, $t0, 24
    fld.d $f4, $t0, 32
    fld.d $f5, $t0, 40
    fld.d $f6, $t0, 48
    fld.d $f7, $t0, 56
    fld.d $f8, $t0, 64
    fld.d $f9, $t0, 72
    fld.d $f10, $t0, 80
    fld.d $f11, $t0, 88
    fld.d $f12, $t0, 96
    fld.d $f13, $t0, 104
    fld.d $f14, $t0, 112
    fld.d $f15, $t0, 120
    fld.d $f16, $t0, 128
    fld.d $f17, $t0, 136
    fld.d $f18, $t0, 144
    fld.d $f19, $t0, 152
    fld.d $f20, $t0, 160
    fld.d $f21, $t0, 168
    fld.d $f22, $t0, 176
    fld.d $f23, $t0, 184
    fld.d $f24, $t0, 192
    fld.d $f25, $t0, 200
    fld.d $f26, $t0, 208
    fld.d $f27, $t0, 216
    fld.d $f28, $t0, 224
    fld.d $f29, $t0, 232
    fld.d $f30, $t0, 240
    fld.d $f31, $t0, 248
    ld.d $t1, $t0, 256
    movgr2fcsr $fcsr0, $t1
    ld.bu $t1, $t0, 264
    movgr2cf $fcc0, $t1
    ld.bu $t1, $t0, 265
    movgr2cf $fcc1, $t1
    ld.bu $t1, $t0, 266
    movgr2cf $fcc2, $t1
    ld.bu $t1, $t0, 267
    movgr2cf $fcc3, $t1
    ld.bu $t1, $t0, 268
    movgr2cf $fcc4, $t1
    ld.bu $t1, $t0, 269
    movgr2cf $fcc5, $t1
    ld.bu $t1, $t0, 270
    movgr2cf $fcc6, $t1
    ld.bu $t1, $t0, 271
    movgr2cf $fcc7, $t1
    jirl $zero, $ra, 0
.global conduitos_loongarch64_verify_floating
conduitos_loongarch64_verify_floating:
    addi.d $sp, $sp, -288
    st.d $ra, $sp, 272
    move $t6, $t0
    move $t0, $sp
    bl conduitos_loongarch64_save_floating
    li.d $t2, 0
1:  add.d $t3, $t6, $t2
    ld.d $t3, $t3, 0
    add.d $t4, $sp, $t2
    ld.d $t4, $t4, 0
    bne $t3, $t4, 2f
    addi.d $t2, $t2, 8
    li.d $t3, 272
    bltu $t2, $t3, 1b
    li.d $a0, 1
    b 3f
2:  move $a0, $zero
3:  ld.d $ra, $sp, 272
    addi.d $sp, $sp, 288
    jirl $zero, $ra, 0
"#
);
