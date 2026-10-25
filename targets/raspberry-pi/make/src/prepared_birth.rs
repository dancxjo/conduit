//! Inert retained-birth transport for real B+ raw kernels and FAT32 SD images.
//! No normal ARM product consumes this section yet. These receipts cannot prove
//! Host admission, selected Plot execution, boot or membership.
use conduit_body::{PreparedBirthMedia, PREPARED_BIRTH_MEDIA_BYTES};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};
mod fat;

pub const SLOT_HEADER_BYTES: usize = 32;
pub const MAX_KERNEL_BYTES: usize = 16 * 1024 * 1024;
const SLOT_MAGIC: &[u8] = b"CONDUIT_RPI_BIRTH_SLOT@1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Bounds,
    Binding,
    SlotMissing,
    SlotAmbiguous,
    SlotOccupied,
    CorruptMedia,
    InvalidFat,
    KernelMismatch,
    Io,
}

/// These identities must come from the exact compiled normal product's make
/// record. A caller passing invented strings has not established that product.
pub struct CompiledKernelBinding<'a> {
    pub profile_id: &'a str,
    pub build_id: &'a str,
    pub base_kernel_sha256: &'a str,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelBirthReceipt {
    pub body_id: String,
    pub host_entry: String,
    pub frame_offset: usize,
    pub kernel_bytes: usize,
    pub kernel_sha256: String,
    pub ordinary_product_consumer_verified: bool,
    pub boot_observed: bool,
    pub membership_observed: bool,
}

/// Bind only the linker-reserved pristine slot in the caller-owned raw image.
/// Validation is complete before mutation; no birth occurs here.
pub fn bind_kernel(
    kernel: &mut [u8],
    media: &PreparedBirthMedia,
    binding: CompiledKernelBinding<'_>,
) -> Result<KernelBirthReceipt, Refusal> {
    if kernel.is_empty() || kernel.len() > MAX_KERNEL_BYTES {
        return Err(Refusal::Bounds);
    }
    if media.binding().target != crate::B_PLUS_TARGET
        || media.binding().profile_id != binding.profile_id
        || media.binding().build_id != binding.build_id
        || digest(kernel) != binding.base_kernel_sha256
    {
        return Err(Refusal::Binding);
    }
    let mut found = None;
    for (offset, window) in kernel.windows(SLOT_HEADER_BYTES).enumerate() {
        if window.starts_with(SLOT_MAGIC)
            && window[SLOT_MAGIC.len()..].iter().all(|byte| *byte == 0)
            && found.replace(offset + SLOT_HEADER_BYTES).is_some()
        {
            return Err(Refusal::SlotAmbiguous);
        }
    }
    let offset = found.ok_or(Refusal::SlotMissing)?;
    let slot = kernel
        .get(
            offset
                ..offset
                    .checked_add(PREPARED_BIRTH_MEDIA_BYTES)
                    .ok_or(Refusal::Bounds)?,
        )
        .ok_or(Refusal::Bounds)?;
    if slot.iter().any(|byte| *byte != 0) {
        return Err(Refusal::SlotOccupied);
    }
    let mut frame = [0; PREPARED_BIRTH_MEDIA_BYTES];
    media
        .encode_into(&mut frame)
        .map_err(|_| Refusal::CorruptMedia)?;
    if PreparedBirthMedia::decode(&frame).map_err(|_| Refusal::CorruptMedia)? != *media {
        return Err(Refusal::CorruptMedia);
    }
    kernel[offset..offset + frame.len()].copy_from_slice(&frame);
    Ok(KernelBirthReceipt {
        body_id: media.evidence().body_id.as_str().into(),
        host_entry: media.binding().host_entry.clone(),
        frame_offset: offset,
        kernel_bytes: kernel.len(),
        kernel_sha256: digest(kernel),
        ordinary_product_consumer_verified: false,
        boot_observed: false,
        membership_observed: false,
    })
}

/// Reopen the actual KERNEL.IMG directory entry from an MBR/FAT32 image, follow
/// its finite cluster chain, compare all bytes, then independently decode birth.
/// This never executes the kernel or upgrades the receipt's consumer claim.
pub fn verify_sd_kernel(
    sd: &mut File,
    expected_kernel: &[u8],
    media: &PreparedBirthMedia,
    receipt: &KernelBirthReceipt,
) -> Result<(), Refusal> {
    if expected_kernel.len() != receipt.kernel_bytes
        || expected_kernel.len() > MAX_KERNEL_BYTES
        || digest(expected_kernel) != receipt.kernel_sha256
        || receipt.body_id != media.evidence().body_id.as_str()
        || receipt.host_entry != media.binding().host_entry
    {
        return Err(Refusal::Binding);
    }
    let frame = expected_kernel
        .get(
            receipt.frame_offset
                ..receipt
                    .frame_offset
                    .checked_add(PREPARED_BIRTH_MEDIA_BYTES)
                    .ok_or(Refusal::Bounds)?,
        )
        .ok_or(Refusal::Bounds)?;
    if PreparedBirthMedia::decode(frame).map_err(|_| Refusal::CorruptMedia)? != *media {
        return Err(Refusal::CorruptMedia);
    }
    fat::verify_kernel(sd, expected_kernel)
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn read_at(file: &mut File, offset: u64, bytes: &mut [u8]) -> Result<(), Refusal> {
    let end = offset
        .checked_add(bytes.len() as u64)
        .ok_or(Refusal::Bounds)?;
    if end > file.metadata().map_err(|_| Refusal::Io)?.len() {
        return Err(Refusal::Bounds);
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| Refusal::Io)?;
    file.read_exact(bytes).map_err(|_| Refusal::Io)
}
#[cfg(test)]
mod tests;
