use super::*;

fn fixture() -> (conduit_observatory::ObservatorySnapshot, serde_json::Value) {
    let identities = conduitos::identity::BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        "build",
        conduitos::offer::CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        512 * 1024,
    );
    let prepared = conduitos::dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    let record = conduitos::boot::BootRecord {
        firmware: conduitos::boot::Firmware::X86Bios,
        timestamp: 1,
        hhdm_offset: 2,
        rsdp_address: None,
        image_physical_start: 3,
        image_length: 4,
        memory_region_count: 5,
        artifact_count: 0,
        framebuffer_count: 0,
        command_line_bytes: 0,
        runtime_arena: conduitos::boot::RuntimeArena {
            physical_start: 6,
            length: 512 * 1024,
        },
    };
    let export = conduitos::observatory::prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        "build",
        "image",
        None,
    )
    .unwrap();
    let mut snapshot: conduit_observatory::ObservatorySnapshot =
        serde_json::from_slice(export.as_bytes()).unwrap();
    let fragment = &mut snapshot.plans[0].fragments[0];
    fragment.execution_regions[0].execution_profile_id =
        conduitos::ordinary_plan::PROTECTED_REGION_PROFILE.into();
    fragment.execution_regions[0].preemption_required = true;
    fragment.execution_regions[0].isolation_required = true;
    let upper = fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "text/upper")
        .unwrap();
    upper.resources[0].units = 4096 + 118784 + 22480;
    (
        snapshot,
        serde_json::json!({"reserved_bytes":118784,"root_metadata_bytes":22374}),
    )
}

#[test]
fn protected_text_reservation_covers_measured_storage_without_growing_other_gears() {
    let (snapshot, cost) = fixture();
    validate_protected_realization(&snapshot, &cost).unwrap();
    for units in [4096, 118784, 4096 + 118784 + 22373, 1048576] {
        let mut changed = snapshot.clone();
        let upper = changed.plans[0].fragments[0]
            .placements
            .iter_mut()
            .find(|placement| placement.kind_id.as_str() == "text/upper")
            .unwrap();
        upper.resources[0].units = units;
        assert!(validate_protected_realization(&changed, &cost).is_err());
    }
    let mut changed = snapshot.clone();
    changed.plans[0].fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "text/literal")
        .unwrap()
        .resources[0]
        .units += 1;
    assert!(validate_protected_realization(&changed, &cost).is_err());
}

#[test]
fn cooperative_downgrade_or_unexpected_timer_protection_is_refused() {
    let (snapshot, cost) = fixture();
    for (index, profile) in [
        (0, conduitos::ordinary_plan::COOPERATIVE_REGION_PROFILE),
        (1, conduitos::ordinary_plan::PROTECTED_REGION_PROFILE),
    ] {
        let mut changed = snapshot.clone();
        changed.plans[0].fragments[0].execution_regions[index].execution_profile_id =
            profile.into();
        assert!(validate_protected_realization(&changed, &cost).is_err());
    }
    for field in ["isolation_required", "preemption_required"] {
        let mut changed = serde_json::to_value(&snapshot).unwrap();
        changed["plans"][0]["fragments"][0]["execution_regions"][0][field] = false.into();
        assert!(
            validate_protected_realization(&serde_json::from_value(changed).unwrap(), &cost)
                .is_err()
        );
    }
}

#[test]
fn a_provider_in_machine_and_advertisement_inventory_is_one_base() {
    let (snapshot, _) = fixture();
    let machine = snapshot
        .bases
        .iter()
        .map(|base| base.base_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let advertised = &snapshot.hosts[0].advertisement.bases;
    assert_eq!(advertised.len(), 1);
    assert!(machine
        .iter()
        .any(|id| id == advertised[0].base_id.as_str()));
    let expected = expected_base_ids(&machine, advertised, "display");
    assert_eq!(expected.len(), machine.len() + 1);
    assert!(expected.contains(advertised[0].base_id.as_str()));
    assert!(expected.contains("display"));
}
