//! Exact lowering from a resolved ConduitOS PROFILE to compile and linked-table inputs.

use conduit_host_fabrication::BuildManifest;

use super::ConduitosError;

const NATIVE_PRESENTER: &str = "presenter/native-graphical@1";
const NATIVE_COMPOSITOR: &str = "compositor/native@1";
const PRESENT_OPERATION: &str = "conduit.host/present@1";
const SURFACE_CLASS: &str = "presentation/surface";
const SCANOUT_BASE: &str = "display/scanout";
const FRAMEBUFFER_DRIVER: &str = "display/linear-framebuffer@1";

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
}

pub(super) fn lower(manifest: &BuildManifest) -> Result<TargetBuildInputs, ConduitosError> {
    if manifest.target != "conduitos/x86_64/pc" {
        return Err(refusal(
            "unsupported-profile-target",
            format!("expected conduitos/x86_64/pc, found {}", manifest.target),
        ));
    }

    let native = manifest
        .presenters
        .iter()
        .any(|item| item == NATIVE_PRESENTER);
    let closure = [
        (
            PRESENT_OPERATION,
            manifest.host_operations.contains(&PRESENT_OPERATION.into()),
        ),
        (
            NATIVE_COMPOSITOR,
            manifest.facilities.contains(&NATIVE_COMPOSITOR.into()),
        ),
        (
            SURFACE_CLASS,
            manifest
                .resource_budgets
                .iter()
                .any(|item| item.class == SURFACE_CLASS),
        ),
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
    Ok(TargetBuildInputs {
        cargo_features: native.then_some("native-compositor").into_iter().collect(),
        implementations: backbone
            | if native {
                conduitos::fabrication::IMPL_NATIVE_PRESENTER
            } else {
                0
            },
        facilities: if native {
            conduitos::fabrication::FACILITY_NATIVE_COMPOSITOR
        } else {
            0
        },
        resources: u16::from(native),
        bases: u16::from(native),
        drivers: u16::from(native),
        presenters: u16::from(native),
        proof_instrumentation: 0,
    })
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
        assert_eq!(native.proof_instrumentation, 0);
    }

    #[test]
    fn lowering_rejects_missing_and_leaked_graphical_closure() {
        let mut native = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-native.profile.json"
        ));
        native.driver_selections.clear();
        assert!(lower(&native)
            .unwrap_err()
            .to_string()
            .contains("profile-lowering-prerequisite-missing"));

        let mut headless = manifest(include_str!(
            "../../../../profiles/hosts/conduitos-headless.profile.json"
        ));
        headless.facilities.push(NATIVE_COMPOSITOR.into());
        assert!(lower(&headless)
            .unwrap_err()
            .to_string()
            .contains("headless-graphical-machinery-leaked"));
    }
}
