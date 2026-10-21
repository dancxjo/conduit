//! Bounded transport staging; all retained application semantics are Models.
use crate::{
    MAX_BLOB,
    data::{self, Command, Refusal, Reply},
    models::{Builder, Models},
    wire::{Request, Response},
};
use alloc::vec::Vec;
#[derive(Default)]
pub struct State {
    models: Option<Models>,
    refused: bool,
    preparing: Option<Builder>,
    workspace: Option<data::Workspace>,
    projection: Option<crate::graph_facts::PreparedProjection>,
    pending_target: Option<(u8, u8, data::Target)>,
    incoming: Option<(usize, Vec<u8>)>,
    preflighted: Option<usize>,
    outgoing: Vec<u8>,
    read: usize,
}
impl State {
    pub fn execute(&mut self, request: Request, transfer: &mut [u8]) -> Result<Response, Refusal> {
        if self.refused {
            return Err(Refusal::Order);
        }
        let result = self.execute_admitted(request, transfer);
        if result.is_err() {
            self.refused = true;
            self.models = None;
            self.preparing = None;
            self.projection = None;
            self.pending_target = None;
            self.preflighted = None;
            self.workspace = None;
            self.incoming = None;
            self.outgoing = Vec::new();
        }
        result
    }
    fn execute_admitted(
        &mut self,
        request: Request,
        transfer: &mut [u8],
    ) -> Result<Response, Refusal> {
        match request {
            Request::Begin { bytes } => {
                if self.incoming.is_some()
                    || bytes == 0
                    || bytes as usize > MAX_BLOB
                    || (self.projection.is_some() && self.read != self.outgoing.len())
                    || (self.pending_target.is_some()
                        && (bytes > 256 || self.read != self.outgoing.len()))
                {
                    return Err(Refusal::Order);
                }
                if self.workspace.is_none() {
                    let workspace = data::Workspace::new()?;
                    self.outgoing
                        .try_reserve_exact(MAX_BLOB)
                        .map_err(|_| Refusal::Capacity)?;
                    self.workspace = Some(workspace);
                }
                self.outgoing.clear();
                self.read = 0;
                self.preflighted = None;
                let mut incoming = Vec::new();
                incoming
                    .try_reserve_exact(bytes as usize)
                    .map_err(|_| Refusal::Capacity)?;
                self.incoming = Some((bytes as usize, incoming));
                Ok(Response::Ready)
            }
            Request::Write { offset, bytes } => {
                if self.preflighted.is_some() {
                    return Err(Refusal::Order);
                }
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
            Request::Preflight => {
                if self.preflighted.is_some()
                    || self.projection.is_some()
                    || self.pending_target.is_some()
                {
                    return Err(Refusal::Order);
                }
                let (total, incoming) = self.incoming.as_ref().ok_or(Refusal::Order)?;
                if incoming.len() != *total {
                    return Err(Refusal::Order);
                }
                let workspace = self.workspace.as_mut().ok_or(Refusal::Order)?;
                self.preflighted = Some(workspace.preflight_command(incoming)?);
                Ok(Response::Ready)
            }
            Request::Commit => {
                let (total, incoming) = self.incoming.take().ok_or(Refusal::Order)?;
                if incoming.len() != total {
                    return Err(Refusal::Order);
                }
                let workspace = self.workspace.as_mut().ok_or(Refusal::Order)?;
                let command: Command = if self.projection.is_some() {
                    workspace.decode_projection_finish(&incoming)?
                } else if let Some(length) = self.preflighted.take() {
                    workspace.decode_preflighted_command(length)?
                } else {
                    workspace.decode_command(&incoming)?
                };
                drop(incoming);
                let mut initialized = None;
                if self.preparing.is_some()
                    && !matches!(
                        &command,
                        Command::PrepareTarget { .. }
                            | Command::FinishTarget { .. }
                            | Command::PrepareTutorial { .. }
                            | Command::PrepareTour { .. }
                            | Command::FinishInit
                    )
                {
                    return Err(Refusal::Order);
                }
                if self.pending_target.is_some()
                    && !matches!(&command, Command::FinishTarget { .. })
                {
                    return Err(Refusal::Order);
                }
                if self.projection.is_some() && !matches!(&command, Command::FinishGraphProjection)
                {
                    return Err(Refusal::Order);
                }
                let reply = match command {
                    Command::PrepareGraphProjection { graph, scope } => {
                        self.projection =
                            Some(crate::graph_facts::PreparedProjection::new(graph, scope)?);
                        Reply::Prepared
                    }
                    Command::FinishGraphProjection => Reply::GraphFacts {
                        contribution: self
                            .projection
                            .take()
                            .ok_or(Refusal::Order)?
                            .finish(workspace, &mut self.outgoing)?,
                    },
                    Command::ProjectTutorial { evidence, scope } => {
                        scope.validate(&evidence)?;
                        let view = crate::tutorial::project(&evidence, scope.lulled)?;
                        Reply::Tutorial {
                            scope,
                            view: view.encode().map_err(|_| Refusal::Application)?,
                        }
                    }
                    Command::PrepareInit { manifest } => {
                        if self.models.is_some() || self.preparing.is_some() {
                            return Err(Refusal::Order);
                        }
                        self.preparing = Some(Builder::new(manifest)?);
                        Reply::Prepared
                    }
                    Command::PrepareTarget {
                        slot,
                        target,
                        value,
                    } => {
                        self.preparing
                            .as_ref()
                            .ok_or(Refusal::Order)?
                            .validate_target(slot, target)?;
                        self.pending_target = Some((slot, target, value));
                        Reply::Prepared
                    }
                    Command::FinishTarget { slot, target } => {
                        if self
                            .pending_target
                            .as_ref()
                            .is_none_or(|(s, t, _)| *s != slot || *t != target)
                        {
                            return Err(Refusal::Order);
                        }
                        let (_, _, value) = self.pending_target.take().ok_or(Refusal::Order)?;
                        self.preparing
                            .as_mut()
                            .ok_or(Refusal::Order)?
                            .target(slot, target, value)?;
                        Reply::Prepared
                    }
                    Command::PrepareTutorial { slot, evidence } => {
                        self.preparing
                            .as_mut()
                            .ok_or(Refusal::Order)?
                            .tutorial(slot, evidence)?;
                        Reply::Prepared
                    }
                    Command::PrepareTour { slot } => {
                        self.preparing.as_mut().ok_or(Refusal::Order)?.tour(slot)?;
                        Reply::Prepared
                    }
                    Command::FinishInit => {
                        initialized = Some(self.preparing.take().ok_or(Refusal::Order)?.finish()?);
                        Reply::Initialized
                    }
                    command => self
                        .models
                        .as_mut()
                        .ok_or(Refusal::Order)?
                        .execute(command)?,
                };
                workspace.encode_reply_into(&reply, &mut self.outgoing)?;
                if let Some(models) = initialized {
                    self.models = Some(models);
                }
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
#[cfg(test)]
mod tests;
