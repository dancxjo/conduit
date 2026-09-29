//! Target-oriented host installation planning.

use conduit_host_fabrication::DeploymentCarrierDescriptor;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub(crate) struct InstallRequest<'a> {
    pub(crate) target: &'a str,
    pub(crate) catalog: &'a Path,
    pub(crate) catalog_id: &'a str,
    pub(crate) mirror: &'a Path,
    pub(crate) cache: &'a Path,
    pub(crate) carrier_descriptors: &'a [PathBuf],
    pub(crate) carrier: Option<&'a str>,
    pub(crate) minimum_generation: u64,
    pub(crate) dry_run: bool,
}

#[derive(Debug, Serialize)]
struct InstallPlan {
    schema: &'static str,
    target_id: String,
    acquisition: crate::release_obtain::ObtainReceipt,
    selected_carrier: DeploymentCarrierDescriptor,
    stages: [&'static str; 3],
    required_authority: bool,
    carrier_effects_performed: bool,
    durable_host_observed: bool,
}

pub(crate) fn run(request: InstallRequest<'_>) -> Result<(), String> {
    if !request.dry_run {
        return Err(
            "host install currently requires --dry-run; carrier effects are not yet integrated"
                .into(),
        );
    }
    let obtained = crate::release_obtain::obtain(
        request.target,
        request.catalog,
        request.catalog_id,
        request.mirror,
        request.cache,
        request.minimum_generation,
    )?;
    let selected = select_carrier(request.target, request.carrier_descriptors, request.carrier)?;
    let plan = InstallPlan {
        schema: "conduit.host/install-plan@1",
        target_id: request.target.into(),
        acquisition: obtained,
        required_authority: selected.requires_explicit_authority,
        selected_carrier: selected,
        stages: [
            "release-acquisition",
            "carrier-authorization",
            "carrier-realization",
        ],
        carrier_effects_performed: false,
        durable_host_observed: false,
    };
    println!(
        "{}",
        serde_json::to_string(&plan)
            .map_err(|error| format!("encode host install plan: {error}"))?
    );
    Ok(())
}

fn select_carrier(
    target: &str,
    paths: &[PathBuf],
    requested: Option<&str>,
) -> Result<DeploymentCarrierDescriptor, String> {
    let mut compatible = paths
        .iter()
        .map(|path| crate::deployment_carrier::read_json::<DeploymentCarrierDescriptor>(path))
        .collect::<Result<Vec<_>, _>>()?;
    for descriptor in &compatible {
        descriptor
            .validate()
            .map_err(|error| format!("carrier descriptor refused: {error:?}"))?;
        if descriptor.target_id != target {
            return Err(format!(
                "carrier {} belongs to target {}, not {target}",
                descriptor.carrier_id, descriptor.target_id
            ));
        }
    }
    compatible.sort_by(|left, right| left.carrier_id.cmp(&right.carrier_id));
    compatible.dedup_by(|left, right| left.carrier_id == right.carrier_id);
    if let Some(requested) = requested {
        return compatible
            .into_iter()
            .find(|descriptor| descriptor.carrier_id == requested)
            .ok_or_else(|| format!("reviewed carrier {requested} is unavailable for {target}"));
    }
    match compatible.len() {
        0 => Err(format!("no reviewed carrier is available for {target}")),
        1 => Ok(compatible.remove(0)),
        _ => Err(format!(
            "multiple reviewed carriers are available for {target}; select one exact --carrier"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_fabrication::DeploymentCarrierKind;
    use std::fs;

    fn descriptor(id: &str, target: &str) -> DeploymentCarrierDescriptor {
        DeploymentCarrierDescriptor {
            schema: "conduit.carrier/descriptor@1".into(),
            carrier_id: id.into(),
            target_id: target.into(),
            kind: DeploymentCarrierKind::NativeInstallStart,
            implementation_id: format!("test/{id}@1"),
            maximum_artifact_bytes: 1024,
            requires_explicit_authority: true,
            verifies_written_bytes: true,
        }
    }

    #[test]
    fn carrier_selection_refuses_ambiguity_and_target_mismatch() {
        let root =
            std::env::temp_dir().join(format!("conduit-install-plan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let paths = [
            descriptor("native-a", "hosted/linux"),
            descriptor("native-b", "hosted/linux"),
        ]
        .into_iter()
        .map(|descriptor| {
            let path = root.join(format!("{}.json", descriptor.carrier_id));
            fs::write(&path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
            path
        })
        .collect::<Vec<_>>();
        assert!(select_carrier("hosted/linux", &paths, None)
            .unwrap_err()
            .contains("multiple"));
        assert_eq!(
            select_carrier("hosted/linux", &paths, Some("native-b"))
                .unwrap()
                .carrier_id,
            "native-b"
        );
        assert!(select_carrier("another/target", &paths[..1], None)
            .unwrap_err()
            .contains("belongs"));
        fs::remove_dir_all(root).unwrap();
    }
}
