use super::EspeakFailure;
/// eSpeak --stdout uses streaming RIFF/data length sentinels (0x7ffff024 /
/// 0x7ffff000); successful process EOF supplies the actual bounded extent.
pub(super) fn pcm(bytes: &[u8], maximum: usize) -> Result<&[u8], EspeakFailure> {
    let invalid = EspeakFailure::InvalidWav;
    if bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u32_at(bytes, 16) != 16
        || u16_at(bytes, 20) != 1
        || u16_at(bytes, 22) != 1
        || u32_at(bytes, 24) != 22050
        || u32_at(bytes, 28) != 44100
        || u16_at(bytes, 32) != 2
        || u16_at(bytes, 34) != 16
        || &bytes[36..40] != b"data"
    {
        return Err(invalid);
    }
    let payload = &bytes[44..];
    if payload.len() > maximum {
        return Err(EspeakFailure::OutputOverflow);
    }
    if payload.is_empty() || payload.len() % 2 != 0 {
        return Err(invalid);
    }
    let riff = u32_at(bytes, 4);
    let data = u32_at(bytes, 40);
    if !((riff as usize == bytes.len() - 8 && data as usize == payload.len())
        || (riff == 0x7ffff024 && data == 0x7ffff000))
    {
        return Err(invalid);
    }
    Ok(payload)
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
