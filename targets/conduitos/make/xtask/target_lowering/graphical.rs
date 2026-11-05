//! Exact finite graphical closure shared by the serial/framebuffer products.
use super::{refusal, BuildManifest, ConduitosError, PRESENT_OPERATION, SURFACE_CLASS};

const INTERACTION: &str = "conduit.host/face-interaction@1";
const MEMORY: &str = "conduit.resource/runtime-memory@1";
const SHOW: &str = "face.resource/show@1";

pub(super) fn lower(
    manifest: &BuildManifest,
    graphical: bool,
    reason: &'static str,
) -> Result<(u32, u64), ConduitosError> {
    let calls: &[&str] = if graphical {
        &[PRESENT_OPERATION, INTERACTION]
    } else {
        &[PRESENT_OPERATION]
    };
    if manifest.host_calls.len() != calls.len()
        || calls.iter().any(|call| {
            manifest
                .host_calls
                .iter()
                .filter(|item| item.as_str() == *call)
                .count()
                != 1
        })
    {
        return Err(refusal(
            reason,
            format!(
                "expected exactly host-call:{calls:?}, found {:?}",
                manifest.host_calls
            ),
        ));
    }
    let classes: &[&str] = if graphical {
        &[SURFACE_CLASS, MEMORY, SHOW]
    } else {
        &[]
    };
    if manifest.resource_budgets.len() != classes.len()
        || classes.iter().any(|class| {
            manifest
                .resource_budgets
                .iter()
                .filter(|pool| pool.class == *class && pool.slots != 0 && pool.bytes != 0)
                .count()
                != 1
        })
    {
        return Err(refusal(
            reason,
            format!("expected exactly finite resource budgets {classes:?}"),
        ));
    }
    Ok(manifest
        .resource_budgets
        .iter()
        .find(|pool| pool.class == SURFACE_CLASS)
        .map_or((0, 0), |pool| (pool.slots, pool.bytes)))
}

#[cfg(test)]
mod tests {
    use super::super::tests::manifest;
    use super::*;

    #[test]
    fn shipped_proof_profiles_close_the_same_finite_mask_stores() {
        for source in [
            include_str!("../../../proof/profiles/conduitos-proof.profile.json"),
            include_str!("../../../proof/profiles/conduitos-hotplug-proof.profile.json"),
            include_str!("../../../proof/profiles/conduitos-ps2-input.profile.json"),
            include_str!("../../../proof/profiles/conduitos-usb-configuration-proof.profile.json"),
            include_str!("../../../proof/profiles/conduitos-usb-endpoint-read-proof.profile.json"),
            include_str!("../../../proof/profiles/conduitos-usb-hid-endpoint-proof.profile.json"),
            include_str!("../../../proof/profiles/conduitos-usb-hid-mouse-proof.profile.json"),
        ] {
            let checked = manifest(source);
            assert_eq!(
                lower(&checked, true, "proof-closure").unwrap(),
                (4, 8388608)
            );
            assert!(super::super::lower(&checked).is_ok());
        }
    }

    #[test]
    fn every_graphical_product_requires_each_exact_finite_store_and_call() {
        for source in [
            include_str!("../../../profiles/conduitos-aarch64-graphical.host.conduit"),
            include_str!("../../../profiles/conduitos-ia32-graphical.host.conduit"),
            include_str!("../../../profiles/conduitos-riscv64-graphical.host.conduit"),
            include_str!("../../../profiles/conduitos-loongarch64-graphical.host.conduit"),
        ] {
            let original = manifest(source);
            assert!(super::super::lower(&original).is_ok());
            for class in [SURFACE_CLASS, MEMORY, SHOW] {
                for mutation in 0..5 {
                    let mut changed = original.clone();
                    let i = changed
                        .resource_budgets
                        .iter()
                        .position(|pool| pool.class == class)
                        .unwrap();
                    match mutation {
                        0 => {
                            changed.resource_budgets.remove(i);
                        }
                        1 => changed.resource_budgets[i].slots = 0,
                        2 => changed.resource_budgets[i].bytes = 0,
                        3 => changed
                            .resource_budgets
                            .push(changed.resource_budgets[i].clone()),
                        _ => changed.resource_budgets[i].class = "unselected/store".into(),
                    }
                    assert!(
                        super::super::lower(&changed).is_err(),
                        "{class}, mutation {mutation}"
                    );
                }
            }
            for call in [PRESENT_OPERATION, INTERACTION] {
                let mut missing = original.clone();
                missing.host_calls.retain(|item| item != call);
                assert!(super::super::lower(&missing).is_err());
                let mut duplicate = original.clone();
                duplicate.host_calls.push(call.into());
                assert!(super::super::lower(&duplicate).is_err());
            }
        }
    }
}
