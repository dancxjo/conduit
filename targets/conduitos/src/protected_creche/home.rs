//! Home transport owns an independent Home-profile service lease.
use super::*;
use alloc::string::String;
use conduit_home_model::HomeAction;
use conduit_human::KeyEvent;
use conduitos_creche_core::home::data::{Key, Reply, Snapshot, View, validate_titles};
use core::cell::RefCell;
pub(crate) struct Home {
    service: RefCell<CrecheService>,
    projection: Snapshot,
}
impl Home {
    pub(crate) fn try_new() -> Result<Self, crate::front_door::Error> {
        Self::install().map_err(|_| crate::front_door::Error::Face)
    }
    fn install() -> Result<Self, TransportError> {
        #[cfg(conduitos_creche_image)]
        {
            let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/creche.elf"));
            let image = ServiceImage::for_profile(bytes, ServiceProfile::Home)
                .map_err(|_| TransportError::Protocol)?;
            let domain = ServiceDomain::install(&image).map_err(TransportError::Backend)?;
            let mut service = CrecheService::from_domain(domain);
            let response = service.request(Request::HomeInit)?;
            let output = service.blob(response, OutputKind::Home, 1)?;
            let reply: Reply = decode(&mut service, &output)?;
            if reply.snapshot.revision != 1
                || reply.action(0).map_err(|_| TransportError::Protocol)? != HomeAction::Unchanged
            {
                service.quarantine();
                return Err(TransportError::Protocol);
            }
            Ok(Self {
                service: RefCell::new(service),
                projection: reply.snapshot,
            })
        }
        #[cfg(not(conduitos_creche_image))]
        {
            Err(TransportError::Backend(DomainRefusal::Unsupported))
        }
    }
    pub(crate) fn reset(&mut self) -> Result<(), crate::front_door::Error> {
        let result = self
            .service
            .get_mut()
            .request(Request::HomeReset {
                revision: self.projection.revision,
            })
            .and_then(|reply| self.commit_reply(reply, 0));
        if result.is_err() {
            self.service.get_mut().quarantine();
            return Err(crate::front_door::Error::Face);
        }
        Ok(())
    }
    pub(crate) fn is_live(&self) -> bool {
        self.service
            .try_borrow()
            .is_ok_and(|service| service.is_live())
    }
    pub(crate) fn model(&self) -> &Snapshot {
        &self.projection
    }
    pub(crate) fn refusal(&self) -> Option<&str> {
        if self.is_live() {
            self.projection.refusal.as_deref()
        } else {
            None
        }
    }
    pub(crate) fn accept_checked(
        &mut self,
        event: KeyEvent,
        titles: &[&str],
    ) -> Result<HomeAction, crate::front_door::Error> {
        let plots = titles.len();
        let result = (|| {
            let titles = owned_titles(titles)?;
            let bytes = encode(&Key {
                revision: self.projection.revision,
                key: event.encode(),
                titles,
            })?;
            let response = self.service.get_mut().input(InputKind::HomeKey, &bytes)?;
            self.commit_reply(response, plots)
        })();
        match result {
            Ok(action) => Ok(action),
            Err(_) => {
                self.service.get_mut().quarantine();
                Err(crate::front_door::Error::Face)
            }
        }
    }
    fn commit_reply(
        &mut self,
        response: Response,
        plots: usize,
    ) -> Result<HomeAction, TransportError> {
        let Response::Blob {
            kind: OutputKind::Home,
            revision,
            ..
        } = response
        else {
            return Err(TransportError::Protocol);
        };
        let previous = self.projection.revision;
        let bytes = self
            .service
            .get_mut()
            .blob(response, OutputKind::Home, revision)?;
        let reply: Reply = decode(self.service.get_mut(), &bytes)?;
        let action = reply.action(plots).map_err(|_| TransportError::Protocol)?;
        let expected = if action == HomeAction::Unchanged {
            Some(previous)
        } else {
            previous.checked_add(1)
        };
        if Some(revision) != expected || reply.snapshot.revision != revision {
            return Err(TransportError::Protocol);
        }
        self.projection = reply.snapshot;
        Ok(action)
    }
    pub(crate) fn record_result(
        &mut self,
        _result: &HomeAction,
        absent: Option<&str>,
    ) -> Result<(), crate::front_door::Error> {
        if let Some(title) = absent {
            let code = match title {
                "Tour" => 1,
                "Patchbay" => 2,
                _ => return Err(crate::front_door::Error::Face),
            };
            let result = self
                .service
                .get_mut()
                .request(Request::HomeRefusal {
                    revision: self.projection.revision,
                    code,
                })
                .and_then(|reply| self.commit_reply(reply, 0));
            if result.is_err() {
                self.service.get_mut().quarantine();
                return Err(crate::front_door::Error::Face);
            }
        }
        Ok(())
    }
    pub(crate) fn application_view(
        &self,
        revision: u32,
        titles: &[&str],
    ) -> Result<conduit_face::ApplicationView, crate::front_door::Error> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| crate::front_door::Error::Face)?;
        let result = (|| {
            let bytes = encode(&View {
                revision: self.projection.revision,
                face_revision: revision,
                titles: owned_titles(titles)?,
            })?;
            let response = service.input(InputKind::HomeView, &bytes)?;
            let output = service.blob(response, OutputKind::HomeView, self.projection.revision)?;
            let view: conduit_face::SemanticApplicationView = decode(&mut service, &output)?;
            if view.revision != revision {
                return Err(TransportError::Protocol);
            }
            view.lower().map_err(|_| TransportError::Protocol)
        })();
        if result.is_err() {
            service.quarantine();
        }
        result.map_err(|_| crate::front_door::Error::Face)
    }
}
fn owned_titles(titles: &[&str]) -> Result<Vec<String>, TransportError> {
    let titles: Vec<String> = titles.iter().map(|s| String::from(*s)).collect();
    validate_titles(&titles).map_err(|_| TransportError::Bounds)?;
    Ok(titles)
}
fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, TransportError> {
    conduitos_creche_core::data::encode(value).map_err(|_| TransportError::Bounds)
}
fn decode<T: serde::de::DeserializeOwned>(
    service: &mut CrecheService,
    bytes: &[u8],
) -> Result<T, TransportError> {
    conduitos_creche_core::data::decode(bytes).map_err(|_| {
        service.quarantine();
        TransportError::Protocol
    })
}
