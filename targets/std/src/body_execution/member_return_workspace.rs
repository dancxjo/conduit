//! Original-counter raw capture for member Return. This owns framing only:
//! parsing, typed metadata, Core, journal/compiler and descendants need their
//! own qualified reservations. No public byte-grant or Hosted fallback exists.
use crate::{kernel_preparation::ImageMemoryLease, StdHost};
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read};

pub const MEMBER_RETURN_FRAME_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum MemberReturnFrameRefusal {
    NoOriginalImage,
    Unavailable,
    WrongOriginalOwner,
    Allocation,
    Extent,
    Read(ErrorKind),
}

pub struct MemberReturnFrame {
    bytes: Vec<u8>,
    scope: [u8; 32],
    // Raw storage drops before its original counter charge.
    memory: ImageMemoryLease,
}

/// The borrowed raw extent cannot outlive the charged buffer. Every return,
/// read refusal and unwind scrubs the complete initialized buffer on drop.
pub struct CapturedMemberReturnFrame<'a> {
    frame: &'a mut MemberReturnFrame,
    length: usize,
}

impl StdHost {
    pub fn prepare_member_return_frame(
        &self,
    ) -> Result<MemberReturnFrame, MemberReturnFrameRefusal> {
        let scope = scope(self)?;
        self.kernel_resources
            .require_owner_identity_preparation()
            .map_err(|_| MemberReturnFrameRefusal::Unavailable)?;
        let memory = self
            .kernel_resources
            .fund_tick_preparation(backing_bytes()?)
            .map_err(|_| MemberReturnFrameRefusal::Unavailable)?;
        // The original reservation exists before the first buffer allocation.
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(MEMBER_RETURN_FRAME_BYTES + 1)
            .map_err(|_| MemberReturnFrameRefusal::Allocation)?;
        bytes.resize(MEMBER_RETURN_FRAME_BYTES + 1, 0);
        Ok(MemberReturnFrame {
            bytes,
            scope,
            memory,
        })
    }
}

fn backing_bytes() -> Result<usize, MemberReturnFrameRefusal> {
    core::mem::size_of::<MemberReturnFrame>()
        .checked_add(MEMBER_RETURN_FRAME_BYTES + 1)
        .and_then(|n| n.checked_add(ImageMemoryLease::owned_backing_bytes()))
        .ok_or(MemberReturnFrameRefusal::Unavailable)
}

fn scope(host: &StdHost) -> Result<[u8; 32], MemberReturnFrameRefusal> {
    let image = host
        .image_identity()
        .ok_or(MemberReturnFrameRefusal::NoOriginalImage)?;
    let actual = host.advertisement();
    if image.host_id != actual.host_id
        || image.boot_id != actual.boot_id
        || image.offer_generation != actual.offer_generation
    {
        return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
    }
    let mut hash = Sha256::new();
    for text in [
        &image.profile_id,
        &image.build_id,
        &image.image_id,
        &image.host_id.0,
        &image.boot_id.0,
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.update(image.offer_generation.0.to_le_bytes());
    Ok(hash.finalize().into())
}

impl MemberReturnFrame {
    pub fn capture<'a>(
        &'a mut self,
        host: &StdHost,
        reader: &mut impl Read,
    ) -> Result<CapturedMemberReturnFrame<'a>, MemberReturnFrameRefusal> {
        // Counter identity precedes all reader effects, even for equal labels.
        if !host
            .kernel_resources
            .member_return_frame_owner_is_current(&self.memory)
        {
            return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
        }
        if scope(host)? != self.scope {
            return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
        }
        self.bytes.fill(0);
        let mut captured = CapturedMemberReturnFrame {
            frame: self,
            length: 0,
        };
        while captured.length < captured.frame.bytes.len() {
            let remaining = &mut captured.frame.bytes[captured.length..];
            match reader.read(remaining) {
                Ok(0) => break,
                Ok(count) if count <= remaining.len() => captured.length += count,
                Ok(_) => return Err(MemberReturnFrameRefusal::Read(ErrorKind::InvalidData)),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(MemberReturnFrameRefusal::Read(error.kind())),
            }
        }
        if captured.length == 0 || captured.length > MEMBER_RETURN_FRAME_BYTES {
            return Err(MemberReturnFrameRefusal::Extent);
        }
        Ok(captured)
    }
}
impl CapturedMemberReturnFrame<'_> {
    /// Raw bytes only; this is not permission for parser or typed allocations.
    pub fn as_bytes(&self) -> &[u8] {
        &self.frame.bytes[..self.length]
    }
}
impl Drop for CapturedMemberReturnFrame<'_> {
    fn drop(&mut self) {
        self.frame.bytes.fill(0);
    }
}
impl Drop for MemberReturnFrame {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

#[cfg(test)]
mod tests;
