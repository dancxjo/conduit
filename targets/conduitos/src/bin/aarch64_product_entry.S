// Limine's original EL1 entry symbol and stack remain unchanged. x9 is an
// AAPCS64 caller-saved scratch register. No stack or FP/SIMD access may precede
// this enable: compiler-generated Rust prologues can spill d8..d15.
.section .text.conduitos_aarch64_product_start, "ax"
.global conduitos_aarch64_product_start
.type conduitos_aarch64_product_start, %function
conduitos_aarch64_product_start:
    mrs x9, cpacr_el1
    orr x9, x9, #0x300000
    msr cpacr_el1, x9
    isb
    b conduitos_aarch64_product_rust_start
.size conduitos_aarch64_product_start, . - conduitos_aarch64_product_start
