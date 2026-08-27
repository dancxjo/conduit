//! Non-flashing physical proof for provisioned Pico Body admission.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use conduit_body::{
    AdmissionManager, AdmissionSigns, Body, BodyMembership, CandidateInventory, DiscoveryProofId,
};
use conduit_core::{
    BaseImplementationId, CheckedFormId, HostAdvertisement, LinkBindingId, Plan, SignId,
    SourceDocumentId,
};
use conduit_std_host::pico_admission::PicoAdmissionSocket;
use serde::Serialize;

use super::{PicoArgs, PicoResult};
use crate::workspace::workspace_root;

const IO_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
struct PhysicalAdmissionReceipt {
    schema: &'static str,
    proof_class: &'static str,
    physical_device_claim: bool,
    link_path: String,
    body_id: String,
    candidate_id: String,
    part_id: String,
    host_id: String,
    boot_id: String,
    offer_generation: u64,
    capability_count: usize,
    plan_id: String,
    plan_fragment_count: usize,
    pico_plan_fragment_count: usize,
    all_plan_fragments_on_admitted_pico: bool,
    membership_parts: usize,
}

pub(super) fn run(args: &PicoArgs) -> PicoResult<()> {
    let path = args
        .link_port
        .as_deref()
        .ok_or("pico prove-body-admission requires --link-port or PICO_W_LINK_PORT")?;
    if args.dry_run {
        println!("==> would prove provisioned Pico Body admission over {path} without flashing");
        return Ok(());
    }
    validate_physical_pico(path)?;

    let body = Body::born(
        SourceDocumentId::from("source/physical-pico-admission"),
        CheckedFormId::from("checked/physical-pico-admission"),
        1,
        SignId::from("physical-pico/body-born"),
    )
    .map_err(debug("birth proof Body"))?;
    let mut candidates = CandidateInventory::new(body.body_id.clone())
        .map_err(debug("create candidate inventory"))?;
    let proof_id = DiscoveryProofId::bind("physical-pico/udev-and-usb-frame")
        .map_err(debug("bind discovery proof"))?;
    let socket = PicoAdmissionSocket::open(path).map_err(debug("open Pico admission Line"))?;
    let arrival = socket
        .observe(
            LinkBindingId::from(format!("physical-pico/usb-cdc/{path}")),
            SignId::from("physical-pico/candidate-observed"),
            proof_id,
            IO_TIMEOUT,
        )
        .map_err(debug("observe Pico advertisement"))?;
    let advertisement = arrival.observation.advertisement.clone();
    let candidate_id = candidates
        .observe(arrival.observation)
        .map_err(debug("record inert Pico candidate"))?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(debug("create proof membership"))?;
    if !membership.parts.is_empty() {
        return Err("physical Pico became a Part before explicit admission".into());
    }

    let mut nonce = [0; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let now = now_millis()?;
    let mut manager =
        AdmissionManager::new(body.body_id.clone()).map_err(debug("create admission manager"))?;
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate_id,
            arrival.verifying_key,
            nonce,
            now,
            now.checked_add(60_000).ok_or("challenge expiry overflow")?,
            SignId::from("physical-pico/admission-requested"),
        )
        .map_err(debug("begin explicit Pico admission"))?;
    nonce.fill(0);
    let (proof, _socket) = arrival
        .socket
        .prove(&challenge, IO_TIMEOUT)
        .map_err(debug("receive Pico admission proof"))?;
    let credential = manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &proof,
            now_millis()?,
            AdmissionSigns {
                part_admitted: SignId::from("physical-pico/part-admitted"),
                host_attached: SignId::from("physical-pico/host-attached"),
                candidate_admitted: SignId::from("physical-pico/candidate-admitted"),
            },
        )
        .map_err(debug("complete Pico admission"))?;
    if membership.parts.len() != 1 {
        return Err("physical admission did not create exactly one Part".into());
    }

    let plan = plan_from_advertisement(&advertisement)?;
    let pico_plan_fragment_count = plan
        .fragments
        .iter()
        .filter(|fragment| {
            fragment.host_id == advertisement.host_id
                && fragment.boot_id == advertisement.boot_id
                && fragment.offer_generation == advertisement.offer_generation
        })
        .count();
    let all_plan_fragments_on_admitted_pico = plan.fragments.iter().all(|fragment| {
        fragment.host_id == advertisement.host_id && fragment.boot_id == advertisement.boot_id
    });
    if pico_plan_fragment_count != 1 {
        return Err(
            "ordinary planner did not select exactly one admitted current Pico fragment".into(),
        );
    }

    let receipt = PhysicalAdmissionReceipt {
        schema: "conduit.body/physical-pico-admission@1",
        proof_class: "physical-hardware",
        physical_device_claim: true,
        link_path: path.into(),
        body_id: body.body_id.as_str().into(),
        candidate_id: candidate_id.as_str().into(),
        part_id: credential.part_id.as_str().into(),
        host_id: credential.host_id.as_str().into(),
        boot_id: credential.boot_id.as_str().into(),
        offer_generation: advertisement.offer_generation.0,
        capability_count: advertisement.capabilities.len(),
        plan_id: plan.plan_id.as_str().into(),
        plan_fragment_count: plan.fragments.len(),
        pico_plan_fragment_count,
        all_plan_fragments_on_admitted_pico,
        membership_parts: membership.parts.len(),
    };
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

