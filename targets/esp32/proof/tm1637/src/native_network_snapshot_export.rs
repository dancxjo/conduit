//! Finite read-only lease of an exact signed offer and its pinned physical owner.
use super::Draft;
use alloc::vec::Vec;
use conduit_environment_http::network_custody::{Reply, Request};
use ed25519_dalek::{Signature, VerifyingKey};
pub(crate) const REPLY_BYTES: usize = 1408;
pub(crate) struct Export {
    draft: Draft,
    signed: [u8; 240],
    id: [u8; 32],
    deadline: u64,
}
impl Export {
    pub(crate) fn new(
        draft: Draft,
        signed: [u8; 240],
        deadline: u64,
    ) -> Result<Self, &'static str> {
        if signed[..176] != draft.payload {
            return Err("network-export-offer-mismatch");
        }
        let id = draft.transcript();
        VerifyingKey::from_bytes(draft.root())
            .map_err(|_| "network-export-root")?
            .verify_strict(
                &id,
                &Signature::from_bytes(signed[176..].try_into().unwrap()),
            )
            .map_err(|_| "network-export-signature")?;
        Ok(Self {
            draft,
            signed,
            id,
            deadline,
        })
    }
    pub(crate) fn expired(&self, now: u64) -> bool {
        now >= self.deadline
    }
    pub(crate) fn reply(&mut self, request: Request, now: u64) -> Reply {
        if self.expired(now) {
            return Reply::Refused;
        }
        let mut out = Vec::new();
        if out.try_reserve_exact(REPLY_BYTES).is_err() {
            return Reply::Unavailable;
        }
        match request {
            Request::Offer => {
                out.extend_from_slice(
                    b"{\"profile\":\"network-retirement-offer@1\",\"snapshot\":\"",
                );
                hex(&mut out, &self.id);
                out.extend_from_slice(b"\",\"offer\":\"");
                hex(&mut out, &self.signed);
                out.extend_from_slice(b"\"}");
            }
            Request::Chunk { snapshot, offset } => {
                if snapshot != self.id || offset >= 32768 || offset % 512 != 0 {
                    return Reply::Refused;
                }
                let mut bytes = [0; 512];
                if self.draft._owner.read_chunk(offset, &mut bytes).is_err() {
                    return Reply::Unavailable;
                }
                out.extend_from_slice(
                    b"{\"profile\":\"network-retirement-chunk@1\",\"snapshot\":\"",
                );
                hex(&mut out, &self.id);
                out.extend_from_slice(b"\",\"offset\":");
                decimal(&mut out, offset);
                out.extend_from_slice(b",\"bytes\":\"");
                hex(&mut out, &bytes);
                out.extend_from_slice(b"\"}");
            }
        }
        debug_assert!(out.len() <= REPLY_BYTES);
        Reply::Offer(out)
    }
}
fn hex(out: &mut Vec<u8>, bytes: &[u8]) {
    for b in bytes {
        out.push(b"0123456789abcdef"[(b >> 4) as usize]);
        out.push(b"0123456789abcdef"[(b & 15) as usize]);
    }
}
fn decimal(out: &mut Vec<u8>, mut value: u32) {
    let mut bytes = [0; 10];
    let mut at = bytes.len();
    loop {
        at -= 1;
        bytes[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.extend_from_slice(&bytes[at..]);
}
