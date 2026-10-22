ENTRY(compositor_entry)
PHDRS { code PT_LOAD FLAGS(5); constants PT_LOAD FLAGS(4); }
SECTIONS {
    . = 0x00400000;
    .text : ALIGN(4096) { KEEP(*(.text.entry)) *(.text .text.*) } :code
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) *(.got .got.*) } :constants
    __compositor_image_end = ALIGN(4096);
    .mutable_rejected : { *(.data .data.*) *(.bss .bss.*) *(COMMON) *(.tdata .tdata.*) *(.tbss .tbss.*) }
    ASSERT(SIZEOF(.mutable_rejected) == 0, "compositor image must not contain mutable LOAD state")
    ASSERT(__compositor_image_end <= 0x00600000, "compositor image exceeds admitted window")
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) *(.comment) }
}
