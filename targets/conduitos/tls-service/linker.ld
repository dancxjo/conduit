ENTRY(tls_entry)
PHDRS { code PT_LOAD FLAGS(5); constants PT_LOAD FLAGS(4); private_data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x00400000;
    .text : ALIGN(4096) { KEEP(*(.text.entry)) *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) *(.got .got.*) } :constants
    __tls_image_end = ALIGN(4096);
    ASSERT(__tls_image_end <= 0x00600000, "TLS image exceeds admitted window")
    . = 0x00709000;
    .data : { *(.data .data.*) } :private_data
    .bss : { *(.bss .bss.*) *(COMMON) } :private_data
    ASSERT(. <= 0x0070a000, "TLS private data exceeds admitted first metadata page")
    .tls_rejected : { *(.tdata .tdata.*) *(.tbss .tbss.*) }
    ASSERT(SIZEOF(.tls_rejected) == 0, "TLS guest does not admit thread-local storage")
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.comment) }
}
