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
        let mut membership =
            BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let mut evidence =
            BodyBiographyEvidence::born(body.clone(), membership.clone(), friendly_name)
                .map_err(|error| format!("{error:?}"))?;
        let admitted = membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                proof.clone(),
                bind_sign(
                    &advertised.host_id,
                    &advertised.boot_id,
                    None,
                    admitted_sequence,
                )
                .sign_id,
            )
            .map_err(|error| format!("{error:?}"))?;
        let present = membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
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
                    present_sequence,
                )
                .sign_id,
            )
            .map_err(|error| format!("{error:?}"))?;
        evidence
            .append_membership_events(
                membership,
                &[(admitted, admitted_sequence), (present, present_sequence)],
            )
            .map_err(|error| format!("{error:?}"))?;
        let session = BodyLifecycleSession::open(evidence).map_err(|error| format!("{error:?}"))?;
        Ok(PreparedBirth {
            resident: body.workset.plots().first().cloned(),
            session,
        })
    }

    pub(crate) fn from_prepared_born(host: StdHost, prepared: PreparedBirth) -> Self {
        Self {
            host: OwnerHost::new(host),
            session: prepared.session,
            resident: prepared.resident,
            resident_name: None,
            clock_interval_ms: None,
            last_execution: None,
            admissions: None,
            pending_browser: None,
            pending_native_mask: None,
            presentation_wardrobe: None,
        }
    }
}
