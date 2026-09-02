OUTPUT_FORMAT(elf32-i386)
OUTPUT_ARCH(i386)
ENTRY(conduitos_ia32_product_start)
PHDRS { text PT_LOAD FLAGS(5); rodata PT_LOAD FLAGS(4); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x00100000;
    __conduitos_image_start = .;
    .multiboot : { KEEP(*(.multiboot)) } :text
    .text : {
        KEEP(*(.text.conduitos_ia32_product_start)) *(.text .text.*)
    } :text
    . = ALIGN(0x1000);
    .rodata : { *(.rodata .rodata.*) *(.gcc_except_table .gcc_except_table.*) } :rodata
    . = ALIGN(0x1000);
    .data : { *(.data .data.*) *(.got .got.*) } :data
    . = ALIGN(0x1000);
    .bss (NOLOAD) : { *(.bss .bss.* COMMON) } :data
    __conduitos_image_end = .;
    /DISCARD/ : { *(.comment) *(.eh_frame .eh_frame_hdr) *(.note .note.*) }
}
