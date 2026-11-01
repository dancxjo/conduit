//! Existing CBD1 documents above one retained authenticated protected Line.
//! Reassembly is transport custody only; ordinary JSON admission remains owner.
use super::{RendezvousLine, MAXIMUM_FRAME_BYTES};
use conduit_protected_line::{ProtectedCarrier, ProtectedFrameCarrier};
use conduit_wire::{
    encode_bounded_document_chunk, BoundedDocumentAssembly, BOUNDED_DOCUMENT_HEADER_BYTES,
};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

pub(crate) struct ProtectedDocumentLine<C: ProtectedFrameCarrier> {
    inner: ProtectedCarrier<C>,
    timeout: Duration,
    failed: bool,
    workspace: Option<DocumentWorkspace>,
    encounter_deadline: Option<Instant>,
}
/// Local finite storage selection, not authority. The owning bridge must derive
/// it independently from its actual admitted workspace, never traffic quotas.
#[derive(Clone, Copy)]
pub(crate) struct DocumentWorkspace {
    pub(crate) maximum_document_bytes: usize,
    pub(crate) capacity_bytes: usize,
}
struct Bytes(Vec<u8>);
impl Drop for Bytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl<C: ProtectedFrameCarrier> ProtectedDocumentLine<C> {
    pub(crate) fn new(
        inner: ProtectedCarrier<C>,
        timeout_millis: u32,
        workspace: Option<DocumentWorkspace>,
    ) -> Result<Self, String> {
        if timeout_millis == 0 || timeout_millis > 300_000 {
            return Err("invalid admitted document deadline".into());
        }
        if workspace.is_some_and(|space| {
            space.maximum_document_bytes == 0
                || space.maximum_document_bytes > MAXIMUM_FRAME_BYTES
                || space.capacity_bytes == 0
        }) {
            return Err("invalid selected document workspace".into());
        }
        Ok(Self {
            inner,
            timeout: Duration::from_millis(u64::from(timeout_millis)),
            failed: false,
            workspace,
            encounter_deadline: None,
        })
    }
    fn deadline(&self) -> Result<Instant, String> {
        let operation = Instant::now()
            .checked_add(self.timeout)
            .ok_or("document deadline overflow")?;
        Ok(self
            .encounter_deadline
            .map_or(operation, |end| end.min(operation)))
    }
    fn remaining(deadline: Instant) -> Result<u32, String> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("document deadline elapsed")?;
        let millis = remaining.as_millis();
        if millis == 0 {
            return Err("document deadline elapsed".into());
        }
        u32::try_from(millis).map_err(|_| "document deadline capacity".into())
    }
    fn payload(&self) -> usize {
        self.inner.session().limits().maximum_payload_bytes as usize
    }
    fn receive_document(&mut self) -> Result<Vec<u8>, String> {
        let deadline = self.deadline()?;
        let mut frame = Bytes(Vec::with_capacity(self.payload()));
        frame.0.extend_from_slice(
            self.inner
                .receive_with_timeout_millis(Self::remaining(deadline)?)
                .map_err(|error| format!("protected document receive:{error:?}"))?,
        );
        Self::remaining(deadline)?;
        if !frame.0.starts_with(b"CBD1") {
            if self
                .workspace
                .is_some_and(|space| frame.0.len() > space.maximum_document_bytes)
            {
                return Err("document exceeds selected logical bound".into());
            }
            return Ok(core::mem::take(&mut frame.0));
        }
        let workspace = self
            .workspace
            .ok_or("bounded document requires selected workspace")?;
        if frame.0.len() <= BOUNDED_DOCUMENT_HEADER_BYTES {
            return Err("document chunk header".into());
        }
        let total = u32::from_le_bytes(frame.0[4..8].try_into().unwrap()) as usize;
        if total == 0 || total > workspace.maximum_document_bytes {
            return Err("document exceeds existing rendezvous bound".into());
        }
        let capacity = self
            .payload()
            .checked_sub(BOUNDED_DOCUMENT_HEADER_BYTES)
            .filter(|n| *n > 0)
            .ok_or("document carrier payload too small")?;
        let maximum_chunks = workspace.maximum_document_bytes.div_ceil(capacity);
        if total
            .checked_add(self.payload())
            .is_none_or(|bytes| bytes > workspace.capacity_bytes)
        {
            return Err("selected document workspace capacity exceeded".into());
        }
        // Allocate only the validated declared extent, never a second full copy.
        let mut assembly = BoundedDocumentAssembly::new(total, maximum_chunks);
        for _ in 0..maximum_chunks {
            Self::remaining(deadline)?;
            let complete = assembly
                .push(&frame.0)
                .map_err(|error| format!("document chunk:{error:?}"))?;
            frame.0.fill(0);
            if complete {
                let (bytes, digest) = assembly.completed().ok_or("document incomplete")?;
                if Sha256::digest(bytes).as_slice() != digest {
                    return Err("document digest mismatch".into());
                }
                Self::remaining(deadline)?;
                return assembly
                    .into_completed()
                    .map(|(bytes, _)| bytes)
                    .ok_or_else(|| "document incomplete".into());
            }
            let remaining = Self::remaining(deadline)?;
            frame.0.clear();
            frame.0.extend_from_slice(
                self.inner
                    .receive_with_timeout_millis(remaining)
                    .map_err(|error| format!("protected document receive:{error:?}"))?,
            );
        }
        Err("document chunk pressure".into())
    }
    fn send_document(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_FRAME_BYTES {
            return Err("document exceeds existing rendezvous bound".into());
        }
        let deadline = self.deadline()?;
        if self
            .workspace
            .is_some_and(|space| bytes.len() > space.maximum_document_bytes)
        {
            return Err("document exceeds selected logical bound".into());
        }
        if bytes.len() <= self.payload() {
            self.inner
                .send_with_timeout_millis(bytes, Self::remaining(deadline)?)
                .map_err(|error| format!("bounded timed protected send required:{error:?}"))?;
            Self::remaining(deadline)?;
            return Ok(());
        }
        let workspace = self
            .workspace
            .ok_or("bounded document requires selected workspace")?;
        if self.payload() > workspace.capacity_bytes {
            return Err("selected document workspace capacity exceeded".into());
        }
        if self.payload() <= BOUNDED_DOCUMENT_HEADER_BYTES {
            return Err("document carrier payload too small".into());
        }
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let mut frame = Bytes(vec![0; self.payload()]);
        let mut offset = 0;
        while offset < bytes.len() {
            let remaining = Self::remaining(deadline)?;
            let (length, next) =
                encode_bounded_document_chunk(&mut frame.0, bytes, offset, &digest)
                    .map_err(|error| format!("document chunk encode:{error:?}"))?;
            self.inner
                .send_with_timeout_millis(&frame.0[..length], remaining)
                .map_err(|error| format!("bounded timed protected send required:{error:?}"))?;
            offset = next;
        }
        Self::remaining(deadline)?;
        Ok(())
    }
    fn finish<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = result {
            self.failed = true;
            // The operation deadline is exhausted/refused. Bound retirement
            // rather than waiting for a fresh idle window or reusing the Line.
            return match self.inner.close_with_timeout_millis(1) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(format!("{error}; bounded retirement:{cleanup:?}")),
            };
        }
        result
    }
}
impl<C: ProtectedFrameCarrier> ProtectedDocumentLine<C> {
    /// Preflight the fixed read-only response profile against actual selected
    /// document storage and remaining protected send quotas, without enlargement.
    pub(crate) fn require_document_capacity(&self, bytes: usize) -> Result<(), String> {
        let payload = self.payload();
        if self.failed
            || bytes == 0
            || bytes > MAXIMUM_FRAME_BYTES
            || self
                .workspace
                .is_some_and(|w| bytes > w.maximum_document_bytes)
        {
            return Err("native snapshot document profile not selected".into());
        }
        let chunks = if bytes <= payload {
            1
        } else {
            let w = self
                .workspace
                .ok_or("native snapshot document workspace missing")?;
            if payload <= BOUNDED_DOCUMENT_HEADER_BYTES || w.capacity_bytes < bytes + payload {
                return Err("native snapshot document workspace insufficient".into());
            }
            bytes.div_ceil(payload - BOUNDED_DOCUMENT_HEADER_BYTES)
        };
        let overhead = if chunks > 1 {
            chunks * BOUNDED_DOCUMENT_HEADER_BYTES
        } else {
            0
        };
        let e = self.evidence();
        if e.sent_frames + chunks as u64 > e.limits.maximum_frames_per_direction
            || e.sent_bytes + (bytes + overhead) as u64 > e.limits.maximum_bytes_per_direction
        {
            return Err("native snapshot protected traffic profile insufficient".into());
        }
        Ok(())
    }
    pub(crate) fn evidence(&self) -> conduit_protected_line::SessionEvidence<'_> {
        self.inner.session().evidence()
    }
    /// Tighten the whole encounter; individual document deadlines cannot reset it.
    pub(crate) fn restrict_deadline(&mut self, deadline: Instant) -> Result<(), String> {
        if deadline <= Instant::now() {
            return Err("encounter deadline elapsed".into());
        }
        self.encounter_deadline = Some(
            self.encounter_deadline
                .map_or(deadline, |old| old.min(deadline)),
        );
        Ok(())
    }
    pub(crate) fn receive(&mut self) -> Result<Vec<u8>, String> {
        if self.failed {
            return Err("document Line is retired".into());
        }
        let result = self.receive_document();
        self.finish(result)
    }
    pub(crate) fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.failed {
            return Err("document Line is retired".into());
        }
        let result = self.send_document(bytes);
        self.finish(result)
    }
    pub(crate) fn close(&mut self) -> Result<(), String> {
        if self.failed {
            return Ok(());
        }
        self.failed = true;
        self.inner
            .close_with_timeout_millis(1)
            .map_err(|error| format!("bounded close document Line:{error:?}"))
    }
}
impl<C: ProtectedFrameCarrier> RendezvousLine for ProtectedDocumentLine<C> {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        Self::receive(self)
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        Self::send(self, bytes)
    }
    fn close(&mut self) -> Result<(), String> {
        Self::close(self)
    }
}
impl<C: ProtectedFrameCarrier> Drop for ProtectedDocumentLine<C> {
    fn drop(&mut self) {
        if !self.failed {
            let _ = self.close();
        }
    }
}
