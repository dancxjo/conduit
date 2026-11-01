//! Running public entrance actions over canonical model-owned Body truth.

use super::{PatchbayHtmlServer, ServerError};
use crate::front_door::{snapshot_for_front_door, snapshot_for_zero_body_front_door};
use conduit_core::SignId;
use conduit_patchbay_workbench::{PlotCandidate, ZeroBodyFrontDoor};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex};

impl PatchbayHtmlServer {
    pub fn bind_front_door(address: SocketAddr) -> Result<Self, ServerError> {
        Self::bind_front_door_with_plots(address, Vec::new())
    }

    pub fn bind_front_door_with_plots(
        address: SocketAddr,
        plots: Vec<PlotCandidate>,
    ) -> Result<Self, ServerError> {
        let mut session =
            ZeroBodyFrontDoor::fresh(std::sync::Arc::new(patchbay_hosted::HostedPatchbayAdapter))
                .map_err(ServerError::Interaction)?;
        for plot in plots {
            session.add_plot(plot).map_err(ServerError::Interaction)?;
        }
        let snapshot =
            snapshot_for_zero_body_front_door(&session).map_err(ServerError::Interaction)?;
        let mut server = Self::bind(address, &snapshot)?;
        server.zero_body_front_door = Some(Arc::new(Mutex::new(session)));
        Ok(server)
    }

    pub fn bind_front_door_ephemeral() -> Result<Self, ServerError> {
        Self::bind_front_door(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into())
    }

    pub fn bind_front_door_with_plots_ephemeral(
        plots: Vec<PlotCandidate>,
    ) -> Result<Self, ServerError> {
        Self::bind_front_door_with_plots(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(), plots)
    }

    pub(super) fn refresh_front_door(&mut self) -> Result<(), ServerError> {
        if let Some(session) = &self.zero_body_front_door {
            let session = session
                .lock()
                .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?;
            // Every canonical front-door mutation advances this revision.
            // Polling unchanged truth must not rebuild the checked graph and
            // catalog ahead of queued human interactions.
            let host = session.advertisement();
            let host_subject = format!("host/{}/{}", host.host_id.as_str(), host.boot_id.as_str());
            if self.snapshot.face.basis.body_id.is_none()
                && self.snapshot.face.subjects.iter().any(|subject| {
                    subject.role == conduit_face::FaceRole::Host && subject.identity == host_subject
                })
                && session.revision() == self.snapshot.face.revision
            {
                return Ok(());
            }
            let projection = session.project().map_err(ServerError::Interaction)?;
            if projection.face.revision == self.snapshot.face.revision
                && projection.face.identity == self.snapshot.face.identity
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
                .update(&snapshot.face)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
            snapshot.entrance = entrance;
            snapshot.interaction = prior_interaction;
            snapshot.interaction.selected_subject = snapshot.entrance.selected_subject.clone();
            drop(session);
            self.snapshot = snapshot;
            self.reset_navigation()?;
            self.encoded_snapshot = self.snapshot.encode()?;
            return Ok(());
        }
        let session = self
            .front_door
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("front-door session is absent".into()))?
            .lock()
            .map_err(|_| ServerError::Interaction("front-door session lock failed".into()))?;
        // Birth/Join can replace the owner with an equal revision. Fence this
        // shortcut by Body identity as well as the owner's revision.
        if self.snapshot.face.basis.body_id.as_ref() == Some(&session.body().body_id)
            && session.revision() == self.snapshot.face.revision
        {
            return Ok(());
        }
        let projection = session.project().map_err(ServerError::Interaction)?;
        if projection.face.revision == self.snapshot.face.revision
            && projection.face.identity == self.snapshot.face.identity
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
        let mut entrance = if prior_entrance.body_id == snapshot.face.basis.body_id {
            prior_entrance
        } else {
            patchbay_application::PatchbayEntranceState::enter(&snapshot.face)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?
        };
        if entrance.face_id != snapshot.face.identity.as_str() {
            entrance
                .update(&snapshot.face)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        }
        snapshot.entrance = entrance;
        snapshot.interaction = prior_interaction;
        snapshot.interaction.selected_subject = snapshot.entrance.selected_subject.clone();
        drop(session);
        self.snapshot = snapshot;
        self.reset_navigation()?;
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(())
    }
}
