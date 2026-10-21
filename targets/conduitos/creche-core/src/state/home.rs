//! Home role shares the immutable ELF, never the Draft's controller instance.
use super::*;
use crate::home::{
    Controller,
    data::{Key, View, validate_titles},
};
use alloc::string::String;
use conduit_home_model::HomeAction;
impl State {
    fn home_current(&self, revision: u32) -> Result<&Controller, Refusal> {
        let home = self.home.as_ref().ok_or(Refusal::Order)?;
        if home.revision() != revision {
            return Err(Refusal::StaleFace);
        }
        Ok(home)
    }
    fn home_publish<T: serde::Serialize>(
        &mut self,
        kind: OutputKind,
        value: &T,
    ) -> Result<Response, Refusal> {
        self.outgoing = Vec::new();
        self.read_offset = 0;
        self.outgoing = data::encode(value)?;
        Ok(Response::Blob {
            kind,
            revision: self.home.as_ref().ok_or(Refusal::Order)?.revision(),
            bytes: self.outgoing.len() as u32,
        })
    }
    pub(super) fn home_init(&mut self) -> Result<Response, Refusal> {
        if self.draft.is_some() || self.home.is_some() || self.incoming.is_some() {
            return Err(Refusal::Order);
        }
        self.home = Some(Controller::new());
        let reply = self
            .home
            .as_ref()
            .ok_or(Refusal::Order)?
            .reply(HomeAction::Unchanged);
        self.home_publish(OutputKind::Home, &reply)
    }
    pub(super) fn home_reset(&mut self, revision: u32) -> Result<Response, Refusal> {
        if self.incoming.is_some() {
            return Err(Refusal::Order);
        }
        self.home_current(revision)?;
        self.home.as_mut().ok_or(Refusal::Order)?.reset()?;
        let reply = self
            .home
            .as_ref()
            .ok_or(Refusal::Order)?
            .reply(HomeAction::Changed);
        self.home_publish(OutputKind::Home, &reply)
    }
    pub(super) fn home_refuse(&mut self, revision: u32, code: u8) -> Result<Response, Refusal> {
        if self.incoming.is_some() {
            return Err(Refusal::Order);
        }
        self.home_current(revision)?;
        self.home.as_mut().ok_or(Refusal::Order)?.refuse(code)?;
        let reply = self
            .home
            .as_ref()
            .ok_or(Refusal::Order)?
            .reply(HomeAction::Changed);
        self.home_publish(OutputKind::Home, &reply)
    }
    pub(super) fn home_key(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let input: Key = data::decode(bytes)?;
        validate_titles(&input.titles)?;
        self.home_current(input.revision)?;
        let key = conduit_human::KeyEvent::decode(&input.key).map_err(|_| Refusal::Malformed)?;
        let plots: Vec<&str> = input.titles.iter().map(String::as_str).collect();
        let action = self
            .home
            .as_mut()
            .ok_or(Refusal::Order)?
            .accept(key, &plots)?;
        let reply = self.home.as_ref().ok_or(Refusal::Order)?.reply(action);
        self.home_publish(OutputKind::Home, &reply)
    }
    pub(super) fn home_view(&mut self, bytes: &[u8]) -> Result<Response, Refusal> {
        let input: View = data::decode(bytes)?;
        validate_titles(&input.titles)?;
        if input.face_revision == 0 {
            return Err(Refusal::Bounds);
        }
        let plots: Vec<&str> = input.titles.iter().map(String::as_str).collect();
        let view = self
            .home_current(input.revision)?
            .face(input.face_revision, &plots);
        self.home_publish(OutputKind::HomeView, &view)
    }
}
