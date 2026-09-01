pub const SPORE_FLASH_ADDRESS: u32 = 0x003f_f000;
pub const SPORE_REGION_BYTES: usize = 4096;
pub const SPORE_READ_BYTES: usize = 1024;
const _: () = assert!(SPORE_FLASH_ADDRESS as usize + SPORE_REGION_BYTES == 4 * 1024 * 1024);
const MAGIC: &[u8; 16] = b"CONDUIT_SPORE@1\0";
const VERSION: u8 = 1;
const FIXED_BYTES: usize = 91;
const FIELD_COUNT: usize = 4;
const MAX_ID_BYTES: usize = 128;

#[derive(Debug, PartialEq, Eq)]
pub struct EmbeddedSpore<'a> {
    pub expires_at_millis: u64,
    pub nonce: &'a [u8],
    pub secret: &'a [u8],
    pub spore_id: &'a str,
    pub image_id: &'a str,
    pub invitation_id: &'a str,
    pub body_id: &'a str,
}

pub fn parse(bytes: &[u8]) -> Option<EmbeddedSpore<'_>> {
    if bytes.len() < SPORE_READ_BYTES
        || bytes.get(..MAGIC.len())? != MAGIC
        || bytes[MAGIC.len()] != VERSION
    {
        return None;
    }
    let total = u16::from_le_bytes([bytes[17], bytes[18]]) as usize;
    if !(FIXED_BYTES + FIELD_COUNT * 2..=SPORE_READ_BYTES).contains(&total) {
        return None;
    }
    let expires_at_millis = u64::from_le_bytes(bytes[19..27].try_into().ok()?);
    if expires_at_millis == 0 {
        return None;
    }
    let nonce = &bytes[27..59];
    let secret = &bytes[59..91];
    let mut cursor = FIXED_BYTES;
    let mut fields = [""; FIELD_COUNT];
    for field in &mut fields {
        let length = *bytes.get(cursor)? as usize;
        cursor += 1;
        if length == 0 || length > MAX_ID_BYTES || cursor + length > total {
            return None;
        }
        let value = bytes.get(cursor..cursor + length)?;
        if value.iter().any(|byte| !(0x21..=0x7e).contains(byte)) {
            return None;
        }
        *field = core::str::from_utf8(value).ok()?;
        cursor += length;
    }
    if cursor != total {
        return None;
    }
    Some(EmbeddedSpore {
        expires_at_millis,
        nonce,
        secret,
        spore_id: fields[0],
        image_id: fields[1],
        invitation_id: fields[2],
        body_id: fields[3],
    })
}

#[cfg(any(target_arch = "xtensa", target_arch = "riscv32"))]
pub fn attest_from_flash() {
    let mut words = [0_u32; SPORE_READ_BYTES / 4];
    let result = unsafe {
        esp_hal::rom::spiflash::esp_rom_spiflash_read(
            SPORE_FLASH_ADDRESS,
            words.as_mut_ptr(),
            SPORE_READ_BYTES as u32,
        )
    };
    if result != esp_hal::rom::spiflash::ESP_ROM_SPIFLASH_RESULT_OK {
        esp_println::println!("CONDUIT_SPORE status=read-refused code={result}");
        return;
    }
    let bytes =
        unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), SPORE_READ_BYTES) };
    if let Some(spore) = parse(bytes) {
        esp_println::println!(
            "CONDUIT_SPORE status=present spore_id={} image_id={} invitation_id={} body_id={} expiry={} secret=retained",
            spore.spore_id,
            spore.image_id,
            spore.invitation_id,
            spore.body_id,
            spore.expires_at_millis,
        );
    } else {
        esp_println::println!("CONDUIT_SPORE status=absent");
    }
    words.fill(0);
}
