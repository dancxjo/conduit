//! Finite attended admission, return and preparation selected by File policy.
use super::*;
/// Original independently selected member basis and sole opened carrier.
/// Possessing this socket does not establish membership, a Ready Line or Start.
#[cfg(target_os = "linux")]
pub(super) struct SelectedPlayCarrier {
    member: owner::MemberPresentationBasis,
    socket: conduit_std_host::pico_spawn::PicoSpawnSocket,
    canonical_port: std::path::PathBuf,
    completed_return: Option<CompletedMemberReturn>,
}
// Constructed only after the existing proof, commit, receipt delivery and durable
// presentation publication have ALL completed on this same owned socket.
#[cfg(target_os = "linux")]
struct CompletedMemberReturn;
#[cfg(target_os = "linux")]
impl SelectedPlayCarrier {
    pub(super) fn canonical_port(&self) -> &Path {
        &self.canonical_port
    }
    pub(super) fn authenticated_return(&self) -> bool {
        self.completed_return.is_some()
    }
    pub(super) fn consume_for_selected_play(
        self,
        selected: &UsbPlaySelection,
    ) -> Result<Self, String> {
        if !self.authenticated_return() {
            return Err("selected carrier has no completed authenticated return".into());
        }
        let expected = selected_play_basis(selected)?;
        let canonical = selected
            .port
            .canonicalize()
            .map_err(|e| format!("selected Play carrier path: {e}"))?;
        check_retained_selection(&self.canonical_port, &self.member, &canonical, &expected)?;
        Ok(self)
    }
}
#[cfg(target_os = "linux")]
fn check_retained_selection(
    actual_path: &Path,
    actual: &owner::MemberPresentationBasis,
    expected_path: &Path,
    expected: &owner::MemberPresentationBasis,
) -> Result<(), String> {
    if actual_path != expected_path || actual != expected {
        return Err("selected Play differs from original authenticated return carrier or actual member basis; refusing reopen".into());
    }
    Ok(())
}

