OUTPUT_FORMAT(elf32-i386)
OUTPUT_ARCH(i386)
ENTRY(conduitos_ia32_a2_start)

PHDRS { text PT_LOAD FLAGS(5); rodata PT_LOAD FLAGS(4); data PT_LOAD FLAGS(6); }

SECTIONS {
    . = 0x00100000;
    .text : ALIGN(CONSTANT(MAXPAGESIZE)) {
        KEEP(*(.multiboot))
        KEEP(*(.text.conduitos_ia32_a2_start))
        *(.text .text.*)
    } :text
    .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.rodata .rodata.*) } :rodata
    .data : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.data .data.*) } :data
    .bss (NOLOAD) : ALIGN(CONSTANT(MAXPAGESIZE)) { *(.bss .bss.* COMMON) } :data
    /DISCARD/ : { *(.comment) *(.eh_frame .eh_frame_hdr) *(.note .note.*) }
}
