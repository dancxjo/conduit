//! Finite attended admission, return and preparation selected by File policy.
use super::*;
/// Original independently selected member basis and sole opened carrier.
/// Possessing this socket does not establish membership, a Ready Line or Start.
#[cfg(target_os = "linux")]
pub(super) struct SelectedPlayCarrier {
    member: owner::MemberPresentationBasis,
    socket: conduit_std_host::pico_spawn::PicoSpawnSocket,
}

/// Open before planning and retain the original decoder through preparation.
/// A later Line provider must independently verify installed contracts and
/// current identities before turning this possession into planning evidence.
#[cfg(target_os = "linux")]
pub(super) fn open_selected_play_carrier(
    selected: &UsbPlaySelection,
) -> Result<SelectedPlayCarrier, String> {
    if !(1..=10_000).contains(&selected.wait_milliseconds) {
        return Err("selected preparation carriage exceeds original challenge lifetime".into());
    }
    let member: owner::MemberPresentationBasis = serde_json::from_slice(&bounded_read(
        &selected.member_basis,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|e| format!("selected member basis: {e}"))?;
    member.validate()?;
    let socket = selected
        .carrier
        .open(
            selected
                .port
                .to_str()
                .ok_or("selected Play port is not UTF-8")?,
        )
        .map_err(|e| format!("open selected Play carrier: {e:?}"))?;
    Ok(SelectedPlayCarrier { member, socket })
}

/// A transport report never proves admission or Start. Preserve owner approval
/// on every outcome, including a lost reply, until receiver retirement is proved.
#[cfg(target_os = "linux")]
pub(super) fn submit_selected_play(
    owner: &mut owner::Owner,
    selected: &UsbPlaySelection,
    carrier: &mut SelectedPlayCarrier,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::PreparationCommandBytes;
    let SelectedPlayCarrier { member, socket } = carrier;
    let limits = conduit_embedded_build::BodyFragmentProjectionLimits {
        image: selected.image.bounds(),
        computations: conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: selected.maximum_program_bytes,
            maximum_total_program_bytes: selected.maximum_total_program_bytes,
        },
        assigned: conduit_core::AssignedPlanMaxima {
            encoded_bytes: selected.assigned_encoded_bytes,
            runtime_state_bytes: selected.assigned_runtime_state_bytes,
            counts: selected.assigned_counts,
        },
        maximum_fore_ports: selected.maximum_fore_ports,
        maximum_document_bytes: selected.maximum_document_bytes,
    };
    let mut approved = None;
    let mut refusal = None;
    let exchange = socket.exchange_preparation(
        std::time::Duration::from_millis(selected.wait_milliseconds),
        selected.maximum_document_bytes as usize,
        |challenge| {
            match owner.approve_selected_remote_workset(member, challenge, limits) {
                Ok(command) => {
                    approved = Some(command);
                }
                Err(error) => {
                    refusal = Some(error);
                    return Err(());
                }
            }
            let (payload, signature) = approved.as_ref().expect("just approved");
            Ok(PreparationCommandBytes { payload, signature })
        },
    );
    write_json_atomic(
        &selected.report_output,
        &serde_json::json!({
            "schema": "conduit.diagnostic/preparation-carriage@1",
            "member_host": member.host_advertisement.host_id,
            "member_boot": member.host_advertisement.boot_id,
            "reported": format!("{exchange:?}"),
            "owner_refusal": refusal,
            "admission_proved": false,
            "play_proved": false,
            "approval_retained": approved.is_some(),
        }),
    )?;
    exchange.map(|_| ()).map_err(|e| {
        format!("selected preparation carriage: {e:?}; original approval retained if issued")
    })
}

