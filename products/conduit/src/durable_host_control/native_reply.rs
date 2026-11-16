//! Internal native reply custody through serialization, actual write and drop.
//! The witness is already funded by the original producer; transport admits none.
use super::{write_frame, Response, MAXIMUM_LOCAL_CONTROL_FRAME_BYTES};
use conduit_std_host::body_execution::{ImageMemoryRetention, ImageRootPreparationOwner};
use serde::{Serialize, Serializer};
use std::io::{self, Write};
use std::{cell::RefCell, mem::size_of};

impl super::DurableHostRuntime {
    /// Transfer only an already funded witness; no active StdHost access or
    /// copied identity can manufacture another preparation budget here.
    pub(super) fn take_native_response_backing(&mut self) -> Option<ImageMemoryRetention> {
        match &mut self.host {
            super::HostSource::Body { owner, .. } => owner.take_native_response_backing(),
            super::HostSource::Bare(_) | super::HostSource::Transitioning => None,
        }
    }
}

impl super::DurableHostRuntime {
    pub(super) fn guard_native_response(&mut self, response: Response) -> GuardedResponse {
        let Some(old) = self.take_native_response_backing() else {
            return GuardedResponse::new(response, None);
        };
        let preparation = match &self.host {
            super::HostSource::Body { owner, .. } => owner.native_response_preparation().cloned(),
            _ => None,
        };
        match transfer(response, old, preparation.as_ref()) {
            Ok(response) => response,
            Err(error) => {
                // A semantic effect may already have happened: do not call this
                // pre-effect refusal, acknowledge delivery, or replay the action.
                eprintln!("native reply custody unavailable: {error}");
                GuardedResponse::new(super::refused("native-response-outcome-unavailable"), None)
            }
        }
    }
}

pub(super) struct GuardedResponse {
    // Rust drops fields in declaration order: owned response precedes its charge.
    response: Response,
    backing: Option<ImageMemoryRetention>,
}
impl GuardedResponse {
    pub(super) fn new(response: Response, backing: Option<ImageMemoryRetention>) -> Self {
        Self { response, backing }
    }
    /// Consume the actual response owner, retaining original backing through all
    /// synchronous codec work and the last write/error. Neither retries nor an
    /// acknowledgement of remote receipt are introduced here.
    pub(super) fn write(self, writer: &mut impl Write) -> Result<(), String> {
        if self.backing.is_none() {
            return write_frame(writer, &self.response);
        }
        let mut count = FrameCount(0);
        serde_json::to_writer(&mut count, &self)
            .map_err(|_| "native control response exceeds its finite encoding bound".to_string())?;
        // Count inside the original producer's funded codec workspace. Refuse
        // the existing frame ceiling BEFORE allocating the output buffer.
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(count.0)
            .map_err(|_| "native control response output backing unavailable".to_string())?;
        let mut output = ExactOutput {
            bytes: &mut bytes,
            maximum: count.0,
        };
        serde_json::to_writer(&mut output, &self)
            .map_err(|_| "native control response differs from its counted encoding".to_string())?;
        if bytes.len() != count.0 {
            return Err("native control response changed its counted encoding".into());
        }
        writer
            .write_all(&bytes)
            .map_err(|error| format!("write local control frame: {error}"))
    }
}
impl Serialize for GuardedResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.response.serialize(serializer)
    }
}
struct FrameCount(usize);
impl Write for FrameCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= MAXIMUM_LOCAL_CONTROL_FRAME_BYTES)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct ExactOutput<'a> {
    bytes: &'a mut Vec<u8>,
    maximum: usize,
}
impl Write for ExactOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.maximum)
        {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[path = "native_reply/funding.rs"]
mod funding;

fn transfer(
    response: Response,
    old: ImageMemoryRetention,
    preparation: Option<&ImageRootPreparationOwner>,
) -> Result<GuardedResponse, String> {
    let preflight = (|| {
        let owner = preparation.ok_or("native reply lacks original preparation owner")?;
        owner.verify_retention(&old)?;
        let bytes =
            funding::envelope(&response).ok_or("native reply has unsupported typed custody")?;
        Ok::<_, String>((owner, bytes))
    })();
    let (owner, bytes) = match preflight {
        Ok(value) => value,
        Err(error) => {
            drop(response);
            drop(old);
            return Err(error);
        }
    };
    // SAFETY: funding visits only the enumerated actual typed DTO owners. The
    // original stage witness funds their existing backing through transfer.
    // Independent new retention includes DTO+exact inline owners, codec scratch
    // and the enforced finite output Vec slot before any serialization. The
    // factory MOVES once into a private cell, clones no payload and has no IO,
    // mutable shared descendants, workers or authority effects. Original charge
    // remains held on both successful and failed preparation paths.
    let prepared =
        unsafe { owner.prepare_value(bytes, bytes, || Ok(RefCell::new(Some(response)))) };
    let result = match prepared {
        Ok(prepared) => {
            let response = prepared
                .value()
                .borrow_mut()
                .take()
                .ok_or("native reply transfer lost owned response")?;
            let backing = prepared.retain_memory();
            drop(prepared);
            Ok(GuardedResponse::new(response, Some(backing)))
        }
        Err(error) => Err(error),
    };
    drop(old);
    result
}

#[cfg(test)]
#[path = "native_reply/tests.rs"]
mod tests;
