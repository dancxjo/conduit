ENTRY(conduitos_aarch64_a0_start)

SECTIONS
{
    . = 0x40000000;

    .text : ALIGN(16) {
        KEEP(*(.text.conduitos_aarch64_a0_start))
        *(.text .text.*)
    }

    .rodata : ALIGN(16) { *(.rodata .rodata.*) }
    .data : ALIGN(16) { *(.data .data.*) }
    .bss (NOLOAD) : ALIGN(16) { *(.bss .bss.* COMMON) }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame .eh_frame_hdr)
    }
}
