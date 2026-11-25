//! One private issuer reservation, before any provider job or delivery exists.
use super::*;
use conduit_body::SpawnInvitation;
use conduit_host_make::CheckedHostConfiguration;
use serde::Deserialize;
use sha2::{Digest, Sha256};
mod binding;
pub(crate) use binding::bind;
pub(crate) use binding::prepared_request;
pub(super) use binding::{confirm_started, request_for_started};
mod browser_release;
mod host_job;
mod resource;
pub(super) use host_job::{begin_job, complete_job, fail_job};
pub(super) use resource::observe;
mod delivery;
mod provider;
mod seed;
pub(super) use delivery::{begin_delivery, release_after_admission};
mod staging;
#[cfg(test)]
pub(crate) mod test_release;
use conduit_body_make::stem_runtime::StemOutcome;
pub(crate) use provider::{record_kernel_outcome, validate_completion};
pub(super) use provider::{result, run, ProviderCall};
pub(super) use staging::{stage, Stage};

#[derive(Deserialize)]
#[serde(transparent)]
struct SecretInput(Vec<u8>);
impl Drop for SecretInput {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reservation {
    target_id: String,
    host_id: HostId,
    boot_id: BootId,
    secret: SecretInput,
    nonce: [u8; 32],
    now_millis: u64,
    expires_at_millis: u64,
    selection: crate::creche::BrowserConfigurationSelection,
    expected_release_digest: String,
    #[serde(default)]
    release_pin: Option<staging::ReleasePin>,
}

struct Provision {
    id: [u8; 32],
    host: HostId,
    boot: BootId,
    invitation: SpawnInvitation,
    configuration: CheckedHostConfiguration,
    expected_release_digest: String,
    release_pin: Option<staging::ReleasePin>,
    material: Option<staging::Material>,
    binding: Option<binding::Binding>,
    provider_outcome: Option<StemOutcome>,
    provider_attempted: bool,
    delivery_committed: bool,
    target_job_pending: bool,
    target_job_sequence: Option<u32>,
    target_job_failure: Option<conduit_body_make::stem_runtime::StemDisposition>,
    kernel_outcome: Option<StemOutcome>,
    provider_evidence: Vec<u8>,
}
thread_local! {
    static PROVISION: RefCell<Option<Provision>> = const { RefCell::new(None) };
}

fn require_issuer(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
) -> Result<(), Refusal> {
    BIRTH_ISSUER.with(|slot| {
        if slot
            .borrow()
            .as_ref()
            .is_some_and(|custody| custody.matches(body.evidence(), host, boot))
        {
            Ok(())
        } else {
            Err(Refusal::new(
                "Stem.Denied",
                "This Boot has no retained Birth issuer custody",
            ))
        }
    })
}

impl Provision {
    fn response(&self) -> Result<Vec<u8>, Refusal> {
        encode(&serde_json::json!({
            "schema": "conduit.stem/reservation@1", "disposition": "reserved",
            "reservation_id": self.id, "body_id": self.invitation.body_id,
            "configuration_id": self.configuration.configuration_id(),
            "target_id": self.configuration.profile().target.key(),
            "expected_release_digest": self.expected_release_digest,
            "checked": crate::creche::stem_configuration_review(&self.configuration).map_err(|error|Refusal::new("Stem.Configuration",error))?,
            "artifact_prepared": false, "invitation_delivered": false,
            "membership_claimed": false,
        }))
    }
}

pub(super) fn reserve(
    body: &BodyLifecycleSession,
    request: Reservation,
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, &request.host_id, &request.boot_id)?;
    if body.evidence().body.state != BodyState::Lulled {
        return Err(Refusal::new(
            "Stem.NotLulled",
            "Reserve provisioning before proposing the Stem Play",
        ));
    }
    if request.target_id != "browser/wasm32/page" {
        return Err(Refusal::new(
            "Stem.UnsupportedTarget",
            "This selected provider prepares browser page Hosts only",
        ));
    }
    crate::plot_runner::workspace::require_empty()?;
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(Refusal::new(
                "Stem.Busy",
                "One private Stem provision is already reserved",
            ));
        }
        let digest = request.expected_release_digest.as_bytes();
        if digest.len() != 71
            || &digest[..7] != b"sha256:"
            || !digest[7..].iter().all(u8::is_ascii_hexdigit)
            || digest[7..].iter().any(u8::is_ascii_uppercase)
        {
            return Err(Refusal::new(
                "Stem.InvalidSelection",
                "Select one exact reviewed release digest",
            ));
        }
        let configuration = crate::creche::check_stem_configuration(request.selection)
            .map_err(|error| Refusal::new("Stem.InvalidSelection", error))?;
        let secret = <[u8; 32]>::try_from(request.secret.0.as_slice()).map_err(|_| {
            Refusal::new(
                "Stem.InvalidSelection",
                "Invitation entropy must have 32 bytes",
            )
        })?;
        let secret = SpawnInvitationSecret::from_csprng_bytes(secret).map_err(|error| {
            Refusal::new(
                &format!("Admission.{error:?}"),
                "Invitation entropy refused",
            )
        })?;
        ADMISSIONS.with(|manager| {
            let mut manager = manager.borrow_mut();
            let manager = manager
                .as_mut()
                .ok_or("Workspace admission state is missing")?;
            if manager.body_id != body.evidence().body_id {
                return Err(Refusal::new(
                    "Stem.Denied",
                    "Issuer manager names another Body",
                ));
            }
            let invitation = manager
                .issue_spawn_invitation(
                    secret,
                    request.nonce,
                    request.now_millis,
                    request.expires_at_millis,
                )
                .map_err(|error| {
                    Refusal::new(
                        &format!("Admission.{error:?}"),
                        "Invitation issuance refused",
                    )
                })?;
            let identity = serde_json::to_vec(&(
                "conduit.stem/private-reservation@1",
                &request.target_id,
                &request.host_id,
                &request.boot_id,
                invitation.claim(),
                invitation.issued_at_millis,
                configuration.configuration_id(),
                &request.expected_release_digest,
                &request.release_pin,
            ));
            let identity = match identity {
                Ok(identity) => identity,
                Err(error) => {
                    manager
                        .revoke_pending_spawn_invitation(&invitation)
                        .map_err(|failure| {
                            Refusal::new(
                                &format!("Stem.Cleanup.{failure:?}"),
                                "Owned invitation cleanup refused",
                            )
                        })?;
                    return Err(Refusal::new("Stem.Encode", error.to_string()));
                }
            };
            let provision = Provision {
                id: Sha256::digest(identity).into(),
                host: request.host_id,
                boot: request.boot_id,
                invitation,
                configuration,
                expected_release_digest: request.expected_release_digest,
                release_pin: request.release_pin,
                material: None,
                binding: None,
                provider_outcome: None,
                provider_attempted: false,
                delivery_committed: false,
                target_job_pending: false,
                target_job_sequence: None,
                target_job_failure: None,
                kernel_outcome: None,
                provider_evidence: Vec::with_capacity(4096),
            };
            match provision.response() {
                Ok(response) => {
                    *slot = Some(provision);
                    Ok(response)
                }
                Err(error) => {
                    manager
                        .revoke_pending_spawn_invitation(&provision.invitation)
                        .map_err(|failure| {
                            Refusal::new(
                                &format!("Stem.Cleanup.{failure:?}"),
                                "Owned invitation cleanup refused",
                            )
                        })?;
                    Err(error)
                }
            }
        })
    })
}

