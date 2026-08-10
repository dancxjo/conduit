use sha2::Digest;

pub(crate) fn deterministic_pcm(text: &str) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(text.len() * 64);
    for (index, byte) in text.bytes().enumerate() {
        for sample in 0..32u16 {
            let value = (i16::from(byte) - 64) * 128
                + i16::try_from((index + usize::from(sample)) % 32).unwrap() * 8;
            pcm.extend_from_slice(&value.to_le_bytes());
        }
    }
    pcm
}

pub(crate) fn wav(pcm: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}
