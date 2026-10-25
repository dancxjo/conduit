ENTRY(conduitos_jetson_nano_image_header)

PHDRS { text PT_LOAD FLAGS(5); rodata PT_LOAD FLAGS(4); data PT_LOAD FLAGS(6); }

SECTIONS {
    . = 0xf0080000;
    __conduitos_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.jetson_nano_header))
        *(.text .text.*)
    } :text
    .rodata : ALIGN(4096) { *(.rodata .rodata.*) } :rodata
    .data : ALIGN(4096) { *(.data .data.*) } :data
    .bss (NOLOAD) : ALIGN(4096) {
        __conduitos_bss_start = .;
        *(.bss .bss.* COMMON)
        __conduitos_bss_end = ALIGN(16);
    } :data
    .boot_stack (NOLOAD) : ALIGN(4096) {
        __conduitos_boot_stack_start = .;
        . += 16384;
        __conduitos_boot_stack_top = .;
    } :data
    __conduitos_image_end = .;
    __conduitos_image_size = __conduitos_image_end - __conduitos_image_start;
    /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
