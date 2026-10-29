//! Root-only full Face endorsement and one finite read-only HTTP request slot.
//! No Face copy is retained. The remote challenge is freshness evidence only.
use crate::native_face::stream::{Snapshot, CHUNK_BYTES};
use alloc::vec::Vec;
use conduit_environment_http::face_export::{Reply, Request};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
#[path = "native_face_export_state.rs"]
mod state;
use state::State;
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static STATE: Mutex<CriticalSectionRawMutex, RefCell<State>> =
    Mutex::new(RefCell::new(State::new()));
fn request(request: Request) -> Reply {
    STATE.lock(|state| {
        state
            .borrow_mut()
            .request(request, embassy_time::Instant::now().as_millis())
    })
}
/// Only this Root child module constructs endorsement drafts, from its actual
/// retained producer. The HTTP callback cannot construct or sign one.
pub(crate) struct Draft {
    root: [u8; 32],
    payload: [u8; 156],
}
impl Draft {
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.face/export@1\0");
        hash.update(self.root);
        hash.update(self.payload);
        hash.finalize().into()
    }
}
fn write(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), &'static str> {
    if out.len().checked_add(bytes.len()).is_none_or(|n| n > 1408) {
        return Err("face-export-capacity");
    }
    out.extend_from_slice(bytes);
    Ok(())
}
fn hex(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() > 704 || out.len() + bytes.len() * 2 > 1408 {
        return Err("face-export-capacity");
    }
    crate::native_play::journal::snapshot::export::hex(out, bytes);
    Ok(())
}
fn string<T: serde::Serialize>(out: &mut Vec<u8>, value: &T) -> Result<(), &'static str> {
    crate::native_restart::payload::json::stream(value, &mut |bytes: &[u8]| {
        write(out, bytes).map_err(|_| crate::native_restart::payload::json::CodecError::Sink)
    })
    .map_err(|_| "face-export-capacity")
}
impl super::NativeRoot {
    #[inline(never)]
    pub(crate) fn advance_face_export(&mut self, now: u64) {
        // Retirement cannot depend on the reader returning or on admission
        // remaining available. Release expired response storage every Root turn.
        STATE.lock(|state| state.borrow_mut().expire(now));
        if !crate::native_restart::publication_ready() || self.body_join_scope().is_err() {
            return;
        }
        if !conduit_environment_http::face_export::available() {
            let _ = conduit_environment_http::face_export::install_provider(request);
        }
        let pending = STATE.lock(|state| state.borrow_mut().begin(now));
        let Some(pending) = pending else {
            return;
        };
        let result = self.face_export_response(&pending, now);
        STATE.lock(|state| {
            state
                .borrow_mut()
                .complete(pending, embassy_time::Instant::now().as_millis(), result)
        });
    }
    #[inline(never)]
    fn face_export_response(&self, work: &state::Work, now: u64) -> Result<Vec<u8>, &'static str> {
        if !work.is_current(embassy_time::Instant::now().as_millis()) {
            return Err("face-export-expired");
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(1408)
            .map_err(|_| "face-export-memory")?;
        let reader = self
            .face_reader
            .as_ref()
            .ok_or("face-export-no-producer")?
            .lease();
        match work.request() {
            Request::Snapshot { nonce } => {
                let (_, boot, body, root) = self.body_join_scope()?;
                let (snapshot, until) = reader.with_current(now, |face, status| {
                    if !status.publisher_retained || status.refusal.is_some() {
                        return Err("face-export-unavailable");
                    }
                    if face.basis.body_id.as_ref() != Some(body) {
                        return Err("face-export-producer-scope");
                    }
                    let mut until = now.checked_add(5000).ok_or("face-export-clock")?;
                    use conduit_face::FacePropertyValue;
                    let property = |name| {
                        face.properties
                            .iter()
                            .find(|p| p.subject == "environment" && p.name == name)
                            .map(|p| &p.value)
                    };
                    if matches!(property("fresh"), Some(FacePropertyValue::Flag(true))) {
                        let Some(FacePropertyValue::Count(observed)) =
                            property("observed-at-boot-ms")
                        else {
                            return Err("face-export-observation-clock");
                        };
                        let (_, cadence) = crate::native_software::source_facts();
                        let lifetime = cadence
                            .period_ms
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(1))
                            .ok_or("face-export-clock")?;
                        until =
                            until.min(observed.checked_add(lifetime).ok_or("face-export-clock")?);
                    }
                    if until <= now {
                        return Err("face-export-expired");
                    }
                    Ok((
                        crate::native_face::stream::snapshot(face)
                            .map_err(|_| "face-export-capacity")?,
                        until,
                    ))
                })??;
                let mut payload = [0; 156];
                payload[..32].copy_from_slice(&nonce);
                payload[32..64].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
                payload[64..96].copy_from_slice(&Sha256::digest(boot.as_str().as_bytes()));
                payload[96..128].copy_from_slice(&snapshot.digest);
                payload[128..136].copy_from_slice(&snapshot.revision.to_le_bytes());
                payload[136..140].copy_from_slice(&snapshot.bytes.to_le_bytes());
                payload[140..148].copy_from_slice(&now.to_le_bytes());
                payload[148..156].copy_from_slice(&until.to_le_bytes());
                let draft = Draft { root, payload };
                let signature = work
                    .while_current(embassy_time::Instant::now().as_millis(), || {
                        self.admission.sign_face_export(&draft)
                    })?;
                let mut signed = [0; 220];
                signed[..156].copy_from_slice(&draft.payload);
                signed[156..].copy_from_slice(&signature);
                write(
                    &mut output,
                    b"{\"profile\":\"conduit.face/export@1\",\"producer_boot\":",
                )?;
                string(&mut output, boot)?;
                write(&mut output, b",\"body_id\":")?;
                string(&mut output, body)?;
                write(&mut output, b",\"offer\":\"")?;
                hex(&mut output, &signed)?;
                write(&mut output, b"\"}")?;
            }
            Request::Chunk {
                digest,
                revision_le,
                bytes,
                offset,
            } => {
                let mut chunk = [0; CHUNK_BYTES];
                let length = reader.export_chunk(
                    now,
                    Snapshot {
                        digest,
                        revision: u64::from_le_bytes(revision_le),
                        bytes,
                    },
                    offset,
                    &mut chunk,
                )?;
                write(
                    &mut output,
                    b"{\"profile\":\"conduit.face/chunk@1\",\"digest\":\"",
                )?;
                hex(&mut output, &digest)?;
                write(&mut output, b"\",\"offset\":")?;
                if output.len() + 10 > 1408 {
                    return Err("face-export-capacity");
                }
                crate::native_play::journal::snapshot::export::decimal(&mut output, offset);
                write(&mut output, b",\"bytes\":\"")?;
                hex(&mut output, &chunk[..length])?;
                write(&mut output, b"\"}")?;
            }
        }
        Ok(output)
    }
}
