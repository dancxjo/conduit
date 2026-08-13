//! Running public entrance actions over canonical model-owned Body truth.

use super::{PatchbayHtmlServer, ServerError};
use crate::front_door::{snapshot_for_front_door, snapshot_for_zero_body_front_door};
use conduit_core::SignId;
use patchbay_model::ZeroBodyFrontDoor;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};

impl PatchbayHtmlServer {
    pub fn bind_front_door(address: SocketAddr) -> Result<Self, ServerError> {
        let session = ZeroBodyFrontDoor::fresh().map_err(ServerError::Interaction)?;
        let snapshot =
            snapshot_for_zero_body_front_door(&session).map_err(ServerError::Interaction)?;
        let mut server = Self::bind(address, &snapshot)?;
        server.zero_body_front_door = Some(Arc::new(Mutex::new(session)));
        Ok(server)
    }

    pub fn bind_front_door_ephemeral() -> Result<Self, ServerError> {
        Self::bind_front_door(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into())
    }

    pub(super) fn refresh_front_door(&mut self) -> Result<(), ServerError> {
        if let Some(session) = &self.zero_body_front_door {
            let session = session
                .lock()
                .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?;
            let projection = session.project().map_err(ServerError::Interaction)?;
            if projection.presentation.revision == self.snapshot.presentation.revision
                && projection.presentation.identity == self.snapshot.presentation.identity
            {
                return Ok(());
            }
            let prior_entrance = self.snapshot.entrance.clone();
            let prior_interaction = self.snapshot.interaction.clone();
            let mut snapshot =
                snapshot_for_zero_body_front_door(&session).map_err(ServerError::Interaction)?;
            snapshot.mark_available(SignId::from(format!(
                "patchbay-html/front-door/revision-{}/available",
                snapshot.revision
            )))?;
            let mut entrance = prior_entrance;
            entrance
                .update(&snapshot.presentation)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
            snapshot.entrance = entrance;
            snapshot.interaction = prior_interaction;
            snapshot.interaction.selected_subject = snapshot.entrance.selected_subject.clone();
            self.snapshot = snapshot;
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(());
        }
        let session = self
            .front_door
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("front-door session is absent".into()))?
            .lock()
            .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?;
        let projection = session.project().map_err(ServerError::Interaction)?;
        if projection.presentation.revision == self.snapshot.presentation.revision
            && projection.presentation.identity == self.snapshot.presentation.identity
        {
            return Ok(());
        }
        let prior_entrance = self.snapshot.entrance.clone();
        let prior_interaction = self.snapshot.interaction.clone();
        let mut snapshot = snapshot_for_front_door(&session).map_err(ServerError::Interaction)?;
        snapshot.mark_available(SignId::from(format!(
            "patchbay-html/front-door/revision-{}/available",
            snapshot.revision
        )))?;
        let mut entrance = if prior_entrance.body_id == snapshot.presentation.basis.body_id {
            prior_entrance
        } else {
            patchbay_model::PatchbayEntranceState::enter(&snapshot.presentation)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?
        };
        if entrance.presentation_id != snapshot.presentation.identity.as_str() {
            entrance
                .update(&snapshot.presentation)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        }
        snapshot.entrance = entrance;
        snapshot.interaction = prior_interaction;
        snapshot.interaction.selected_subject = snapshot.entrance.selected_subject.clone();
        self.snapshot = snapshot;
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(())
    }
}
