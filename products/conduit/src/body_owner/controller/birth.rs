//! Adopt one reviewed Crèche Birth into the installed owner's biography.
//!
//! Preparation does not make another Body. The service journals the prepared
//! biography before it transfers its one live Host into `Owner`.

use super::{state, Owner, OwnerHost};
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{bind_sign, HostAdvertisement};
use conduit_std_host::StdHost;
use std::path::Path;

pub(crate) struct PreparedBirth {
    session: BodyLifecycleSession,
    resident: Option<ResidentPlot>,
}

impl PreparedBirth {
    pub(crate) fn retain(&self, root: &Path, primary_source: Option<&[u8]>) -> Result<(), String> {
        state::retain_with_source(root, self.session.evidence(), None, None, primary_source)
    }

    pub(crate) fn body_id(&self) -> &conduit_body::BodyId {
        &self.session.evidence().body_id
    }
}

impl Owner {
    pub(crate) fn prepare_born(
        body: Body,
        friendly_name: String,
        advertised: &HostAdvertisement,
    ) -> Result<PreparedBirth, String> {
        let admitted_sequence = body
            .birth_sequence
            .checked_add(1)
            .ok_or("Birth membership sequence exhausted")?;
        let present_sequence = admitted_sequence
            .checked_add(1)
            .ok_or("Birth membership sequence exhausted")?;
        let part = PartId::bind(&body.body_id, advertised.host_id.as_str(), 0)
            .map_err(|error| format!("{error:?}"))?;
        let proof = MembershipProofId::bind("conduit/installed-host/creche-birth")
            .map_err(|error| format!("{error:?}"))?;
        let membership =
            BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let evidence = BodyBiographyEvidence::born(body.clone(), membership, friendly_name)
            .map_err(|error| format!("{error:?}"))?;
        let session = BodyLifecycleSession::prepare_first_host(
            &evidence,
            part,
            AuthenticatedHostObservation {
                host_id: advertised.host_id.clone(),
                boot_id: advertised.boot_id.clone(),
                offer_generation: advertised.offer_generation,
                proof_id: proof,
                sequence: 0,
            },
            bind_sign(
                &advertised.host_id,
                &advertised.boot_id,
                None,
                admitted_sequence,
            )
            .sign_id,
            bind_sign(
                &advertised.host_id,
                &advertised.boot_id,
                None,
                present_sequence,
            )
            .sign_id,
        )
        .map_err(|error| format!("{error:?}"))?;
        Ok(PreparedBirth {
            resident: body.workset.plots().first().cloned(),
            session,
        })
    }

    pub(crate) fn from_prepared_born(host: StdHost, prepared: PreparedBirth) -> Self {
        Self {
            prepared_admission_route: None,
            member_carrier_identity: None,
            host: OwnerHost::new(host)?,
            session: prepared.session,
            resident: prepared.resident,
            resident_name: None,
            clock_interval_ms: None,
            last_execution: None,
            execution_persistence_pending: false,
            workset_recovery_required: false,
            admissions: None,
            pending_browser: None,
            pending_native_mask: None,
            pending_member_return: None,
            invited_mask_offers: Default::default(),
            attached_terminal_route: None,
            direct_spoken_route: None,
            direct_spoken_template: None,
            llm_spoken_route: None,
            face_wardrobe: None,
            thermostat_live: None,
            todo_live: None,
            todo_verified: None,
            notebook: None,
        }
    }
}
