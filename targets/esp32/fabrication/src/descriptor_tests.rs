use crate::descriptor::*;
use conduit_host_fabrication::*;

const HEADLESS: &str = include_str!("../../../conduitos/profiles/conduitos-headless.profile.json");

fn fixture() -> Esp32BoardDescriptor {
    Esp32BoardDescriptor {
        schema: ESP32_DESCRIPTOR_SCHEMA.into(),
        id: "fixture/esp-wroom-32@1".into(),
        fabrication: Esp32FabricationIdentity {
            board_marking: "FIXTURE BOARD; NOT PHYSICAL EVIDENCE".into(),
            module_marking: "FIXTURE MODULE".into(),
            soc_marking: "FIXTURE SOC".into(),
            revision: "fixture-revision".into(),
            inspection_evidence: "fixture/not-physical".into(),
        },
        target: Esp32TargetFacts {
            architecture: "xtensa-lx6".into(),
            machine: "hw-463-esp-wroom-32".into(),
            chip: "fixture-chip".into(),
            cores: 2,
            clock_hz: 240_000_000,
        },
        memory_regions: vec![Esp32MemoryRegion {
            id: "dram".into(),
            kind: Esp32MemoryKind::DataRam,
            physical_bytes: 128,
            usable_bytes: 96,
        }],
        flash: Esp32FlashFacts {
            bytes: 4096,
            mode: "fixture".into(),
            maximum_frequency_hz: 1,
        },
        boot: Esp32BootFacts {
            image_format: "fixture".into(),
            flash_transport: "fixture-uart".into(),
            diagnostic_transport: "fixture-uart".into(),
        },
        pins: vec![Esp32PinFacts {
            gpio: 2,
            functions: vec![Esp32PinFunction::DigitalOutput],
            reservation: None,
        }],
        controllers: vec![Esp32ControllerFacts {
            id: "uart0".into(),
            kind: Esp32ControllerKind::Uart,
            channels: 1,
        }],
        radios: vec![Esp32RadioFacts {
            id: "wifi0".into(),
            kind: Esp32RadioKind::Wifi24Ghz,
        }],
    }
}

fn profile(descriptor: Option<String>) -> HostProfile {
    let mut profile: HostProfile = serde_json::from_str(HEADLESS).unwrap();
    profile.name = "fixture-esp32".into();
    profile.target.family = "esp32".into();
    profile.target.architecture = "xtensa-lx6".into();
    profile.target.machine = "hw-463-esp-wroom-32".into();
    profile.target.fabrication_descriptor = descriptor;
    profile.lines.clear();
    profile.exclusions.clear();
    profile
}

fn catalog() -> (FabricationCatalog, String) {
    let packages = conduit_host_fabrication::FabricationPackageSet::compose(&[
        &crate::Esp32FabricationPackage,
    ])
    .unwrap();
    let mut catalog = FabricationCatalog::canonical().with_packages(&packages);
    let descriptor = fixture();
    let binding = esp32_descriptor_binding(&descriptor).unwrap();
    catalog.fabrication_descriptors.insert(
        binding.clone(),
        "esp32/xtensa-lx6/hw-463-esp-wroom-32".into(),
    );
    (catalog, binding)
}

fn build_inputs() -> BuildInputs {
    BuildInputs {
        source_identity: "git:fixture".into(),
        toolchain_available: true,
    }
}

#[test]
fn esp32_profile_requires_an_exact_descriptor_and_binds_it_to_identity() {
    let (base_catalog, binding) = catalog();
    let diagnostics = validate_profile(profile(None), &base_catalog).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|item| matches!(item, ProfileDiagnostic::MissingFabricationDescriptor { .. })));

    let first = validate_profile(profile(Some(binding)), &base_catalog).unwrap();
    let mut changed = fixture();
    changed.target.clock_hz -= 1;
    let changed_binding = esp32_descriptor_binding(&changed).unwrap();
    let (mut changed_catalog, _) = catalog();
    changed_catalog.fabrication_descriptors.insert(
        changed_binding.clone(),
        "esp32/xtensa-lx6/hw-463-esp-wroom-32".into(),
    );
    let second = validate_profile(profile(Some(changed_binding)), &changed_catalog).unwrap();
    assert_ne!(first.profile_id(), second.profile_id());
}

#[test]
fn descriptor_rejects_false_capacity_duplicates_and_target_cross_wiring() {
    let mut descriptor = fixture();
    descriptor.memory_regions[0].usable_bytes = 129;
    descriptor.pins.push(descriptor.pins[0].clone());
    let diagnostics = validate_esp32_descriptor(&descriptor).unwrap_err();
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        Esp32DescriptorDiagnostic::UsableExceedsPhysical { .. }
    )));
    assert!(diagnostics.iter().any(|item| matches!(
        item,
        Esp32DescriptorDiagnostic::DuplicateIdentity { field: "gpio", .. }
    )));

    let mut target = profile(Some(esp32_descriptor_binding(&fixture()).unwrap())).target;
    target.architecture = "riscv32".into();
    assert!(matches!(
        validate_esp32_target(&target, &fixture()),
        Err(Esp32DescriptorDiagnostic::TargetMismatch { .. })
    ));
}

#[test]
fn build_and_image_retain_exact_descriptor_binding_without_runtime_truth() {
    let (catalog, binding) = catalog();
    let (image, bytes) = build_default_host_image(
        profile(Some(binding.clone())),
        &catalog,
        &FabricationPackageSet::compose(&[&crate::Esp32FabricationPackage]).unwrap(),
        &build_inputs(),
    )
    .unwrap();
    assert_eq!(
        image.manifest.fabrication_descriptor.as_deref(),
        Some(binding.as_str())
    );
    assert_eq!(image.payload.fabrication_descriptor, Some(binding));
    assert_eq!(
        image.manifest.post_build_actions,
        [PostBuildAction::Flash, PostBuildAction::Boot]
    );
    verify_image_binding(&image, &bytes).unwrap();
    let evidence = String::from_utf8(bytes).unwrap();
    for runtime_truth in ["HostId", "BootId", "OfferGeneration", "ActivePlayId"] {
        assert!(!evidence.contains(runtime_truth));
    }
}
