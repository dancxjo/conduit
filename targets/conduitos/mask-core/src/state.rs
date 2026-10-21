//! Bounded transport staging around the one retained Workspace Mask reader.
use crate::{
    MAX_BLOB,
    data::{self, Command, Refusal, Reply},
    reader::Reader,
    wire::{Request, Response},
};
use alloc::vec::Vec;
#[derive(Default)]
pub struct State {
    reader: Reader,
    control: crate::control::Controller,
    incoming: Option<(usize, Vec<u8>)>,
    outgoing: Vec<u8>,
    read: usize,
}
impl State {
    pub fn execute(&mut self, request: Request, transfer: &mut [u8]) -> Result<Response, Refusal> {
        if transfer.len() != crate::memory::TRANSFER_BYTES {
            return Err(Refusal::Bounds);
        }
        match request {
            Request::Begin { bytes } => {
                if self.incoming.is_some() || bytes == 0 || bytes as usize > MAX_BLOB {
                    return Err(Refusal::Order);
                }
                let mut incoming = Vec::new();
                incoming
                    .try_reserve_exact(bytes as usize)
                    .map_err(|_| Refusal::Capacity)?;
                self.incoming = Some((bytes as usize, incoming));
                self.outgoing = Vec::new();
                self.read = 0;
                Ok(Response::Ready)
            }
            Request::Write { offset, bytes } => {
                let (total, incoming) = self.incoming.as_mut().ok_or(Refusal::Order)?;
                if bytes == 0
                    || bytes as usize > crate::memory::TRANSFER_BYTES
                    || offset as usize != incoming.len()
                    || incoming
                        .len()
                        .checked_add(bytes as usize)
                        .is_none_or(|end| end > *total)
                {
                    return Err(Refusal::Order);
                }
                incoming.extend_from_slice(transfer.get(..bytes as usize).ok_or(Refusal::Bounds)?);
                Ok(Response::Ready)
            }
            Request::Commit => {
                let (total, incoming) = self.incoming.take().ok_or(Refusal::Order)?;
                if incoming.len() != total {
                    return Err(Refusal::Order);
                }
                let command: Command = data::decode(&incoming)?;
                let output = match command {
                    Command::Control {
                        expected,
                        scope,
                        operation,
                    } => {
                        let (changed, snapshot) =
                            self.control.execute(scope, expected, operation)?;
                        data::encode(&Reply::Control { changed, snapshot })?
                    }
                    Command::Update {
                        expected,
                        mut update,
                    } => {
                        let scope = crate::control::Scope {
                            session: update.scope.session,
                            host: update.scope.host.clone(),
                            boot: update.scope.boot.clone(),
                        };
                        // The retained controller is authoritative for this mode;
                        // Root's observational copy cannot replace guest policy.
                        update.facts.details_open = self.control.view(&scope)?.exact_details_open;
                        data::encode(&Reply::Updated {
                            snapshot: self.reader.update(expected, update)?,
                        })?
                    }
                    Command::Navigate { expected, key } => {
                        let (changed, snapshot) = self.reader.navigate(expected, key)?;
                        data::encode(&Reply::Navigated { changed, snapshot })?
                    }
                    Command::Reset { expected } => data::encode(&Reply::Updated {
                        snapshot: self.reader.reset(expected)?,
                    })?,
                    Command::Selection { expected } => data::encode(&Reply::Selected {
                        snapshot: self.reader.selection(expected)?,
                    })?,
                    Command::Frame { expected } => {
                        let (receipt, scene) = self.reader.frame(expected)?;
                        data::encode_frame(&receipt, scene)?
                    }
                };
                drop(incoming);
                self.outgoing = output;
                self.read = 0;
                Ok(Response::Blob {
                    bytes: self.outgoing.len() as u32,
                })
            }
            Request::Read { offset, maximum } => {
                if self.incoming.is_some()
                    || maximum == 0
                    || maximum as usize > crate::memory::TRANSFER_BYTES
                    || offset as usize != self.read
                    || self.read >= self.outgoing.len()
                {
                    return Err(Refusal::Order);
                }
                let bytes = (maximum as usize).min(self.outgoing.len() - self.read);
                transfer.fill(0);
                transfer
                    .get_mut(..bytes)
                    .ok_or(Refusal::Bounds)?
                    .copy_from_slice(&self.outgoing[self.read..self.read + bytes]);
                self.read += bytes;
                Ok(Response::Chunk {
                    offset,
                    bytes: bytes as u32,
                    total: self.outgoing.len() as u32,
                })
            }
        }
    }
}
