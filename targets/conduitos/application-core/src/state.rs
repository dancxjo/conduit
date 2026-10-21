//! Bounded transport staging; all retained application semantics are Models.
use crate::{
    MAX_BLOB,
    data::{self, Command, Refusal, Reply},
    models::Models,
    wire::{Request, Response},
};
use alloc::vec::Vec;
#[derive(Default)]
pub struct State {
    models: Option<Models>,
    incoming: Option<(usize, Vec<u8>)>,
    outgoing: Vec<u8>,
    read: usize,
}
impl State {
    pub fn execute(&mut self, request: Request, transfer: &mut [u8]) -> Result<Response, Refusal> {
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
                if offset as usize != incoming.len()
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
                let reply = match command {
                    Command::ProjectTutorial { evidence, scope } => {
                        scope.validate(&evidence)?;
                        let view = crate::tutorial::project(&evidence, scope.lulled)?;
                        Reply::Tutorial {
                            scope,
                            view: view.encode().map_err(|_| Refusal::Application)?,
                        }
                    }
                    Command::Init { applications } => {
                        if self.models.is_some() {
                            return Err(Refusal::Order);
                        }
                        self.models = Some(Models::new(applications)?);
                        Reply::Initialized
                    }
                    command => self
                        .models
                        .as_mut()
                        .ok_or(Refusal::Order)?
                        .execute(command)?,
                };
                drop(incoming);
                self.outgoing = data::encode(&reply)?;
                self.read = 0;
                Ok(Response::Blob {
                    bytes: self.outgoing.len() as u32,
                })
            }
            Request::Read { offset, maximum } => {
                if offset as usize != self.read || self.read >= self.outgoing.len() {
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