/// Refuse aliases before opening any reader, including two symlinks to one TTY.
#[cfg(target_os = "linux")]
pub(super) fn open_selected_play_carriers(
    selected: &[UsbPlaySelection],
    returned: &mut Vec<SelectedPlayCarrier>,
) -> Vec<Result<SelectedPlayCarrier, String>> {
    let paths = selected.iter().map(|play| play.port.as_path());
    if let Err(error) = unique_carrier_paths(paths) {
        return selected.iter().map(|_| Err(error.clone())).collect();
    }
    let mut retained_paths = std::collections::BTreeSet::new();
    if returned
        .iter()
        .any(|carrier| !retained_paths.insert(carrier.canonical_port()))
    {
        return selected
            .iter()
            .map(|_| {
                Err("returned carriers repeat one original path; refusing a second reader".into())
            })
            .collect();
    }
    drop(retained_paths);
    selected
        .iter()
        .map(|play| {
            let canonical = play
                .port
                .canonicalize()
                .map_err(|e| format!("selected Play carrier path: {e}"))?;
            if let Some(index) = returned
                .iter()
                .position(|carrier| carrier.canonical_port() == canonical.as_path())
            {
                returned.remove(index).consume_for_selected_play(play)
            } else {
                open_selected_play_carrier(play)
            }
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn unique_carrier_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Result<(), String> {
    let mut unique = std::collections::BTreeSet::new();
    for path in paths {
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("selected Play carrier path: {error}"))?;
        if !unique.insert(canonical) {
            return Err("selected Plays alias one carrier; exactly one reader is required".into());
        }
    }
    Ok(())
}

/// Open before planning and retain the original decoder through preparation.
/// A later Line provider must independently verify installed contracts and
/// current identities before turning this possession into planning evidence.
#[cfg(target_os = "linux")]
pub(super) fn open_selected_play_carrier(
    selected: &UsbPlaySelection,
) -> Result<SelectedPlayCarrier, String> {
    let member = selected_play_basis(selected)?;
    let canonical_port = selected
        .port
        .canonicalize()
        .map_err(|e| format!("selected Play carrier path: {e}"))?;
    let socket = selected
        .carrier
        .open(
            canonical_port
                .to_str()
                .ok_or("selected Play port is not UTF-8")?,
        )
        .map_err(|e| format!("open selected Play carrier: {e:?}"))?;
    Ok(SelectedPlayCarrier {
        member,
        socket,
        canonical_port,
        completed_return: None,
    })
}
#[cfg(target_os = "linux")]
fn selected_play_basis(
    selected: &UsbPlaySelection,
) -> Result<owner::MemberPresentationBasis, String> {
    if !(1..=10_000).contains(&selected.wait_milliseconds) {
        return Err("selected preparation carriage exceeds original challenge lifetime".into());
    }
    let member: owner::MemberPresentationBasis = serde_json::from_slice(&bounded_read(
        &selected.member_basis,
        MAXIMUM_BODY_ADMISSION_BYTES,
    )?)
    .map_err(|e| format!("selected member basis: {e}"))?;
    member.validate()?;
    Ok(member)
}

#[path = "source_revision_preparation_carriage.rs"]
mod preparation_carriage;
pub(super) use preparation_carriage::submit_selected_play;

/// Use the ordinary canonical image-return protocol on the same live owner.
/// Public basis files pin the reinstalled member to this actual owner peer;
/// neither a copied advertisement nor USB access establishes current offers.
#[cfg(target_os = "linux")]
pub(super) fn return_selected_member(
    owner: &mut owner::Owner,
    state_dir: &Path,
    selected: &UsbReturnSelection,
    peer: &conduit_body::FederationPeer,
) -> Result<SelectedPlayCarrier, String> {
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
    return_selected_basis_owned(
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
    return_selected_basis_owned(
        owner,
        state_dir,
        port,
        carrier,
        receipt_output,
        basis,
        recover_retained_key,
    )
    .map(drop)
}
#[cfg(target_os = "linux")]
pub(super) fn return_selected_basis_owned(
    owner: &mut owner::Owner,
    state_dir: &Path,
    port: &Path,
    carrier: conduit_std_host::pico_spawn::ProvisioningCarrier,
    receipt_output: &Path,
    basis: &conduit_std_host::pico_spawn::PicoMemberReturnBasis,
    recover_retained_key: bool,
) -> Result<SelectedPlayCarrier, String> {
    use conduit_body::MemberReturnImageAcknowledgement;
    use std::time::Duration;
    // Resolve the actual owner again instead of trusting a copied peer argument.
    let peer = owner.member_carrier_peer()?;
    if basis.owner != peer {
        return Err("selected return owner differs from the installed member pin".into());
    }
    let canonical_port = port
        .canonicalize()
        .map_err(|e| format!("selected return carrier path: {e}"))?;
    let mut receipt = membership_receipt_custody::reserve(receipt_output)?;
    let (_, mut next_basis) = membership_receipt_custody::reserve_return_basis(receipt_output)?;
    let timeout = Duration::from_secs(5);
    let mut socket = carrier
        .open(canonical_port.to_str().ok_or("USB path is not UTF-8")?)
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
    let post_commit = |error: String| {
        format!("return committed; original receipt/member state may be retained: {error}")
    };
    let signed = owner
        .attest_current_member_receipt(
            &owner::MembershipReceiptDocument::ImageReturn {
                receipt: MemberReturnImageAcknowledgement::new(
                    credential.clone(),
                    observed.presentation.projection.clone(),
                ),
                images: observed.images,
            },
            &peer,
        )
        .map_err(post_commit)?;
    signed
        .verify_image_return(
            &peer,
            &basis.member_verifying_key,
            &basis.credential,
            &observed.presentation.projection,
        )
        .map_err(|e| format!("return committed; verify selected member receipt: {e:?}"))?;
    membership_receipt_custody::retain(&mut receipt, &signed).map_err(post_commit)?;
    membership_receipt_custody::retain_return_basis(
        &mut next_basis,
        basis,
        &signed,
        &observed.presentation.projection,
    )?;
    socket
        .deliver_member_return(&signed, basis, &observed.presentation.projection, timeout)
        .map_err(|e| {
            format!("return committed and receipt retained; member delivery uncertain: {e:?}")
        })?;
    owner
        .retire_completed_selected_return(&credential)
        .map_err(post_commit)?;
    // Descriptive convenience for this same Root's later checked preparation.
    // Every use still rechecks actual current membership and observed offers.
    let presentation = owner::MemberPresentationBasis::new(credential, observed.advertisement)
        .map_err(post_commit)?;
    write_json_atomic(
        &receipt_output.with_extension("presentation.json"),
        &presentation,
    )
    .map_err(post_commit)?;
    Ok(SelectedPlayCarrier {
        member: presentation,
        socket,
        canonical_port,
        completed_return: Some(CompletedMemberReturn),
    })
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

#[cfg(all(test, target_os = "linux"))]
mod carrier_path_tests {
    use super::*;
    #[test]
    fn canonical_aliases_refuse_before_opening_a_second_reader() {
        let root =
            std::env::temp_dir().join(format!("conduit-carrier-paths-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let first = root.join("first");
        let second = root.join("second");
        let alias = root.join("alias");
        std::fs::write(&first, []).unwrap();
        std::fs::write(&second, []).unwrap();
        std::os::unix::fs::symlink(&first, &alias).unwrap();
        assert!(unique_carrier_paths([first.as_path(), second.as_path()]).is_ok());
        assert!(unique_carrier_paths([first.as_path(), alias.as_path()]).is_err());
        assert!(unique_carrier_paths([root.join("absent").as_path()]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn retained_reuse_requires_exact_path_and_actual_member_basis() {
        let ad = conduit_std_host::StdHost::new().advertisement().clone();
        let credential = serde_json::from_value(serde_json::json!({
            "credential_id":"fixture/credential", "body_id":"fixture/body", "part_id":"fixture/part",
            "host_id":ad.host_id, "boot_id":ad.boot_id, "issued_at_millis":1,
        })).unwrap();
        let actual = owner::MemberPresentationBasis::new(credential, ad).unwrap();
        let path = Path::new("/fixture/original-canonical-port");
        assert!(check_retained_selection(path, &actual, path, &actual).is_ok());
        assert!(check_retained_selection(
            path,
            &actual,
            Path::new("/fixture/reopened-port"),
            &actual
        )
        .is_err());
        let mut expected = actual.clone();
        expected.credential.part_id =
            conduit_body::PartId::bind(&actual.credential.body_id, "fixture/other-part", 2)
                .unwrap();
        assert!(check_retained_selection(path, &actual, path, &expected).is_err());
        let mut expected = actual.clone();
        expected.host_advertisement.offer_generation.0 += 1;
        assert!(check_retained_selection(path, &actual, path, &expected).is_err());
        let mut expected = actual.clone();
        expected.host_advertisement.boot_id = "fixture/stale-boot".into();
        assert!(check_retained_selection(path, &actual, path, &expected).is_err());
        // Equality only selects an already authenticated original carrier; it
        // cannot construct CompletedMemberReturn or mark an opened FD Ready.
    }
}
