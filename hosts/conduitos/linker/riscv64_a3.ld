OUTPUT_FORMAT(elf64-littleriscv)
OUTPUT_ARCH(riscv)
ENTRY(conduitos_riscv64_a3_start)
PHDRS { text PT_LOAD FLAGS(5); rodata PT_LOAD FLAGS(4); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0xffffffff80000000;
    .text : ALIGN(CONSTANT(MAXPAGESIZE)) { KEEP(*(.text.conduitos_riscv64_a3_start)) *(.text .text.*) } :text
    .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.rodata .rodata.*) } :rodata
    .data : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.data .data.*) } :data
    .bss (NOLOAD) : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.bss .bss.* COMMON) } :data
    /DISCARD/ : { *(.comment) *(.eh_frame .eh_frame_hdr) *(.note .note.*) }
}
