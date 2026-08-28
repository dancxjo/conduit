OUTPUT_FORMAT(elf64-x86-64)
ENTRY(conduitos_start)

PHDRS
{
    text   PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data   PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = 0xffffffff80000000;
    __conduitos_image_start = .;

    .text : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.rodata .rodata.*)
    } :rodata

    .data ALIGN(CONSTANT(MAXPAGESIZE)) : {
        *(.data .data.*)
    } :data

    .requests ALIGN(CONSTANT(MAXPAGESIZE)) : {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
        KEEP(*(.limine_reqs*))
        KEEP(*(.limine_requests*))
    } :data

    .bss : ALIGN(CONSTANT(MAXPAGESIZE)) {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    __conduitos_image_end = .;

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
