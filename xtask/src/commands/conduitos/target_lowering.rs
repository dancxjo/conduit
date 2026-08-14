//! Exact lowering from a resolved ConduitOS PROFILE to compile and linked-table inputs.

use conduit_host_fabrication::BuildManifest;

use super::ConduitosError;

mod http;

const NATIVE_PRESENTER: &str = "presenter/native-graphical@1";
const NATIVE_COMPOSITOR: &str = "compositor/native@1";
const PRESENT_OPERATION: &str = "conduit.host/present@1";
const SURFACE_CLASS: &str = "presentation/surface";
const SCANOUT_BASE: &str = "display/scanout";
const FRAMEBUFFER_DRIVER: &str = "display/linear-framebuffer@1";
const LINEAR_SERIAL_PRESENTER: &str = "presenter/linear-serial@1";
const SERIAL_TEXT_BASE: &str = "serial/text";
const PL011_DRIVER: &str = "conduitos/pl011@1";
const SCRIPTED_KEYBOARD_PROOF: &str = "profile-fragment/conduitos-scripted-keyboard-proof@1";
const HOTPLUG_PROOF: &str = "profile-fragment/conduitos-hotplug-proof@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TargetBuildInputs {
    pub cargo_features: Vec<&'static str>,
    pub implementations: u16,
    pub facilities: u16,
    pub resources: u16,
    pub bases: u16,
    pub drivers: u16,
    pub presenters: u16,
    pub proof_instrumentation: u16,
    pub presentation_surface_slots: u32,
    pub presentation_surface_bytes: u64,
}

pub(super) fn lower(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    match manifest.target.as_str() {
        "conduitos/x86_64/pc" => lower_x86_64_pc(manifest),
        "conduitos/aarch64/virt" => lower_aarch64_virt(manifest),
        _ => Err(refusal(
            "unsupported-profile-target",
            format!("no ConduitOS product lowerer for {}", manifest.target),
        )),
    }
}

fn lower_x86_64_pc(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    let native = manifest
        .presenters
        .iter()
        .any(|item| item == NATIVE_PRESENTER);
    let presentation_surfaces = manifest
        .resource_budgets
        .iter()
        .filter(|item| item.class == SURFACE_CLASS)
        .collect::<Vec<_>>();
    if presentation_surfaces.len() > 1 {
        return Err(refusal(
            "profile-lowering-resource-ambiguous",
            format!(
                "expected at most one {SURFACE_CLASS} budget, found {}",
                presentation_surfaces.len()
            ),
        ));
    }
    let closure = [
        (
            PRESENT_OPERATION,
            manifest.host_operations.contains(&PRESENT_OPERATION.into()),
        ),
        (
            NATIVE_COMPOSITOR,
            manifest.facilities.contains(&NATIVE_COMPOSITOR.into()),
        ),
        (SURFACE_CLASS, !presentation_surfaces.is_empty()),
        (
            SCANOUT_BASE,
            manifest
                .base_selections
                .iter()
                .any(|item| item.kind == SCANOUT_BASE),
        ),
        (
            FRAMEBUFFER_DRIVER,
            manifest
                .driver_selections
                .iter()
                .any(|item| item.kind == FRAMEBUFFER_DRIVER),
        ),
    ];
    if native {
        if let Some((missing, _)) = closure.iter().find(|(_, present)| !present) {
            return Err(refusal(
                "profile-lowering-prerequisite-missing",
                format!("presenter:{NATIVE_PRESENTER} -> {missing}"),
            ));
        }
    } else if let Some((leaked, _)) = closure.iter().find(|(_, present)| *present) {
        return Err(refusal(
            "headless-graphical-machinery-leaked",
            format!("headless PROFILE unexpectedly selected {leaked}"),
        ));
    }

    // The five portable nucleus implementations are the current non-optional
    // x86 product backbone. Optional inventory is derived only from the
    // resolved PROFILE closure above.
    let backbone = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION
        | conduitos::fabrication::IMPL_KEYBOARD
        | conduitos::fabrication::IMPL_PC_SPEAKER
        | conduitos::fabrication::IMPL_OPL2;
    let (presentation_surface_slots, presentation_surface_bytes) = presentation_surfaces
        .first()
        .map_or((0, 0), |item| (item.slots, item.bytes));
    let scripted_keyboard = manifest
        .profile_fragments
        .iter()
        .any(|item| item == SCRIPTED_KEYBOARD_PROOF);
    let hotplug = manifest
        .profile_fragments
        .iter()
        .any(|item| item == HOTPLUG_PROOF);
    let http = http::lower(manifest)?;
    if (scripted_keyboard || hotplug) && !native {
        return Err(refusal(
            "proof-profile-prerequisite-missing",
            "ConduitOS graphical proof instrumentation requires the native product closure".into(),
        ));
    }
    let mut cargo_features = native
        .then_some("native-compositor")
        .into_iter()
        .collect::<Vec<_>>();
    if hotplug {
        cargo_features.push("hotplug-proof");
    }
    if scripted_keyboard || hotplug {
        cargo_features.push("scripted-keyboard-proof");
    }
    if http.selected {
        cargo_features.push("native-http-client");
    }
    Ok(TargetBuildInputs {
        cargo_features,
        implementations: backbone
            | if native {
                conduitos::fabrication::IMPL_NATIVE_PRESENTER
            } else {
                0
            }
            | http.implementation,
        facilities: (if native {
            conduitos::fabrication::FACILITY_NATIVE_COMPOSITOR
        } else {
            0
        }) | http.facility,
        resources: (if native {
            conduitos::fabrication::RESOURCE_PRESENTATION_SURFACE
        } else {
            0
        }) | http.resource,
        bases: (if native {
            conduitos::fabrication::BASE_DISPLAY_SCANOUT
        } else {
            0
        }) | http.base,
        drivers: (if native {
            conduitos::fabrication::DRIVER_LINEAR_FRAMEBUFFER
        } else {
            0
        }) | http.driver,
        presenters: if native {
            conduitos::fabrication::PRESENTER_NATIVE_GRAPHICAL
        } else {
            0
        },
        proof_instrumentation: (if hotplug {
            conduitos::fabrication::PROOF_HOTPLUG
        } else {
            0
        }) | (if scripted_keyboard || hotplug {
            conduitos::fabrication::PROOF_SCRIPTED_KEYBOARD
        } else {
            0
        }),
        presentation_surface_slots,
        presentation_surface_bytes,
    })
}

