use crate::*;

const HEADLESS: &str = include_str!("../../../profiles/hosts/conduitos-headless.profile.json");

fn sample_profile(binding: String) -> HostProfile {
    let mut profile: HostProfile = serde_json::from_str(HEADLESS).unwrap();
    profile.name = "observed-hw-463".into();
    profile.target.family = "esp32".into();
    profile.target.architecture = "xtensa-lx6".into();
    profile.target.machine = "hw-463-esp-wroom-32".into();
    profile.target.fabrication_descriptor = Some(binding);
    profile.lines.clear();
    profile
}

fn build_inputs() -> BuildInputs {
    BuildInputs {
        source_identity: "git:observed-sample".into(),
        toolchain_identity: "esp32-toolchain:unexecuted".into(),
        toolchain_available: true,
        maxima: HostBounds {
            static_memory_bytes: u64::MAX,
            heap_arena_bytes: u64::MAX,
            queue_items: u32::MAX,
            buffered_bytes: u64::MAX,
            active_instances: u32::MAX,
            operation_slots: u32::MAX,
            timer_slots: u32::MAX,
            line_sessions: u32::MAX,
            evidence_items: u32::MAX,
        },
    }
}

#[test]
fn observed_wroom_sample_is_exact_and_conservatively_unavailable() {
    let descriptor = hw463_esp_wroom_32_sample();
    validate_esp32_descriptor(&descriptor).unwrap();
    assert_eq!(descriptor.fabrication.board_marking, "HW-463");
    assert_eq!(descriptor.target.chip, "ESP32-D0WD-V3");
    assert_eq!(descriptor.flash.bytes, 4 * 1024 * 1024);
    assert!(descriptor
        .memory_regions
        .iter()
        .all(|region| region.usable_bytes == 0));
    assert!(descriptor.pins.is_empty());
    assert!(descriptor.controllers.is_empty());
}

#[test]
fn only_the_observed_sample_can_reach_profile_and_image_lowering() {
    let descriptor = hw463_esp_wroom_32_sample();
    let binding = esp32_descriptor_binding(&descriptor).unwrap();
    let target = "esp32/xtensa-lx6/hw-463-esp-wroom-32";
    let mut catalog = FabricationCatalog::canonical();
    catalog.targets.push(target.into());
    catalog
        .esp32_descriptors
        .insert(binding.clone(), descriptor);

    let checked = validate_profile(sample_profile(binding.clone()), &catalog).unwrap();
    assert_eq!(checked.profile().target.key(), target);
    assert_eq!(
        checked.profile().target.fabrication_descriptor.as_deref(),
        Some(binding.as_str())
    );
    let (image, _) =
        build_host_image(checked.profile().clone(), &catalog, &build_inputs()).unwrap();
    assert_eq!(image.manifest.image_use, ImageUse::Flash);
    assert_eq!(
        image.manifest.fabrication_descriptor.as_deref(),
        Some(binding.as_str())
    );

    for (architecture, machine) in [
        ("riscv32", "esp32-c3-devkitm-1"),
        ("xtensa-lx7", "esp32-s3-devkitc-n16r8"),
    ] {
        let mut untested = sample_profile(binding.clone());
        untested.target.architecture = architecture.into();
        untested.target.machine = machine.into();
        assert!(validate_profile(untested, &catalog).is_err());
    }
}

#[test]
fn observed_fact_mutation_changes_the_descriptor_binding() {
    let descriptor = hw463_esp_wroom_32_sample();
    let original = esp32_descriptor_binding(&descriptor).unwrap();
    let mut changed = descriptor;
    changed.flash.bytes += 1;
    assert_ne!(esp32_descriptor_binding(&changed).unwrap(), original);
}
