// Installed permission and candidate construction; no separate executor.
use super::*;
use conduit_body as b;
use conduit_mcu_host_runtime::continuation_native::ContinuationNativeContext;
use core::fmt::Write;
pub struct Installed<'a> {
    pub acceptance: conduit_mcu_host_runtime::root::InstalledContinuationAcceptance,
    pub prefix: &'a [u8; 8192],
}
mod factory {
    include!(concat!(
        env!("OUT_DIR"),
        "/source_runtime_continuation_factory.rs"
    ));
}
pub(super) use factory::Factory;
#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_into<'a, 'f: 'a>(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    installed: &Installed<'_>,
    policy: Policy<'_>,
    factory: Factory<'a, 'f, '_, '_>,
    target: &mut Option<crate::play::ContinuationStarted<'a, 'f>>,
    scopes: &mut Option<crate::play_service::Scopes>,
    activity: &mut Option<&'a conduit_mcu_host_runtime::native_runner::NativeActivity>,
) -> Result<(), &'static str> {
    // This small owned Boot text is the only cloned Root identity. Decoder
    // scratch retires before the one-use mutable Root take.
    if remaining() < 8192 {
        return Err("continuation-decode-budget");
    }
    let boot = root.actual().boot_id.0.clone();
    let (verified, artifact) = {
        let mut scratch = scratch_buffer(8192)?;
        let verified = root
            .verify_installed_continuation_boot_digest(
                &installed.acceptance,
                installed.prefix,
                scratch
                    .as_mut_slice()
                    .try_into()
                    .map_err(|_| "continuation-decode-bounds")?,
                &boot,
                crate::play::continuation::SOURCE_DIGEST,
                &crate::play::continuation::PINS,
            )
            .map_err(|_| "continuation-installed-verification")?;
        let artifact = installed
            .acceptance
            .artifact(
                installed.prefix,
                scratch
                    .as_mut_slice()
                    .try_into()
                    .map_err(|_| "continuation-decode-bounds")?,
            )
            .map_err(|_| "continuation-artifact")?;
        (verified, artifact)
    };
    let parts = root
        .take_continuation_parts(&installed.acceptance)
        .map_err(|_| "continuation-root-attempt")?;
    let bundle = b::check_installed_continuation_bundle(
        &verified,
        crate::play::continuation::SOURCE_DIGEST,
        &crate::play::continuation::PINS,
    )
    .map_err(|_| "continuation-bundle")?;
    let mut documents = [&[][..]; 3];
    let scope = {
        let context = ContinuationNativeContext::check_installation(
            parts.bootstrap,
            parts.resources.host(),
            &verified,
            parts.configuration.image_id,
            parts.configuration.configuration_id,
            policy,
        )
        .map_err(|_| "continuation-native-context")?;
        factory
            .issuers
            .authorize_continuation(
                context,
                policy,
                factory.equipment.pulse.as_ref().ok_or("pulse-busy")?,
                factory.equipment.display.as_ref().ok_or("display-busy")?,
                &factory.equipment.renderer,
                &factory.equipment.waits,
                factory.operations,
                remaining,
            )
            .map_err(|_| "continuation-native-authorization")?;
        match (factory.equipment.recording.selected(), factory.recording) {
            (false, None) => {}
            (true, Some(operation)) => factory
                .equipment
                .recording
                .authorize_continuation(
                    context,
                    policy,
                    &mut factory.issuers.recording,
                    operation,
                    remaining(),
                )
                .map_err(|_| "continuation-recording-authorization")?,
            _ => return Err("continuation-recording-scope"),
        }
        let budget = remaining().min(verified.intent().ceilings.runtime_bytes as usize);
        let material = crate::play::continuation::Material::prepare(
            context,
            policy,
            factory.equipment,
            factory.issuers,
            factory.operations,
            factory.recording,
            budget,
        )
        .map_err(|_| "continuation-material")?;
        let material_bytes = material.retained_backing_bytes();
        let budget = budget
            .checked_sub(material_bytes)
            .ok_or("continuation-material-budget")?;
        let size = [
            source::sampling_layout::PROJECTION_TEMPLATE.len(),
            source::mask_layout::PROJECTION_TEMPLATE.len(),
            source::output_layout::PROJECTION_TEMPLATE.len(),
        ]
        .into_iter()
        .max()
        .ok_or("continuation-template")?;
        if size > remaining() {
            return Err("continuation-scratch-budget");
        }
        let mut scratch = scratch_buffer(size)?;
        factory.diagnostic.clear();
        let _ = write!(factory.diagnostic,
            "budget={budget} material={material_bytes} output={} scratch={size} remaining={} signed={}",
            parts.workspace.len(), remaining(), verified.intent().ceilings.runtime_bytes);
        artifact
            .with_scope(|full| {
                material.with_entries(|pins, entries, resources| {
                    let ranges = b::build_continuation_candidate_with_resources_into::<
                        { crate::play::CONTINUATION_FORE_PORTS },
                    >(
                        &verified,
                        &bundle,
                        full,
                        parts.resources.host(),
                        pins,
                        entries,
                        resources,
                        parts.workspace,
                        &mut scratch,
                        budget,
                    )
                    .map_err(|error| {
                        let _ = write!(factory.diagnostic, " candidate={error:?}");
                        "continuation-candidate"
                    })?;
                    for (index, document) in documents.iter_mut().enumerate() {
                        *document = ranges
                            .document(parts.workspace, index)
                            .ok_or("continuation-document")?;
                    }
                    b::FreshContinuationScope::check_local_replacement::<
                        { crate::play::CONTINUATION_FORE_PORTS },
                    >(
                        &verified,
                        &bundle,
                        full,
                        parts.resources.host(),
                        pins,
                        entries,
                        &documents,
                        &mut scratch,
                    )
                    .map_err(|error| {
                        let _ = write!(factory.diagnostic, " fresh={error:?}");
                        "continuation-fresh-scope"
                    })
                })
            })
            .map_err(|_| "continuation-full-scope")??
    };
    // Material, projection scratch and all construction loans retire before the
    // same whole-workset factory withdraws providers and commits original ledger.
    factory::prepare_into(
        parts.bootstrap,
        &verified,
        parts.configuration,
        &scope,
        parts.resources,
        policy,
        factory,
        target,
        scopes,
        activity,
    )
    .await
}

fn scratch_buffer(length: usize) -> Result<alloc::vec::Vec<u8>, &'static str> {
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| "continuation-scratch-capacity")?;
    bytes.resize(length, 0);
    Ok(bytes)
}

pub(super) fn refusal_response(reason: &str, detail: &str, response: &mut [u8; 512]) -> usize {
    #[derive(serde::Serialize)]
    struct Status<'a> {
        phase: &'a str,
        reason: &'a str,
        detail: &'a str,
    }
    serde_json_core::to_slice(
        &Status {
            phase: "refused-before-play",
            reason,
            detail,
        },
        response,
    )
    .expect("bounded continuation refusal")
}
