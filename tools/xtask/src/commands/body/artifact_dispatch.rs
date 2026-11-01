//! Runtime-payload admission for all checked Host selections, before effects.
use conduit_body_make::BuiltSpore;
use conduit_host_make::SporeOutputKind;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArtifactProducer {
    BrowserBundle,
}

#[derive(Debug, Serialize)]
pub(super) struct HostArtifactRefusal {
    host: String,
    target: String,
    package: String,
    builder: String,
    output: SporeOutputKind,
    code: &'static str,
    missing: &'static str,
}
#[derive(Debug, Serialize)]
pub(super) struct BatchArtifactRefusal {
    schema: &'static str,
    all_hosts_fabricated: bool,
    refusals: Vec<HostArtifactRefusal>,
}
impl std::fmt::Display for BatchArtifactRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| std::fmt::Error)?
        )
    }
}
impl std::error::Error for BatchArtifactRefusal {}

// Selection comes from the checked make package, never a friendly Host name or
// a guessed board. A packaging class alone does not prove a PROFILE lowerer.
fn select(
    output: &SporeOutputKind,
    builder: &str,
    target: &str,
    package: &str,
) -> Result<ArtifactProducer, &'static str> {
    use SporeOutputKind::*;
    match (output, builder) {
        (BrowserBundle, "conduit-host-browser/bind-prebuilt@1") if target == "browser/wasm32/page" && package == "browser-wasm@1" => Ok(ArtifactProducer::BrowserBundle),
        (BrowserBundle, _) => Err("the selected browser package has no registered configured-runtime producer"),
        (Uf2, _) => Err("a UF2 Spore binder exists, but this selected PROFILE has no admitted runtime-payload producer in ordinary Body fabrication"),
        (Esp32Image, _) => Err("an ESP32 Spore binder exists, but this selected PROFILE has no admitted runtime-payload producer in ordinary Body fabrication"),
        (NativeBundle, _) => Err("reviewed native release packaging exists, but the selected PROFILE and Body binding are not connected to ordinary Body fabrication"),
        (DiskImage, _) => Err("checked ConduitOS image production and explicit provisioning exist, but ordinary Body fabrication has no selected media binding provider"),
        (EfiArtifact, _) => Err("the selected EFI artifact has no registered Body-bound runtime producer"),
        (IntelHex, _) => Err("the selected Intel HEX package has no registered Body-bound runtime producer"),
        (SdImage, _) => Err("the selected SD image package has no registered Body-bound runtime producer"),
    }
}

pub(super) fn plan(spores: &[BuiltSpore]) -> Result<Vec<ArtifactProducer>, BatchArtifactRefusal> {
    let mut producers = Vec::with_capacity(spores.len());
    let mut refusals = Vec::new();
    for spore in spores {
        match select(
            &spore.manifest.output,
            &spore.manifest.make.builder_adapter,
            &spore.manifest.target,
            &spore.manifest.make.make_package_id,
        ) {
            Ok(producer) => producers.push(producer),
            Err(missing) => refusals.push(HostArtifactRefusal {
                host: spore.manifest.host_entry_name.clone(),
                target: spore.manifest.target.clone(),
                package: spore.manifest.make.make_package_id.clone(),
                builder: spore.manifest.make.builder_adapter.clone(),
                output: spore.manifest.output.clone(),
                code: "BodyArtifactProducerUnavailable",
                missing,
            }),
        }
    }
    if refusals.is_empty() {
        Ok(producers)
    } else {
        Err(BatchArtifactRefusal {
            schema: "conduit.body/fabrication-refused@1",
            all_hosts_fabricated: false,
            refusals,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_host_selection_reports_every_missing_producer_before_publication() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let body =
            super::super::load(&root.join("bodies/pete/profiles/pete-r1.body.conduit")).unwrap();
        let spores = conduit_body_make::build_body_spores(
            &body,
            None,
            "test:dispatch",
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
        assert_eq!(spores.len(), 3);
        let refusal = plan(&spores).unwrap_err();
        assert!(!refusal.all_hosts_fabricated);
        assert_eq!(refusal.refusals.len(), 2);
        assert_eq!(
            refusal
                .refusals
                .iter()
                .map(|item| item.host.as_str())
                .collect::<Vec<_>>(),
            ["forebrain", "brainstem"]
        );
        let browser = spores
            .into_iter()
            .filter(|spore| spore.manifest.output == SporeOutputKind::BrowserBundle)
            .collect::<Vec<_>>();
        assert_eq!(plan(&browser).unwrap(), [ArtifactProducer::BrowserBundle]);
    }
    #[test]
    fn a_filename_or_output_class_cannot_substitute_for_the_selected_runtime_producer() {
        assert_eq!(
            select(
                &SporeOutputKind::BrowserBundle,
                "conduit-host-browser/bind-prebuilt@1",
                "browser/wasm32/page",
                "browser-wasm@1"
            ),
            Ok(ArtifactProducer::BrowserBundle)
        );
        assert!(select(
            &SporeOutputKind::BrowserBundle,
            "unreviewed/browser",
            "browser/wasm32/page",
            "browser-wasm@1"
        )
        .is_err());
        assert!(select(
            &SporeOutputKind::Uf2,
            "conduit-host-browser/bind-prebuilt@1",
            "browser/wasm32/page",
            "browser-wasm@1"
        )
        .is_err());
        for output in [
            SporeOutputKind::Uf2,
            SporeOutputKind::Esp32Image,
            SporeOutputKind::NativeBundle,
            SporeOutputKind::DiskImage,
            SporeOutputKind::EfiArtifact,
            SporeOutputKind::IntelHex,
            SporeOutputKind::SdImage,
        ] {
            assert!(select(&output, "descriptor-only", "unknown", "unknown").is_err());
        }
    }
}

/// The whole selected workset remains incomplete if any later producer fails.
/// Individual retained artifacts are useful recovery evidence, not a successful
/// full-Body fabrication receipt.
pub(super) fn record_batch(
    output: &std::path::Path,
    spores: &[BuiltSpore],
    complete: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipt_path = output.join("fabrication-batch.json");
    if !complete && receipt_path.exists() {
        return Err("BodyArtifactBatchExists: choose a new output; the retained fabrication receipt is preserved".into());
    }
    std::fs::create_dir_all(output)?;
    let receipt = serde_json::json!({
        "schema":"conduit.body/fabrication-batch@1",
        "all_hosts_fabricated":complete,
        "membership_admitted":false,
        "host_boot_observed":false,
        "selected_hosts":spores.iter().map(|spore| &spore.manifest.host_entry_name).collect::<Vec<_>>(),
    });
    let temporary = output.join(format!(".fabrication-batch-{}.tmp", std::process::id()));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(&receipt)?)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temporary, receipt_path)?;
    Ok(())
}