/// Use the ordinary canonical image-return protocol on the same live owner.
/// Public basis files pin the reinstalled member to this actual owner peer;
/// neither a copied advertisement nor USB access establishes current offers.
#[cfg(target_os = "linux")]
pub(super) fn return_selected_member(
    owner: &mut owner::Owner,
    state_dir: &Path,
    selected: &UsbReturnSelection,
    peer: &conduit_body::FederationPeer,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::PicoMemberReturnBasis;
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_millis(selected.wait_milliseconds);
    while !selected.basis.try_exists().map_err(|e| e.to_string())? {
        if Instant::now() >= deadline {
            return Err("selected member basis did not arrive within authorized wait".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let basis: PicoMemberReturnBasis =
        serde_json::from_slice(&bounded_read(&selected.basis, 8192)?)
            .map_err(|e| format!("selected member return basis: {e}"))?;
    basis
        .validate()
        .map_err(|e| format!("selected member return basis: {e:?}"))?;
    if &basis.owner != peer || basis.credential.part_id != selected.part_id {
        return Err("selected return differs from the member's independently installed current owner pin or selected Part".into());
    }
    return_selected_basis(
        owner,
        state_dir,
        &selected.port,
        selected.carrier,
        &selected.receipt_output,
        &basis,
        selected.recover_retained_key,
    )
}

#[cfg(target_os = "linux")]
pub(super) fn return_selected_basis(
    owner: &mut owner::Owner,
    state_dir: &Path,
    port: &Path,
    carrier: conduit_std_host::pico_spawn::ProvisioningCarrier,
    receipt_output: &Path,
    basis: &conduit_std_host::pico_spawn::PicoMemberReturnBasis,
    recover_retained_key: bool,
) -> Result<(), String> {
    use conduit_body::MemberReturnImageAcknowledgement;
    use std::time::Duration;
    // Resolve the actual owner again instead of trusting a copied peer argument.
    let peer = owner.member_carrier_peer()?;
    if basis.owner != peer {
        return Err("selected return owner differs from the installed member pin".into());
    }
    let mut receipt = membership_receipt_custody::reserve(receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = carrier
        .open(port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open selected member return: {e:?}"))?;
    let observed = socket
        .request_member_return(basis, timeout)
        .map_err(|e| format!("observe selected member: {e:?}"))?;
    let resolved = conduit_std_host::pico_spawn::resolve_member_offer_images(
        &observed.presentation.projection,
        &observed.images,
    )
    .map_err(|e| format!("resolve selected member images: {e:?}"))?;
    if resolved != observed.advertisement {
        return Err("selected member image resolution differs from observed advertisement".into());
    }
    let challenge = if recover_retained_key {
        owner.begin_recovered_member_image_return(
            &basis.credential,
            &observed.advertisement,
            observed.presentation.clone(),
        )?
    } else {
        owner.begin_member_image_return(
            &basis.credential,
            &observed.advertisement,
            Some(observed.presentation.clone()),
        )?
    };
    let proof = socket
        .prove_member_return(basis, &observed.advertisement, &challenge, timeout)
        .map_err(|e| format!("selected member continuity proof: {e:?}"))?;
    let credential = owner.complete_member_return(state_dir, &proof)?;
    let signed = owner.attest_current_member_receipt(
        &owner::MembershipReceiptDocument::ImageReturn {
            receipt: MemberReturnImageAcknowledgement::new(
                credential.clone(),
                observed.presentation.projection.clone(),
            ),
            images: observed.images,
        },
        &peer,
    )?;
    signed
        .verify_image_return(
            &peer,
            &basis.member_verifying_key,
            &basis.credential,
            &observed.presentation.projection,
        )
        .map_err(|e| format!("verify selected member receipt: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt, &signed)?;
    socket
        .deliver_member_return(&signed, basis, &observed.presentation.projection, timeout)
        .map_err(|e| {
            format!("return committed and receipt retained; member delivery uncertain: {e:?}")
        })?;
    owner.retire_completed_selected_return(&credential)?;
    // Descriptive convenience for this same Root's later checked preparation.
    // Every use still rechecks actual current membership and observed offers.
    let presentation = owner::MemberPresentationBasis::new(credential, observed.advertisement)?;
    write_json_atomic(
        &receipt_output.with_extension("presentation.json"),
        &presentation,
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn admit_selected_member(
    owner: &mut owner::Owner,
    state_dir: &Path,
    selected: &UsbAdmissionSelection,
    peer: &conduit_body::FederationPeer,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::{PicoMemberReturnBasis, PicoSpawnAdmissionBasis};
    use std::{
        io::Write,
        time::{Duration, Instant},
    };
    // The local OS policy independently authorizes precisely this physical
    // Host/key. The ordinary invitation is still consumed only by actual proof.
    let mut invitation_file = membership_receipt_custody::reserve(&selected.invitation_output)?;
    let invitation = owner.issue_invitation(state_dir, 300, None)?;
    // Derive from the original owner-minted material BEFORE publishing it for
    // physical Seed composition. Incoming basis bytes never select this key.
    let member_verifying_key = selected.member_key.resolve(&invitation)?;
    invitation_file
        .write_all(&serde_json::to_vec(&invitation).map_err(|e| e.to_string())?)
        .and_then(|_| invitation_file.sync_all())
        .map_err(|e| format!("retain selected private invitation: {e}"))?;
    let deadline = Instant::now() + Duration::from_millis(selected.wait_milliseconds);
    while !selected.basis.try_exists().map_err(|e| e.to_string())? {
        if Instant::now() >= deadline {
            return Err("selected admission basis did not arrive in its finite window".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let basis: PicoSpawnAdmissionBasis =
        serde_json::from_slice(&bounded_read(&selected.basis, 8192)?)
            .map_err(|e| format!("selected admission basis: {e}"))?;
    basis
        .validate(current_time_millis()?)
        .map_err(|e| format!("selected admission basis: {e:?}"))?;
    if selected.verify_basis(&basis, peer, &invitation)? != member_verifying_key {
        return Err("owned invitation bootstrap key changed".into());
    }
    let mut receipt_file = membership_receipt_custody::reserve(&selected.receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = selected
        .carrier
        .open(selected.port.to_str().ok_or("USB path is not UTF-8")?)
        .map_err(|e| format!("open selected admission: {e:?}"))?;
    let observed = socket
        .request_join(&basis.spore_id, &basis.image_id, &basis.invitation, timeout)
        .map_err(|e| format!("observe selected physical admission proof: {e:?}"))?;
    let request = basis
        .request(&observed, current_time_millis()?)
        .map_err(|e| format!("validate selected physical admission proof: {e:?}"))?;
    let receipt = owner.admit_invited(state_dir, request, selected.host_id.as_str())?;
    let return_basis = PicoMemberReturnBasis {
        credential: receipt.credential.clone(),
        expected_profile: basis.expected_profile.clone(),
        member_verifying_key,
        owner: peer.clone(),
    };
    let signed = owner
        .attest_current_member_receipt(&owner::MembershipReceiptDocument::Spawn(receipt), peer)?;
    signed
        .verify(
            peer,
            &member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
        )
        .map_err(|e| format!("verify selected admission receipt: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt_file, &signed)?;
    socket
        .deliver_admission_receipt(
            &signed,
            peer,
            &member_verifying_key,
            &basis.invitation.body_id,
            &observed.advertisement,
            timeout,
        )
        .map_err(|e| {
            format!("membership committed and receipt retained; delivery uncertain: {e:?}")
        })?;
    drop(socket);
    let mut return_file = membership_receipt_custody::reserve(&selected.return_basis_output)?;
    return_file
        .write_all(&serde_json::to_vec(&return_basis).map_err(|e| e.to_string())?)
        .and_then(|_| return_file.sync_all())
        .map_err(|e| format!("retain actual return basis: {e}"))?;
    // Ordinary admission does not carry native resource grants. The actual
    // newly joined member must now sign its current complete image projection.
    return_selected_basis(
        owner,
        state_dir,
        &selected.port,
        selected.carrier,
        &selected.return_receipt_output,
        &return_basis,
        false,
    )
}
