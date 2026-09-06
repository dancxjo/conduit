OUTPUT_FORMAT(elf64-littleriscv)
OUTPUT_ARCH(riscv)
ENTRY(conduitos_riscv64_product_start)
PHDRS { text PT_LOAD FLAGS(5); rodata PT_LOAD FLAGS(4); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0xffffffff80000000;
    __conduitos_image_start = .;
    .text : ALIGN(CONSTANT(MAXPAGESIZE)) { KEEP(*(.text.conduitos_riscv64_product_start)) *(.text .text.*) } :text
    .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.rodata .rodata.*) } :rodata
    .data : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.data .data.*) } :data
    .requests : ALIGN(CONSTANT(MAXPAGESIZE)) {
        KEEP(*(.requests_start_marker)) KEEP(*(.requests)) KEEP(*(.requests_end_marker))
        KEEP(*(.limine_reqs*)) KEEP(*(.limine_requests*))
    } :data
    .bss (NOLOAD) : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.bss .bss.* COMMON) } :data
    __conduitos_image_end = .;
    /DISCARD/ : { *(.comment) *(.eh_frame .eh_frame_hdr) *(.note .note.*) }
}
