//! Workspace lifecycle orchestration. The existing browser Body slot executes.
use conduit_body::{
    AdmissionManager, BodyBiographyEvidence, BodyPlayIdentity, ResidentForm, SpawnAdmissionProof,
    SpawnInvitationClaim, SpawnInvitationSecret, Wake,
};
use conduit_core::{BootId, HostAdvertisement, HostId};
use conduit_workspace_model::CurrentHostOffers;
use conduit_workspace_model::WorkspaceBody;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[path = "workspace_refusal.rs"]
mod refusal;
use refusal::Refusal;

const HOST_OFFERS_BYTES: usize =
    conduit_body::MAX_BODY_PARTS * conduit_body::MAX_CANDIDATE_ADVERTISEMENT_BYTES as usize;
const CAPACITY: usize = HOST_OFFERS_BYTES + 256 * 1024;
thread_local! {
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; CAPACITY].into_boxed_slice());
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CAPACITY));
    static BODY: RefCell<Option<WorkspaceBody>> = const { RefCell::new(None) };
    static ADMISSIONS: RefCell<Option<AdmissionManager>> = const { RefCell::new(None) };
    static HOST_OFFERS: RefCell<CurrentHostOffers> = RefCell::new(CurrentHostOffers::new());
}

#[derive(Deserialize)]
#[serde(tag = "action", deny_unknown_fields)]
enum Request {
    Arrive {
        advertisement: HostAdvertisement,
    },
    Restore {
        evidence: Box<BodyBiographyEvidence>,
        admission: Option<AdmissionManager>,
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
    },
    OpenAdmitted {
        evidence: Box<BodyBiographyEvidence>,
        admission: AdmissionManager,
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
    },
    Durable,
    AcknowledgeArchives {
        head_digest: [u8; 32],
    },
    InspectInvitation {
        claim: SpawnInvitationClaim,
        now_millis: u64,
    },
    CreateInvitation {
        host_id: HostId,
        boot_id: BootId,
        secret: Vec<u8>,
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
    },
    AdmitInvitation {
        host_id: HostId,
        boot_id: BootId,
        advertisement: HostAdvertisement,
        proof: ReceivedSpawnProof,
        now_millis: u64,
    },
    HostLost {
        host_id: HostId,
        boot_id: BootId,
        lost_host_id: HostId,
        lost_boot_id: BootId,
    },
    Current,
    SelectForm {
        form: ResidentForm,
    },
    LibraryView {
        source: String,
        query: String,
        revision: u32,
    },
    ChangeWorkset {
        host_id: HostId,
        boot_id: BootId,
        expected_revision: u64,
        form: ResidentForm,
        source: String,
        edit: WorksetEdit,
    },
    Propose {
        host_id: HostId,
        boot_id: BootId,
        source: String,
    },
    Started {
        host_id: HostId,
        boot_id: BootId,
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    },
    Lull {
        host_id: HostId,
        boot_id: BootId,
        terminated_play: Option<BodyPlayIdentity>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceivedSpawnProof {
    invitation_id: conduit_body::SpawnInvitationId,
    body_id: conduit_body::BodyId,
    host_id: HostId,
    boot_id: BootId,
    nonce: [u8; 32],
    signature: Vec<u8>,
}

#[derive(Deserialize)]
enum WorksetEdit {
    Install,
    Remove,
}

#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    evidence: &'a BodyBiographyEvidence,
    realization: Option<&'a conduit_workspace_model::WorkspaceRealization>,
    foreground: Option<&'a ResidentForm>,
    foreground_flow: String,
    current_host_offers: Vec<HostAdvertisement>,
}

#[derive(Serialize)]
struct DurableSnapshot<'a> {
    schema: &'static str,
    evidence: &'a BodyBiographyEvidence,
    admission: &'a AdmissionManager,
    foreground: Option<&'a ResidentForm>,
    pending_archives: &'a [conduit_body::BodyBiographyArchiveSegment],
}

