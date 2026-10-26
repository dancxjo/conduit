//! A single owned input transaction, retained until its existing Commit.
use crate::{
    data::{self, InteractionRequest},
    wire::{InputKind, Refusal, TRANSFER_BYTES},
};
use alloc::vec::Vec;
pub(super) struct Incoming {
    pub kind: InputKind,
    pub total: u32,
    pub bytes: Vec<u8>,
    pub prepared: Option<Result<InteractionRequest, Refusal>>,
}
impl Incoming {
    pub fn new(kind: InputKind, total: u32) -> Result<Self, Refusal> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total as usize)
            .map_err(|_| Refusal::Capacity)?;
        Ok(Self {
            kind,
            total,
            bytes,
            prepared: None,
        })
    }
    pub fn write(
        &mut self,
        offset: u32,
        window_offset: u32,
        bytes: u32,
        transfer: &[u8],
    ) -> Result<(), Refusal> {
        let end = offset
            .checked_add(bytes)
            .filter(|n| *n <= self.total)
            .ok_or(Refusal::Bounds)?;
        let window_end = window_offset
            .checked_add(bytes)
            .filter(|n| *n <= TRANSFER_BYTES)
            .ok_or(Refusal::Bounds)?;
        if bytes == 0 || offset as usize != self.bytes.len() {
            return Err(Refusal::Order);
        }
        self.bytes
            .extend_from_slice(&transfer[window_offset as usize..window_end as usize]);
        if self.bytes.len() != end as usize {
            return Err(Refusal::Bounds);
        }
        if self.kind == InputKind::FaceInteraction && end == self.total {
            // This is useful, pure preparation of the now-complete owned input,
            // under this Write's unchanged domain budget. Neither acceptance nor
            // failure is published until Commit; raw bytes stay owned as before.
            self.prepared = Some(data::decode_interaction(&self.bytes));
        }
        Ok(())
    }
}
