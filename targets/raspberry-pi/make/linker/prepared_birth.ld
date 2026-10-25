/* Include inside SECTIONS of a NORMAL product linker, before .bss.
 * This inert reservation does not make the diagnostic A3 a Body product.
 * KEEP the section in raw-kernel flattening and bind only after exact build.
 */
.conduit_prepared_birth : ALIGN(32)
{
    __conduit_prepared_birth_slot_start = .;
    BYTE(0x43);
    BYTE(0x4f);
    BYTE(0x4e);
    BYTE(0x44);
    BYTE(0x55);
    BYTE(0x49);
    BYTE(0x54);
    BYTE(0x5f);
    BYTE(0x52);
    BYTE(0x50);
    BYTE(0x49);
    BYTE(0x5f);
    BYTE(0x42);
    BYTE(0x49);
    BYTE(0x52);
    BYTE(0x54);
    BYTE(0x48);
    BYTE(0x5f);
    BYTE(0x53);
    BYTE(0x4c);
    BYTE(0x4f);
    BYTE(0x54);
    BYTE(0x40);
    BYTE(0x31);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    BYTE(0x00);
    __conduit_prepared_birth_frame_start = .;
    FILL(0x00000000);
    . += 32768;
    __conduit_prepared_birth_frame_end = .;
}