fn lower_aarch64_virt(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    if manifest.presenters.as_slice() != [LINEAR_SERIAL_PRESENTER] {
        return Err(aarch64_closure_refusal(
            "presenter",
            LINEAR_SERIAL_PRESENTER,
            &manifest.presenters,
        ));
    }
    let bases = manifest
        .base_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if bases.as_slice() != [SERIAL_TEXT_BASE] {
        return Err(aarch64_closure_refusal("base", SERIAL_TEXT_BASE, &bases));
    }
    let drivers = manifest
        .driver_selections
        .iter()
        .map(|item| item.kind.clone())
        .collect::<Vec<_>>();
    if drivers.as_slice() != [PL011_DRIVER] {
        return Err(aarch64_closure_refusal("driver", PL011_DRIVER, &drivers));
    }
    if manifest.host_operations.as_slice() != [PRESENT_OPERATION]
        || !manifest.facilities.is_empty()
        || !manifest.resource_budgets.is_empty()
        || !manifest.profile_fragments.is_empty()
    {
        return Err(refusal(
            "aarch64-product-closure-mismatch",
            "linear AArch64 product requires only present@1, with no facilities, resources, or proof instrumentation".into(),
        ));
    }
    let base = &manifest.base_selections[0];
    if base.driver != PL011_DRIVER {
        return Err(refusal(
            "aarch64-serial-driver-mismatch",
            format!(
                "base {} selects {}, expected {PL011_DRIVER}",
                base.kind, base.driver
            ),
        ));
    }

    let portable_backbone = conduitos::fabrication::IMPL_TIME_TICK
        | conduitos::fabrication::IMPL_TICK_PRESENTATION
        | conduitos::fabrication::IMPL_TEXT_LITERAL
        | conduitos::fabrication::IMPL_TEXT_UPPER
        | conduitos::fabrication::IMPL_TEXT_PRESENTATION;
    Ok(TargetBuildInputs {
        cargo_features: vec!["aarch64-product"],
        implementations: portable_backbone | conduitos::fabrication::IMPL_LINEAR_PRESENTER,
        facilities: 0,
        resources: 0,
        bases: conduitos::fabrication::BASE_SERIAL_TEXT,
        drivers: conduitos::fabrication::DRIVER_PL011_SERIAL,
        presenters: conduitos::fabrication::PRESENTER_LINEAR_SERIAL,
        proof_instrumentation: 0,
        presentation_surface_slots: 0,
        presentation_surface_bytes: 0,
    })
}

fn aarch64_closure_refusal(class: &str, expected: &str, selected: &[String]) -> ConduitosError {
    refusal(
        "aarch64-product-closure-mismatch",
        format!("expected exactly {class}:{expected}, found {selected:?}"),
    )
}

fn refusal(code: &'static str, detail: String) -> ConduitosError {
    ConduitosError::refusal(code, detail)
}

