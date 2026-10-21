ENTRY(creche_entry)
PHDRS { code PT_LOAD FLAGS(5); constants PT_LOAD FLAGS(4); private_data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x00400000;
    .text : ALIGN(4096) { KEEP(*(.text.entry)) *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) *(.got .got.*) } :constants
    __creche_image_end = ALIGN(4096);
    ASSERT(__creche_image_end <= 0x00600000, "Creche image exceeds admitted window")
    . = 0x00619000;
    .data : { *(.data .data.*) } :private_data
    .bss : { *(.bss .bss.*) *(COMMON) } :private_data
    ASSERT(. <= 0x0061a000, "Creche private data exceeds admitted first metadata page")
    .tls_rejected : { *(.tdata .tdata.*) *(.tbss .tbss.*) }
    ASSERT(SIZEOF(.tls_rejected) == 0, "Creche service does not admit TLS")
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.comment) }
}
