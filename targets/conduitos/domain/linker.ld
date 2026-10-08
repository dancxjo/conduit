ENTRY(domain_entry)
PHDRS { code PT_LOAD FLAGS(5); constants PT_LOAD FLAGS(4); }
SECTIONS {
    . = DEFINED(__domain_text_start) ? __domain_text_start : 0x00400000;
    .text : ALIGN(4096) { *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) } :constants
    . = ALIGN(4096);
    __domain_image_end = .;
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.riscv.attributes) }
}
