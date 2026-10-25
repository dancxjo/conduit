//! Streaming copy, exact pristine slot patch and independent output readback.
use super::*;
use conduit_body::PREPARED_BIRTH_MEDIA_BYTES;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

const LEGACY_MAGIC: &[u8] = b"CONDUIT_SPORE_MEDIA@1\0";
const BIRTH_MAGIC: &[u8] = b"CDBIRTH1";
const CHUNK: usize = 64 * 1024;
const MAX_CANDIDATES: usize = 64;

/// Caller admits both regular File handles and the target builder receipt.
/// Output must be empty and opened read/write. Failure leaves caller-owned
/// partial output intact; this function never removes or mutates the input.
#[allow(clippy::too_many_arguments)]
pub fn bind_prepared_birth_iso(
    input: &mut File,
    output: &mut File,
    image: &HostImage,
    media: &PreparedBirthMedia,
    target_build_manifest: &[u8],
    maximum_artifact_bytes: u64,
    progress: &mut impl FnMut() -> Result<(), PreparedBirthArtifactRefusal>,
) -> Result<PreparedBirthArtifactReceipt, PreparedBirthArtifactRefusal> {
    use PreparedBirthArtifactRefusal as E;
    progress()?;
    let base = manifest::verify(target_build_manifest, image, media)?;
    let source = input.metadata().map_err(|_| E::Io)?;
    let destination = output.metadata().map_err(|_| E::Io)?;
    if !source.is_file() || !destination.is_file() || destination.len() != 0 {
        return Err(E::Io);
    }
    if source.len() < 17 * 2048 {
        return Err(E::InvalidIso);
    }
    if source.len() > maximum_artifact_bytes {
        return Err(E::BoundExhausted);
    }
    seek(output, 0)?;
    let mut probe = [0; 1];
    if output.read(&mut probe).map_err(|_| E::Io)? != 0 {
        return Err(E::Io);
    }
    seek(input, 0)?;
    let mut candidates = Candidates::new();
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; CHUNK + 32];
    let mut overlap = 0;
    loop {
        progress()?;
        let n = input
            .read(&mut buffer[overlap..overlap + CHUNK])
            .map_err(|_| E::Io)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or(E::BoundExhausted)?;
        if total > maximum_artifact_bytes {
            return Err(E::BoundExhausted);
        }
        hash.update(&buffer[overlap..overlap + n]);
        output
            .write_all(&buffer[overlap..overlap + n])
            .map_err(|_| E::Io)?;
        candidates.observe(&buffer[..overlap + n], total - n as u64 - overlap as u64)?;
        let keep = (LEGACY_MAGIC.len() - 1).min(overlap + n);
        buffer.copy_within(overlap + n - keep..overlap + n, 0);
        overlap = keep;
    }
    if total != source.len() || format!("sha256:{:x}", hash.finalize()) != base.sha256 {
        return Err(E::InvalidManifest);
    }
    output.flush().map_err(|_| E::Io)?;
    validate_iso(output)?;
    let slot = locate_slot(output, total, &candidates, progress)?;
    let mut frame = [0_u8; PREPARED_BIRTH_MEDIA_BYTES];
    media
        .encode_into(&mut frame)
        .map_err(|_| E::InvalidSelection)?;
    progress()?;
    seek(output, slot)?;
    output.write_all(&frame).map_err(|_| E::Io)?;
    output.flush().map_err(|_| E::Io)?;
    output.sync_all().map_err(|_| E::Io)?;
    progress()?;
    seek(output, slot)?;
    let mut observed = [0_u8; PREPARED_BIRTH_MEDIA_BYTES];
    output.read_exact(&mut observed).map_err(|_| E::Io)?;
    if observed != frame
        || PreparedBirthMedia::decode(&observed).map_err(|_| E::InvalidSelection)? != *media
    {
        return Err(E::InvalidSelection);
    }
    let frame_sha256 = format!("sha256:{:x}", Sha256::digest(observed));
    seek(output, 0)?;
    let mut hash = Sha256::new();
    let mut observed_bytes = 0_u64;
    let mut buffer = [0_u8; CHUNK];
    loop {
        progress()?;
        let n = output.read(&mut buffer).map_err(|_| E::Io)?;
        if n == 0 {
            break;
        }
        observed_bytes = observed_bytes
            .checked_add(n as u64)
            .ok_or(E::BoundExhausted)?;
        if observed_bytes > maximum_artifact_bytes {
            return Err(E::BoundExhausted);
        }
        let chunk_start = observed_bytes - n as u64;
        let start = chunk_start.max(slot);
        let end = observed_bytes.min(slot + PREPARED_BIRTH_MEDIA_BYTES as u64);
        if start < end
            && buffer[(start - chunk_start) as usize..(end - chunk_start) as usize]
                != frame[(start - slot) as usize..(end - slot) as usize]
        {
            return Err(E::InvalidSelection);
        }
        hash.update(&buffer[..n]);
    }
    if observed_bytes != total || output.metadata().map_err(|_| E::Io)?.len() != total {
        return Err(E::Io);
    }
    progress()?;
    let b = media.binding();
    Ok(PreparedBirthArtifactReceipt {
        schema: "conduit.body/prepared-birth-artifact@1".into(),
        body_id: media.evidence().body_id.as_str().into(),
        host_entry: b.host_entry.clone(),
        first_host_entry: b.first_host_entry.clone(),
        construction_id: b.construction_id.clone(),
        configuration_id: b.configuration_id.clone(),
        profile_id: b.profile_id.clone(),
        build_id: b.build_id.clone(),
        target: b.target.clone(),
        base_image_id: base.image_id,
        base_sha256: base.sha256,
        frame_sha256,
        artifact_sha256: format!("sha256:{:x}", hash.finalize()),
        artifact_bytes: total,
        frame_offset: slot,
        boot_observed: false,
        join_observed: false,
    })
}