#[cfg(test)]
mod tests {
    use conduit_host_fabrication::{
        build_host_image, BuildInputs, FabricationCatalog, HostBounds, HostProfile,
    };

    use super::*;

    fn manifest(source: &str) -> BuildManifest {
        let profile: HostProfile = serde_json::from_str(source).unwrap();
        build_host_image(
            profile,
            &FabricationCatalog::canonical(),
            &BuildInputs {
                source_identity: "test-source".into(),
                toolchain_identity: "test-toolchain".into(),
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
            },
        )
        .unwrap()
        .0
        .manifest
    }

    #[test]
    fn resolved_profiles_lower_to_distinct_exact_product_inputs() {
        let native = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        )))
        .unwrap();
        let headless = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        )))
        .unwrap();
        assert_eq!(native.cargo_features, ["native-compositor"]);
        assert!(headless.cargo_features.is_empty());
        assert_ne!(native.implementations, headless.implementations);
        assert_eq!(headless.presenters, 0);
        assert_eq!(headless.presentation_surface_slots, 0);
        assert_eq!(headless.presentation_surface_bytes, 0);
        assert_eq!(native.presentation_surface_slots, 2);
        assert_eq!(native.presentation_surface_bytes, 4_194_304);
        assert_eq!(native.proof_instrumentation, 0);
    }

    #[test]
    fn http_profile_selects_exact_native_closure_and_headless_omits_it() {
        let http = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-http-client.profile.json"
        ));
        let lowered = lower(&http).unwrap();
        assert_eq!(lowered.cargo_features, ["native-http-client"]);
        assert_ne!(
            lowered.implementations & conduitos::fabrication::IMPL_HTTP_CLIENT,
            0
        );
        assert_eq!(
            lowered.facilities,
            conduitos::fabrication::FACILITY_HTTP_CLIENT
        );
        assert_eq!(
            lowered.resources,
            conduitos::fabrication::RESOURCE_HTTP_CLIENT
        );
        assert_eq!(lowered.bases, conduitos::fabrication::BASE_HTTP_NETWORK);
        assert_eq!(lowered.drivers, conduitos::fabrication::DRIVER_HTTP_NETWORK);
        assert_eq!(http.resource_budgets.len(), 4);
        assert_eq!(http.bounds.heap_arena_bytes, 0);

        let headless = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        )))
        .unwrap();
        assert_eq!(
            headless.implementations & conduitos::fabrication::IMPL_HTTP_CLIENT,
            0
        );
        assert_eq!(
            headless.facilities & conduitos::fabrication::FACILITY_HTTP_CLIENT,
            0
        );
        assert!(!headless.cargo_features.contains(&"native-http-client"));
    }

    #[test]
    fn http_lowering_rejects_each_missing_prerequisite_and_every_leak() {
        let http = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-http-client.profile.json"
        ));
        for missing in 0..8 {
            let mut incomplete = http.clone();
            match missing {
                0 => incomplete.host_operations.clear(),
                1..=4 => {
                    incomplete.resource_budgets.remove(missing - 1);
                }
                5 => incomplete.facilities.clear(),
                6 => incomplete.base_selections.clear(),
                7 => incomplete.driver_selections.clear(),
                _ => unreachable!(),
            }
            assert!(lower(&incomplete)
                .unwrap_err()
                .to_string()
                .contains("http-profile-prerequisite-missing"));
        }

        let headless = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        let mut leaked = headless.clone();
        leaked.host_operations.push(http.host_operations[0].clone());
        assert!(lower(&leaked)
            .unwrap_err()
            .to_string()
            .contains("http-machinery-leaked"));
    }

    #[test]
    fn lowering_rejects_missing_and_leaked_graphical_closure() {
        let native = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        ));
        for remove in 0..5 {
            let mut incomplete = native.clone();
            match remove {
                0 => incomplete.host_operations.clear(),
                1 => incomplete.facilities.clear(),
                2 => incomplete.resource_budgets.clear(),
                3 => incomplete.base_selections.clear(),
                4 => incomplete.driver_selections.clear(),
                _ => unreachable!(),
            }
            assert!(lower(&incomplete)
                .unwrap_err()
                .to_string()
                .contains("profile-lowering-prerequisite-missing"));
        }

        let headless = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        for leak in 0..5 {
            let mut leaked = headless.clone();
            match leak {
                0 => leaked.host_operations.push(PRESENT_OPERATION.into()),
                1 => leaked.facilities.push(NATIVE_COMPOSITOR.into()),
                2 => leaked
                    .resource_budgets
                    .push(native.resource_budgets[0].clone()),
                3 => leaked
                    .base_selections
                    .push(native.base_selections[0].clone()),
                4 => leaked
                    .driver_selections
                    .push(native.driver_selections[0].clone()),
                _ => unreachable!(),
            }
            assert!(lower(&leaked)
                .unwrap_err()
                .to_string()
                .contains("headless-graphical-machinery-leaked"));
        }
    }

    #[test]
    fn unrelated_bounds_do_not_select_graphics_and_wrong_targets_fail_before_cargo() {
        let mut headless = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        let original = lower(&headless).unwrap();
        headless.bounds.queue_items += 1;
        assert_eq!(lower(&headless).unwrap(), original);

        headless.target = "conduitos/aarch64/virt".into();
        assert!(lower(&headless)
            .unwrap_err()
            .to_string()
            .contains("aarch64-product-closure-mismatch"));
    }

    #[test]
    fn duplicate_graphical_resource_ceiling_fails_before_codegen() {
        let mut native = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        ));
        native
            .resource_budgets
            .push(native.resource_budgets[0].clone());
        assert!(lower(&native)
            .unwrap_err()
            .to_string()
            .contains("profile-lowering-resource-ambiguous"));
    }

    #[test]
    fn proof_profiles_are_checked_distinct_and_normal_products_stay_clean() {
        let normal = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        )))
        .unwrap();
        let proof = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-proof.profile.json"
        )))
        .unwrap();
        let hotplug = lower(&manifest(include_str!(
            "../../../../profiles/hosts/conduitos-hotplug-proof.profile.json"
        )))
        .unwrap();
        assert_eq!(normal.proof_instrumentation, 0);
        assert_eq!(
            proof.cargo_features,
            ["native-compositor", "scripted-keyboard-proof"]
        );
        assert_eq!(
            proof.proof_instrumentation,
            conduitos::fabrication::PROOF_SCRIPTED_KEYBOARD
        );
        assert_eq!(
            hotplug.cargo_features,
            [
                "native-compositor",
                "hotplug-proof",
                "scripted-keyboard-proof"
            ]
        );
        assert_eq!(
            hotplug.proof_instrumentation,
            conduitos::fabrication::ALL_KNOWN_PROOF_INSTRUMENTATION
        );
    }

    #[test]
    fn proof_instrumentation_without_native_closure_refuses() {
        let mut headless = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        headless
            .profile_fragments
            .push(SCRIPTED_KEYBOARD_PROOF.into());
        assert!(lower(&headless)
            .unwrap_err()
            .to_string()
            .contains("proof-profile-prerequisite-missing"));
    }

    #[test]
    fn aarch64_virt_lowers_to_a_distinct_linear_product_inventory() {
        let manifest = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-aarch64-headless.profile.json"
        ));
        let lowered = lower(&manifest).unwrap();
        assert_eq!(lowered.cargo_features, ["aarch64-product"]);
        assert_eq!(lowered.facilities, 0);
        assert_eq!(lowered.resources, 0);
        assert_ne!(lowered.bases, conduitos::fabrication::BASE_DISPLAY_SCANOUT);
        assert_ne!(
            lowered.drivers,
            conduitos::fabrication::DRIVER_LINEAR_FRAMEBUFFER
        );
        assert_ne!(
            lowered.presenters,
            conduitos::fabrication::PRESENTER_NATIVE_GRAPHICAL
        );
        assert_eq!(lowered.proof_instrumentation, 0);
        assert_eq!(lowered.presentation_surface_slots, 0);
        assert_eq!(lowered.presentation_surface_bytes, 0);
        assert_eq!(
            lowered.implementations
                & (conduitos::fabrication::IMPL_KEYBOARD
                    | conduitos::fabrication::IMPL_PC_SPEAKER
                    | conduitos::fabrication::IMPL_OPL2),
            0
        );
    }

    #[test]
    fn aarch64_virt_rejects_x86_leaks_and_incomplete_serial_closure() {
        let source =
            include_str!("../../../../profiles/hosts/conduitos-aarch64-headless.profile.json");
        let exact = manifest(source);

        let mut x86_presenter = exact.clone();
        x86_presenter.presenters[0] = NATIVE_PRESENTER.into();
        assert!(lower(&x86_presenter)
            .unwrap_err()
            .to_string()
            .contains("aarch64-product-closure-mismatch"));

        let mut wrong_driver = exact.clone();
        wrong_driver.base_selections[0].driver = FRAMEBUFFER_DRIVER.into();
        assert!(lower(&wrong_driver)
            .unwrap_err()
            .to_string()
            .contains("aarch64-serial-driver-mismatch"));

        let mut proof_leak = exact;
        proof_leak
            .profile_fragments
            .push(SCRIPTED_KEYBOARD_PROOF.into());
        assert!(lower(&proof_leak)
            .unwrap_err()
            .to_string()
            .contains("aarch64-product-closure-mismatch"));
    }
}