fn plan_from_advertisement(advertisement: &HostAdvertisement) -> PicoResult<Plan> {
    if advertisement.profile.as_str() == "rp2040-r1-kernel" {
        let expected =
            conduit_system_continuity::r1_signal_pico_advertisement(advertisement.boot_id.clone());
        if advertisement != &expected {
            return Err("R1 Pico advertisement differs from the exact active profile".into());
        }
        return Ok(conduit_system_continuity::exact_r1_signal_plan(
            advertisement.boot_id.clone(),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )?
        .plan);
    }

    if advertisement.profile.as_str() != "pico-w-signal-kernel" {
        return Err(format!(
            "unsupported physical Pico profile: {}",
            advertisement.profile.as_str()
        )
        .into());
    }
    let root = workspace_root()?;
    let source = std::fs::read_to_string(root.join("fixtures/forms/signal-demo.conduit"))?;
    let checked = conduit_form::parse_with_startup(
        &source,
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    )?;
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_placements(&checked, &hosts)?;
    // The provisioned Pico advertises the exact capacity-one kernel image. Seal
    // that reviewed finite budget into the ordinary Plan instead of asking the
    // planner's hosted convenience default for four queue items.
    Ok(conduit_planner::plan_with_connection_limits(
        &checked,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        conduit_signal::SIGNAL_ENCODED_LEN,
    )?)
}

fn validate_physical_pico(path: &str) -> PicoResult<()> {
    let canonical = std::fs::canonicalize(path)?;
    if !canonical.starts_with(Path::new("/dev")) {
        return Err("physical proof requires a device beneath /dev".into());
    }
    let output = Command::new("udevadm")
        .args(["info", "--query=property", "--name"])
        .arg(&canonical)
        .output()?;
    if !output.status.success() {
        return Err("udevadm could not identify the selected Pico Line".into());
    }
    let properties = String::from_utf8(output.stdout)?;
    if !properties.lines().any(|line| line == "ID_VENDOR=Conduit")
        || !properties
            .lines()
            .any(|line| line == "ID_MODEL=Pico_W_Signal")
    {
        return Err("selected Line is not a physical Conduit Pico W Signal device".into());
    }
    Ok(())
}

fn now_millis() -> PicoResult<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pico_device_cannot_be_promoted_to_physical_evidence() {
        assert!(validate_physical_pico("/dev/null").is_err());
    }

    #[test]
    fn exact_r1_advertisement_yields_std_to_admitted_pico_plan() {
        let advertisement = conduit_system_continuity::r1_signal_pico_advertisement(
            conduit_core::BootId::from("r1/test-boot"),
        );
        let plan = plan_from_advertisement(&advertisement).unwrap();
        assert_eq!(plan.fragments.len(), 2);
        assert_eq!(
            plan.fragments
                .iter()
                .filter(|fragment| fragment.host_id == advertisement.host_id)
                .count(),
            1
        );
    }

    #[test]
    fn altered_r1_advertisement_cannot_select_the_canonical_plan() {
        let mut advertisement = conduit_system_continuity::r1_signal_pico_advertisement(
            conduit_core::BootId::from("r1/test-boot"),
        );
        advertisement.capabilities.clear();
        assert!(plan_from_advertisement(&advertisement).is_err());
    }
}
