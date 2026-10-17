//! Root owns the fixed four-level walk, including absent directories and NX.
//! Refill executes at its physical address in direct mode without a stack.
core::arch::global_asm!(
    r#"
.section .text.conduitos_loongarch64_domain_refill,"ax"
.balign 4096
.global conduitos_loongarch64_domain_refill
conduitos_loongarch64_domain_refill:
    csrwr $t0, 0x8b
    csrwr $t1, 0x32
    csrwr $t2, 0x33
    csrwr $t3, 0x34
    csrrd $t0, 0x1b
    csrrd $t1, 0x89
    srli.d $t2, $t1, 39
    andi $t2, $t2, 511
    slli.d $t2, $t2, 3
    add.d $t0, $t0, $t2
    ld.d $t0, $t0, 0
    beqz $t0, conduitos_loongarch64_refill_absent
    srli.d $t2, $t1, 30
    andi $t2, $t2, 511
    slli.d $t2, $t2, 3
    add.d $t0, $t0, $t2
    ld.d $t0, $t0, 0
    beqz $t0, conduitos_loongarch64_refill_absent
    andi $t2, $t0, 64
    bnez $t2, conduitos_loongarch64_refill_huge_l2
    srli.d $t2, $t1, 21
    andi $t2, $t2, 511
    slli.d $t2, $t2, 3
    add.d $t0, $t0, $t2
    ld.d $t0, $t0, 0
    beqz $t0, conduitos_loongarch64_refill_absent
    andi $t2, $t0, 64
    bnez $t2, conduitos_loongarch64_refill_huge_l1
    srli.d $t2, $t1, 12
    andi $t2, $t2, 510
    slli.d $t2, $t2, 3
    add.d $t0, $t0, $t2
    ld.d $t2, $t0, 0
    csrwr $t2, 0x8c
    ld.d $t2, $t0, 8
    csrwr $t2, 0x8d
    li.d $t2, 12
    b conduitos_loongarch64_refill_fill
conduitos_loongarch64_refill_huge_l2:
    li.d $t2, 29
    b conduitos_loongarch64_refill_huge
conduitos_loongarch64_refill_huge_l1:
    li.d $t2, 20
conduitos_loongarch64_refill_huge:
    // Validated inherited huge leaves have HGlobal set. Split their physical
    // extent into the paired halves and retain every protection attribute.
    li.d $t3, -4161
    and $t0, $t0, $t3
    ori $t0, $t0, 64
    move $t3, $t0
    csrwr $t3, 0x8c
    li.d $t3, 1
    sll.d $t3, $t3, $t2
    add.d $t0, $t0, $t3
    csrwr $t0, 0x8d
    b conduitos_loongarch64_refill_fill
conduitos_loongarch64_refill_absent:
    csrwr $zero, 0x8c
    csrwr $zero, 0x8d
    li.d $t2, 12
conduitos_loongarch64_refill_fill:
    csrrd $t3, 0x8e
    li.d $t0, -64
    and $t3, $t3, $t0
    or $t3, $t3, $t2
    csrwr $t3, 0x8e
    tlbfill
    csrrd $t0, 0x8b
    csrrd $t1, 0x32
    csrrd $t2, 0x33
    csrrd $t3, 0x34
    ertn
"#
);
unsafe extern "C" {
    static conduitos_loongarch64_domain_refill: u8;
}
pub(super) fn physical_entry() -> Result<u64, crate::protected_region::DomainRefusal> {
    crate::boot::executable_physical_address(unsafe {
        &conduitos_loongarch64_domain_refill as *const u8 as u64
    })
    .filter(|address| *address != 0 && *address & 0xfff == 0 && *address >> 48 == 0)
    .ok_or(crate::protected_region::DomainRefusal::InvalidMemory)
}
