ENTRY(conduitos_armv6_rpi_b_plus_product_entry)

SECTIONS
{
    . = 0x00008000;
    __image_start = .;
    .text : ALIGN(4)
    {
        KEEP(*(.text.entry))
        KEEP(*(.text.armv6_vectors))
        *(.text .text.*)
    }
    .rodata : ALIGN(4) { *(.rodata .rodata.*) }
    .data : ALIGN(4) { *(.data .data.*) }
    __image_file_end = .;
    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss.boot_stack)
        *(.bss.irq_stack)
        *(.bss.exception_stack)
        *(.bss.runtime_arena)
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }
    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.ARM.extab*)
        *(.comment)
        *(.eh_frame*)
    }
}