pub(crate) fn scoped_resource(host: &HostId, boot: &BootId) -> Option<conduit_core::ResourceOffer> {
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        let provision = slot.as_ref()?;
        if &provision.host != host
            || &provision.boot != boot
            || provision
                .binding
                .as_ref()
                .is_some_and(|binding| binding.play.is_some())
        {
            return None;
        }
        let owned = BIRTH_ISSUER.with(|custody| {
            custody.borrow().as_ref().is_some_and(|custody| {
                custody.matches_identity(&provision.invitation.body_id, host, boot)
            })
        });
        owned.then(|| provision.resource())
    })
}

impl Provision {
    fn resource(&self) -> conduit_core::ResourceOffer {
        let pool = self
            .id
            .iter()
            .fold(String::from("browser/stem/"), |mut pool, byte| {
                use core::fmt::Write;
                write!(&mut pool, "{byte:02x}").expect("write String");
                pool
            });
        conduit_core::resource_offer(&pool, crate::installed_browser::stem::RESOURCE, 1)
    }
}

pub(crate) fn has_scoped_offer() -> bool {
    PROVISION.with(|slot| {
        let slot = slot.borrow();
        slot.as_ref().is_some_and(|provision| {
            if provision
                .binding
                .as_ref()
                .is_some_and(|binding| binding.play.is_some())
            {
                return false;
            }
            BIRTH_ISSUER.with(|custody| {
                custody.borrow().as_ref().is_some_and(|custody| {
                    custody.matches_identity(
                        &provision.invitation.body_id,
                        &provision.host,
                        &provision.boot,
                    )
                })
            })
        })
    })
}

pub(crate) fn augment_advertisement(host: &mut HostAdvertisement) {
    if let Some(resource) = scoped_resource(&host.host_id, &host.boot_id) {
        let offer = crate::installed_browser::stem::offer();
        host.capabilities
            .retain(|prior| prior.capability_id != offer.capability_id);
        host.resources
            .retain(|prior| prior.pool_id != resource.pool_id);
        host.capabilities.push(offer);
        host.resources.push(resource);
        host.capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    }
}

pub(super) fn cancel(
    body: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    id: [u8; 32],
) -> Result<Vec<u8>, Refusal> {
    require_issuer(body, host, boot)?;
    // Synchronous provider work has returned before this owner can reborrow.
    // Retire the live kernel before discarding a private candidate.
    crate::plot_runner::workspace::require_empty()
        .map_err(|error| Refusal::new("Stem.DrainRequired", error))?;
    PROVISION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let provision = slot.as_ref().ok_or("No private Stem reservation exists")?;
        if provision.id != id || &provision.host != host || &provision.boot != boot {
            return Err(Refusal::new("Stem.StaleReservation", "Cancellation names another reservation"));
        }
        if provision.target_job_pending {return Err(Refusal::new("Stem.DrainRequired","Await the selected browser producer before cancellation"));}
        if provision.delivery_committed {
            return Err(Refusal::new("Stem.DeliveryCommitted", "Exposed invitation bytes cannot be retracted by cancellation"));
        }
        let response = encode(&serde_json::json!({"schema":"conduit.stem/reservation-cancelled@1", "reservation_id":id, "disposition":"cancelled", "invitation_revoked":true, "provider_started":provision.provider_attempted}))?;
        ADMISSIONS.with(|manager| {
            let mut manager = manager.borrow_mut();
            manager.as_mut().ok_or("Workspace admission state is missing")?
                .revoke_pending_spawn_invitation(&provision.invitation)
                .map_err(|failure| Refusal::new(&format!("Stem.Revocation.{failure:?}"), "Owned invitation revocation refused"))
        })?;
        slot.take();
        Ok(response)
    })
}
