//! One foreground presentation Boot from a fresh explicitly installed Host.
//! Retained selected equipment stays in its real runtime for the whole encounter.
use super::{
    bounded_read, current_time_millis, owner_lock, read_installation, write_bytes_atomic,
    write_json_atomic, RuntimeStatus, MAXIMUM_BODY_ADMISSION_BYTES,
};
use conduit_body::{
    PortableAdmissionReceipt, PortableInvitation, PortableSpawnAdmissionRequest,
    SpawnInvitationSecret, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct PresentationHost {
    pub(crate) runtime: crate::durable_host_control::DurableHostRuntime,
    state: PathBuf,
    status: RuntimeStatus,
    _host_owner: owner_lock::OwnershipLock,
    _body_owner: owner_lock::OwnershipLock,
}

impl PresentationHost {
    pub(crate) fn prepare(state: &Path) -> Result<Self, String> {
        let host_owner = owner_lock::acquire(state)?;
        let body_owner = owner_lock::body(state)?;
        let installation = read_installation(&state.join("installation.json"))?;
        if installation.body_state.is_some() || installation.joined_body_state.is_some() {
            return Err(
                "presentation requires a fresh installed Host without Body membership".into(),
            );
        }
        let (status, runtime) = super::runtime_preparation::prepare_presentation_runtime(state)?;
        Ok(Self {
            runtime,
            state: state.into(),
            status,
            _host_owner: host_owner,
            _body_owner: body_owner,
        })
    }

    /// Sign the actual offer only after all foreground providers are attached.
    /// No reconstructed bare Host can stand in for selected terminal or speech.
    pub(crate) fn prepare_join(
        &mut self,
        invitation: PortableInvitation,
    ) -> Result<PortableSpawnAdmissionRequest, String> {
        invitation
            .validate(current_time_millis()?)
            .map_err(|error| error.to_string())?;
        let advertisement = self.runtime.truth().advertisement;
        if advertisement.host_id.as_str() != self.status.host_id
            || advertisement.boot_id.as_str() != self.status.boot_id
        {
            return Err("presentation Host differs from its installed Boot".into());
        }
        self.status.offer_generation = advertisement.offer_generation.0;
        write_json_atomic(&self.state.join("runtime.json"), &self.status)?;
        let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret)
            .map_err(|error| format!("presentation invitation: {error:?}"))?;
        let signature = secret.sign(&invitation.claim.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        ));
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: advertisement,
            nonce: invitation.claim.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        request.validate().map_err(|error| error.to_string())?;
        let pending = super::invitation::PendingBodyJoin {
            schema: "conduit.body/pending-join@1".into(),
            invitation,
            request: request.clone(),
        };
        let bytes = serde_json::to_vec(&pending).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAXIMUM_BODY_ADMISSION_BYTES {
            return Err("presentation admission exceeds its finite file bound".into());
        }
        let body = self.state.join("body");
        fs::create_dir_all(&body).map_err(|error| error.to_string())?;
        super::restrict_directory(&body)?;
        write_bytes_atomic(&body.join("pending-join.json"), &bytes)?;
        Ok(request)
    }

    pub(crate) fn retain_join(&self, receipt: PortableAdmissionReceipt) -> Result<(), String> {
        super::membership::complete_body_join_document(receipt, &self.state, true)
    }
}

impl Drop for PresentationHost {
    fn drop(&mut self) {
        let path = self.state.join("runtime.json");
        let current = bounded_read(&path, 64 * 1024)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RuntimeStatus>(&bytes).ok());
        if current.is_some_and(|current| {
            current.host_id == self.status.host_id && current.boot_id == self.status.boot_id
        }) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(unix)]
#[path = "durable_host_presentation/terminal.rs"]
mod terminal;
#[cfg(unix)]
pub(crate) use terminal::run;