#[no_mangle]
pub extern "C" fn conduit_workspace_input_ptr() -> usize {
    INPUT.with(|value| value.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_workspace_input_capacity() -> usize {
    CAPACITY
}
#[no_mangle]
pub extern "C" fn conduit_workspace_output_ptr() -> usize {
    OUTPUT.with(|value| value.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_workspace_output_len() -> usize {
    OUTPUT.with(|value| value.borrow().len())
}
#[no_mangle]
pub extern "C" fn conduit_workspace_output_capacity() -> usize {
    CAPACITY
}

#[no_mangle]
pub extern "C" fn conduit_workspace_request(length: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    if length == 0 || length > CAPACITY {
        return -1;
    }
    let request = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let request = serde_json::from_slice::<Request>(&input[..length]);
        input[..length].fill(0);
        request
    });
    let result = request
        .map_err(|error| Refusal::new("InvalidRequest", error.to_string()))
        .and_then(dispatch);
    match result {
        Ok(bytes) => {
            OUTPUT.with(|output| *output.borrow_mut() = bytes);
            0
        }
        Err(error) => {
            let refusal = serde_json::json!({ "schema": "conduit.workspace/refusal@1", "disposition": "refused", "code": error.code, "message": error.message });
            if let Ok(bytes) = serde_json::to_vec(&refusal) {
                OUTPUT.with(|output| *output.borrow_mut() = bytes);
            }
            -2
        }
    }
}

fn dispatch(request: Request) -> Result<Vec<u8>, Refusal> {
    BODY.with(|slot| {
        let mut slot = slot.borrow_mut();
        match request {
            Request::Arrive { advertisement } => {
                if slot.is_some() {
                    return Err("Workspace already has a Body".into());
                }
                let evidence = crate::creche::workspace_evidence()?;
                let body = WorkspaceBody::open(evidence).map_err(debug)?;
                let mut offers = CurrentHostOffers::new();
                offers
                    .observe(body.evidence(), advertisement)
                    .map_err(host_offer_refusal)?;
                validate_offer_bytes(&offers)?;
                let admissions = AdmissionManager::new(body.evidence().body_id.clone())
                    .map_err(|error| Refusal::new("Admission.Initialize", format!("{error:?}")))?;
                let bytes = snapshot_with_offers(&body, offers.hosts())?;
                crate::creche::handoff_workspace();
                *slot = Some(body);
                ADMISSIONS.with(|state| *state.borrow_mut() = Some(admissions));
                HOST_OFFERS.with(|state| *state.borrow_mut() = offers);
                return Ok(bytes);
            }
            Request::Restore {
                evidence,
                admission,
                host_id,
                boot_id,
                advertisement,
            } => {
                if slot.is_some() {
                    return Err("Workspace already has a Body".into());
                }
                let body =
                    WorkspaceBody::resume_here(*evidence, &host_id, &boot_id).map_err(debug)?;
                let mut offers = CurrentHostOffers::new();
                offers
                    .observe(body.evidence(), advertisement)
                    .map_err(host_offer_refusal)?;
                validate_offer_bytes(&offers)?;
                let admissions = admission.unwrap_or(AdmissionManager::new(body.evidence().body_id.clone())
                    .map_err(|error| Refusal::new("Admission.Initialize", format!("{error:?}")))?);
                if admissions.body_id != body.evidence().body_id {
                    return Err(Refusal::new(
                        "Admission.WrongBody",
                        "Retained admission state names another Body",
                    ));
                }
                let bytes = snapshot_with_offers(&body, offers.hosts())?;
                *slot = Some(body);
                ADMISSIONS.with(|state| *state.borrow_mut() = Some(admissions));
                HOST_OFFERS.with(|state| *state.borrow_mut() = offers);
                return Ok(bytes);
            }
            Request::OpenAdmitted { evidence, admission, host_id, boot_id, advertisement } => {
                if slot.is_some() { return Err("Workspace already has a Body".into()); }
                if admission.body_id != evidence.body_id {
                    return Err(Refusal::new("Admission.WrongBody", "Admission state names another Body"));
                }
                let body = WorkspaceBody::open_admitted(*evidence, &host_id, &boot_id).map_err(debug)?;
                let mut offers = CurrentHostOffers::new();
                offers
                    .observe(body.evidence(), advertisement)
                    .map_err(host_offer_refusal)?;
                validate_offer_bytes(&offers)?;
                let bytes = snapshot_with_offers(&body, offers.hosts())?;
                *slot = Some(body);
                ADMISSIONS.with(|state| *state.borrow_mut() = Some(admission));
                HOST_OFFERS.with(|state| *state.borrow_mut() = offers);
                return Ok(bytes);
            }
            Request::InspectInvitation { claim, now_millis } => {
                claim.inspect(now_millis).map_err(|error| Refusal::new(
                    &format!("Admission.{error:?}"), "Invitation is malformed, stale, or expired"))?;
                return encode(&claim);
            }
            _ => {}
        }
        let current = slot.as_ref().ok_or("Workspace has no Body")?;
        let mut candidate = current.clone();
        match request {
            Request::Current => return snapshot(current),
            Request::Durable => return ADMISSIONS.with(|admissions| {
                let admissions = admissions.borrow();
                let admissions = admissions.as_ref().ok_or("Workspace admission state is missing")?;
                encode(&DurableSnapshot { schema: "conduit.workspace/body@1", evidence: current.evidence(), admission: admissions, foreground: current.foreground(), pending_archives: current.pending_archives() })
            }),
            Request::AcknowledgeArchives { head_digest } => {
                candidate.acknowledge_archives(head_digest).map_err(debug)?;
            }
            Request::CreateInvitation { host_id, boot_id, secret, nonce, now_millis, expires_at_millis } => {
                let secret = <[u8; 32]>::try_from(secret.as_slice())
                    .map_err(|_| Refusal::new("Admission.WeakSecret", "Invitation secret must have exactly 32 bytes"))?;
                let secret = SpawnInvitationSecret::from_csprng_bytes(secret)
                    .map_err(|error| Refusal::new(&format!("Admission.{error:?}"), "Invitation entropy was refused"))?;
                let claim = ADMISSIONS.with(|admissions| {
                    let mut admissions = admissions.borrow_mut();
                    let admissions = admissions.as_mut().ok_or("Workspace admission state is missing")?;
                    current.issue_invitation(admissions, secret, nonce, now_millis, expires_at_millis, &host_id, &boot_id).map_err(debug)
                })?;
                return encode(&claim);
            }
            Request::AdmitInvitation { host_id, boot_id, advertisement, proof, now_millis } => {
                let signature = <[u8; conduit_body::ADMISSION_SIGNATURE_BYTES]>::try_from(
                    proof.signature.as_slice(),
                )
                .map_err(|_| Refusal::new("Admission.InvalidProof", "Admission signature has the wrong bound"))?;
                let proof = SpawnAdmissionProof {
                    invitation_id: proof.invitation_id,
                    body_id: proof.body_id,
                    host_id: proof.host_id,
                    boot_id: proof.boot_id,
                    nonce: proof.nonce,
                    signature,
                };
                let mut next_admissions = ADMISSIONS.with(|admissions| {
                    admissions
                        .borrow()
                        .clone()
                        .ok_or("Workspace admission state is missing")
                })?;
                let credential = candidate.admit_invited_host(
                    &mut next_admissions,
                    &advertisement,
                    &proof,
                    now_millis,
                    &host_id,
                    &boot_id,
                ).map_err(debug)?;
                let mut next_offers = HOST_OFFERS.with(|offers| offers.borrow().clone());
                next_offers
                    .observe(candidate.evidence(), advertisement.clone())
                    .map_err(host_offer_refusal)?;
                validate_offer_bytes(&next_offers)?;
                let response = encode(&serde_json::json!({ "schema": "conduit.workspace/admission-receipt@1", "credential": credential, "body": snapshot_value(&candidate, next_offers.hosts())?, "durable": durable_value(&candidate, &next_admissions)? }))?;
                *slot = Some(candidate);
                ADMISSIONS.with(|admissions| *admissions.borrow_mut() = Some(next_admissions));
                HOST_OFFERS.with(|offers| *offers.borrow_mut() = next_offers);
                return Ok(response);
            }
            Request::HostLost {
                host_id,
                boot_id,
                lost_host_id,
                lost_boot_id,
            } => {
                candidate
                    .observe_host_lost(&lost_host_id, &lost_boot_id, &host_id, &boot_id)
                    .map_err(debug)?;
                let mut next_offers = HOST_OFFERS.with(|offers| offers.borrow().clone());
                next_offers.reconcile(candidate.evidence());
                validate_offer_bytes(&next_offers)?;
                let response = snapshot_with_offers(&candidate, next_offers.hosts())?;
                *slot = Some(candidate);
                HOST_OFFERS.with(|offers| *offers.borrow_mut() = next_offers);
                return Ok(response);
            }
            Request::LibraryView {
                source,
                query,
                revision,
            } => {
                return crate::creche::workspace_library(&source)?
                    .presentation(current, revision, &query)
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))?
                    .lower()
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))?
                    .encode()
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))
                    .and_then(|bytes| {
                        if bytes.len() <= CAPACITY {
                            Ok(bytes)
                        } else {
                            Err(Refusal::new(
                                "OutputBound",
                                "Form library exceeds its presentation bound",
                            ))
                        }
                    });
            }
            Request::ChangeWorkset {
                host_id,
                boot_id,
                expected_revision,
                form,
                source,
                edit,
            } => {
                crate::form_runner::workspace::require_empty()?;
                match edit {
                    WorksetEdit::Install => {
                        crate::creche::require_workspace_form(&source, &form)?;
                        candidate
                            .admit_form(expected_revision, form.clone(), &host_id, &boot_id)
                            .map_err(debug)?;
                        candidate.select_form(&form).map_err(debug)?;
                    }
                    WorksetEdit::Remove => candidate
                        .remove_form(expected_revision, &form, &host_id, &boot_id)
                        .map_err(debug)?,
                }
            }
            Request::SelectForm { form } => candidate.select_form(&form).map_err(debug)?,
            Request::Propose {
                host_id,
                boot_id,
                source,
            } => {
                let forms = crate::creche::plan_workspace_forms(
                    current.evidence(),
                    &source,
                    &HOST_OFFERS.with(|offers| offers.borrow().hosts().to_vec()),
                    &host_id,
                    &boot_id,
                )?;
                let realization = candidate
                    .propose(forms, &host_id, &boot_id)
                    .map_err(debug)?;
                let bytes = encode(&serde_json::json!({
                    "schema": "conduit.patchbay/body-execution-proposal@1",
                    "wake": realization.wake, "plan": realization.plan,
                    "body_evidence": candidate.evidence(),
                    "source": source,
                }))?;
                *slot = Some(candidate);
                return Ok(bytes);
            }
            Request::Started {
                host_id,
                boot_id,
                play,
                wake_at_start,
            } => {
                crate::form_runner::workspace::require_started(&play)?;
                candidate
                    .started(&host_id, &boot_id, play, wake_at_start)
                    .map_err(debug)?;
                let bytes = snapshot(&candidate)?;
                *slot = Some(candidate);
                crate::form_runner::workspace::acknowledge_start();
                return Ok(bytes);
            }
            Request::Lull {
                host_id,
                boot_id,
                terminated_play,
            } => {
                crate::form_runner::workspace::require_empty()?;
                candidate
                    .lull(&host_id, &boot_id, terminated_play.as_ref())
                    .map_err(debug)?;
            }
            Request::Arrive { .. }
            | Request::Restore { .. }
            | Request::OpenAdmitted { .. }
            | Request::InspectInvitation { .. } => {
                unreachable!("handled before current Body")
            }
        }
        let bytes = snapshot(&candidate)?;
        *slot = Some(candidate);
        Ok(bytes)
    })
}
fn snapshot_value(
    body: &WorkspaceBody,
    offers: &[HostAdvertisement],
) -> Result<serde_json::Value, Refusal> {
    serde_json::to_value(Snapshot {
        schema: "conduit.workspace/body@1",
        evidence: body.evidence(),
        realization: body.realization(),
        foreground: body.foreground(),
        foreground_flow: body.foreground_flow(),
        current_host_offers: offers.to_vec(),
    })
    .map_err(|error| Refusal::new("EncodingFailure", error.to_string()))
}
fn durable_value(
    body: &WorkspaceBody,
    admissions: &AdmissionManager,
) -> Result<serde_json::Value, Refusal> {
    serde_json::to_value(DurableSnapshot {
        schema: "conduit.workspace/body@1",
        evidence: body.evidence(),
        admission: admissions,
        foreground: body.foreground(),
        pending_archives: body.pending_archives(),
    })
    .map_err(|error| Refusal::new("EncodingFailure", error.to_string()))
}

