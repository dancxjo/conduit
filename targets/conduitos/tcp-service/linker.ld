ENTRY(tcp_entry)
PHDRS { code PT_LOAD FLAGS(5); constants PT_LOAD FLAGS(4); private_data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x00400000;
    .text : ALIGN(4096) { KEEP(*(.text.entry)) *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) *(.got .got.*) } :constants
    __tcp_image_end = ALIGN(4096);
    ASSERT(__tcp_image_end <= 0x00600000, "TCP image exceeds admitted window")
    . = 0x00709000;
    .data : { *(.data .data.*) } :private_data
    .bss : { *(.bss .bss.*) *(COMMON) } :private_data
    ASSERT(. <= 0x0070a000, "TCP private data exceeds admitted first metadata page")
    .tcp_rejected : { *(.tdata .tdata.*) *(.tbss .tbss.*) }
    ASSERT(SIZEOF(.tcp_rejected) == 0, "TCP guest does not admit thread-local storage")
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.comment) }
}
