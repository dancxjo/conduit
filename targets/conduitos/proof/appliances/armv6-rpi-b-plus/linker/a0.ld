ENTRY(conduitos_armv6_rpi_b_plus_entry)

SECTIONS
{
    . = 0x00008000;

    .text : ALIGN(4)
    {
        KEEP(*(.text.entry))
        *(.text .text.*)
    }

    .rodata : ALIGN(4)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4)
    {
        *(.data .data.*)
    }

    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
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
