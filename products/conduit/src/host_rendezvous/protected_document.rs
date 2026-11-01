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
}
struct Bytes(Vec<u8>);
impl Drop for Bytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

impl<C: ProtectedFrameCarrier> ProtectedDocumentLine<C> {
    pub(crate) fn new(inner: ProtectedCarrier<C>, timeout_millis: u32) -> Result<Self, String> {
        if timeout_millis == 0 || timeout_millis > 300_000 {
            return Err("invalid admitted document deadline".into());
        }
        Ok(Self {
            inner,
            timeout: Duration::from_millis(u64::from(timeout_millis)),
            failed: false,
        })
    }
    fn deadline(&self) -> Result<Instant, String> {
        Instant::now()
            .checked_add(self.timeout)
            .ok_or_else(|| "document deadline overflow".into())
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
        let mut frame = Bytes(
            self.inner
                .receive()
                .map_err(|error| format!("protected document receive:{error:?}"))?
                .to_vec(),
        );
        if !frame.0.starts_with(b"CBD1") {
            return Ok(core::mem::take(&mut frame.0));
        }
        let deadline = self.deadline()?;
        if frame.0.len() <= BOUNDED_DOCUMENT_HEADER_BYTES {
            return Err("document chunk header".into());
        }
        let total = u32::from_le_bytes(frame.0[4..8].try_into().unwrap()) as usize;
        if total == 0 || total > MAXIMUM_FRAME_BYTES {
            return Err("document exceeds existing rendezvous bound".into());
        }
        let capacity = self
            .payload()
            .checked_sub(BOUNDED_DOCUMENT_HEADER_BYTES)
            .filter(|n| *n > 0)
            .ok_or("document carrier payload too small")?;
        let maximum_chunks = MAXIMUM_FRAME_BYTES.div_ceil(capacity);
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
            frame.0 = self
                .inner
                .receive_with_timeout_millis(remaining)
                .map_err(|error| format!("protected document receive:{error:?}"))?
                .to_vec();
        }
        Err("document chunk pressure".into())
    }
    fn send_document(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_FRAME_BYTES {
            return Err("document exceeds existing rendezvous bound".into());
        }
        let deadline = self.deadline()?;
        if bytes.len() <= self.payload() {
            self.inner
                .send_with_timeout_millis(bytes, Self::remaining(deadline)?)
                .map_err(|error| format!("bounded timed protected send required:{error:?}"))?;
            Self::remaining(deadline)?;
            return Ok(());
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
        if result.is_err() {
            self.failed = true;
            let _ = self.inner.close();
        }
        result
    }
}
impl<C: ProtectedFrameCarrier> ProtectedDocumentLine<C> {
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
        self.failed = true;
        self.inner
            .close()
            .map_err(|error| format!("close document Line:{error:?}"))
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
        let _ = self.inner.close();
    }
}