struct Candidates {
    positions: [u64; MAX_CANDIDATES],
    len: usize,
}
impl Candidates {
    fn new() -> Self {
        Self {
            positions: [0; MAX_CANDIDATES],
            len: 0,
        }
    }
    fn observe(&mut self, bytes: &[u8], start: u64) -> Result<(), PreparedBirthArtifactRefusal> {
        for index in 0..bytes.len() {
            if bytes[index..].starts_with(LEGACY_MAGIC) || bytes[index..].starts_with(BIRTH_MAGIC) {
                let position = start + index as u64;
                if self.positions[..self.len].contains(&position) {
                    continue;
                }
                if self.len == MAX_CANDIDATES {
                    return Err(PreparedBirthArtifactRefusal::BoundExhausted);
                }
                self.positions[self.len] = position;
                self.len += 1;
            }
        }
        Ok(())
    }
}
fn locate_slot(
    file: &mut File,
    bytes: u64,
    candidates: &Candidates,
    progress: &mut impl FnMut() -> Result<(), PreparedBirthArtifactRefusal>,
) -> Result<u64, PreparedBirthArtifactRefusal> {
    use PreparedBirthArtifactRefusal as E;
    let mut slot = None;
    let mut frame = [0_u8; PREPARED_BIRTH_MEDIA_BYTES];
    for offset in &candidates.positions[..candidates.len] {
        progress()?;
        if offset
            .checked_add(PREPARED_BIRTH_MEDIA_BYTES as u64)
            .is_none_or(|end| end > bytes)
        {
            continue;
        }
        seek(file, *offset)?;
        file.read_exact(&mut frame).map_err(|_| E::Io)?;
        if frame.starts_with(BIRTH_MAGIC) {
            // Skip an ordinary embedded parser string; recognize an actual
            // bound frame or coherent occupied header as already provisioned.
            if PreparedBirthMedia::decode(&frame).is_ok()
                || (frame[8..12] == [1, 0, 0, 0]
                    && u32::from_le_bytes(frame[12..16].try_into().unwrap()) > 0)
            {
                return Err(E::SlotOccupied);
            }
            continue;
        }
        // The kernel can contain parser literals as well as the actual slot.
        // Only the marker followed by its reserved prefix padding identifies a
        // slot candidate. Descriptive string occurrences do not become slots.
        if !frame.starts_with(LEGACY_MAGIC)
            || frame[LEGACY_MAGIC.len()..24].iter().any(|b| *b != 0xff)
        {
            continue;
        }
        if frame[24..32].iter().any(|b| *b != 0) || frame[32..].iter().any(|b| *b != 0xff) {
            return Err(E::SlotOccupied);
        }
        if slot.replace(*offset).is_some() {
            return Err(E::SlotAmbiguous);
        }
    }
    slot.ok_or(E::SlotMissing)
}
fn validate_iso(file: &mut File) -> Result<(), PreparedBirthArtifactRefusal> {
    seek(file, 16 * 2048)?;
    let mut descriptor = [0; 7];
    file.read_exact(&mut descriptor)
        .map_err(|_| PreparedBirthArtifactRefusal::Io)?;
    if descriptor != [1, b'C', b'D', b'0', b'0', b'1', 1] {
        return Err(PreparedBirthArtifactRefusal::InvalidIso);
    }
    Ok(())
}
fn seek(file: &mut File, offset: u64) -> Result<(), PreparedBirthArtifactRefusal> {
    file.seek(SeekFrom::Start(offset))
        .map(|_| ())
        .map_err(|_| PreparedBirthArtifactRefusal::Io)
}