fn snapshot(body: &WorkspaceBody) -> Result<Vec<u8>, Refusal> {
    snapshot_with_offers(body, &current_host_offers())
}

fn snapshot_with_offers(
    body: &WorkspaceBody,
    offers: &[HostAdvertisement],
) -> Result<Vec<u8>, Refusal> {
    encode(&Snapshot {
        schema: "conduit.workspace/body@1",
        evidence: body.evidence(),
        realization: body.realization(),
        foreground: body.foreground(),
        foreground_flow: body.foreground_flow(),
        current_host_offers: offers.to_vec(),
    })
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, Refusal> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| Refusal::new("EncodingFailure", error.to_string()))?;
    if bytes.len() > CAPACITY {
        return Err(Refusal::new(
            "OutputBound",
            "Workspace output exceeds its admitted bound",
        ));
    }
    Ok(bytes)
}
fn debug(error: conduit_workspace_model::WorkspaceBodyError) -> Refusal {
    error.into()
}

fn current_host_offers() -> Vec<HostAdvertisement> {
    HOST_OFFERS.with(|offers| offers.borrow().hosts().to_vec())
}

fn host_offer_refusal(error: conduit_workspace_model::CurrentHostOfferError) -> Refusal {
    Refusal::new(
        "HostOffer",
        format!("current Host offer refused: {error:?}"),
    )
}

fn validate_offer_bytes(offers: &CurrentHostOffers) -> Result<(), Refusal> {
    let bytes = serde_json::to_vec(offers.hosts())
        .map_err(|error| Refusal::new("HostOffer", error.to_string()))?;
    if bytes.len() > HOST_OFFERS_BYTES {
        return Err(Refusal::new(
            "HostOffer.Bound",
            "current Host offers exceed the Workspace observation bound",
        ));
    }
    Ok(())
}
