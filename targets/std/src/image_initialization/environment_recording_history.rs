//! Funded original recording capture and ordinary quantity Face on one IMAGE.
use super::*;
use crate::{
    environment_recording_history::CapturedSnapshotSelection,
    environment_recording_history::{
        prepare_capture, provider_preparation_bytes, CheckedEnvironmentRecordingHistory,
    },
    kernel_preparation::StagedImageMemory,
    selected_native_source::image_worker::{stage_native_worker, ImageWorkerFactory},
};
use conduit_dht11_observation::ObservationContract;
use conduit_host_make::CheckedHostConfiguration;
use conduit_std_offers::{
    ENVIRONMENT_RECORDING_HISTORY_BASE as BASE,
    ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION as IMPLEMENTATION,
    ENVIRONMENT_RECORDING_HISTORY_RESOURCE as RESOURCE,
};
use sha2::{Digest, Sha256};
use std::time::Duration;

impl StdHost {
    /// Initialize the selected original-recording Source and shared quantity Face.
    /// Reported recording context selects content interpretation, not authority.
    ///
    /// # Safety
    /// Root must retain the actual directory mapping, independently authorize
    /// its exact current read selection and the separate Source-use grant, and
    /// ensure configured directory selectors correspond to held descriptors.
    /// The IMAGE, references and producer metadata establish no IO authority,
    /// durable storage, publication ACK, producer trust or restart continuity.
    pub unsafe fn from_selected_environment_recording_history_image<'a>(
        config: StdHostConfig,
        image: &'a HostImage,
        catalog: &MakeCatalog,
        checked: &'a CheckedHostConfiguration,
        source_grant: AuthorityGrant,
        capture: CapturedSnapshotSelection<'a>,
    ) -> Result<Self, String> {
        bounded_image_preflight(&config, image)?;
        let memory = StagedImageMemory::verified(&config, image, checked, catalog)?;
        let parameters = CheckedEnvironmentRecordingHistory::from_configuration(checked)?;
        let definition = definition(&config, image, checked, source_grant, &parameters, capture)?;
        // Known checked schema construction is caller/catalog preparation, never
        // decoding of the original recording. Its retained material is charged
        // with the factory before the closure can perform capture/conversion.
        let contract = ObservationContract::prepare()
            .map_err(|why| format!("environment recording observation contract: {why:?}"))?;
        let preparation_bytes = provider_preparation_bytes(&contract)?
            .checked_add(
                contract
                    .retained_backing_bytes()
                    .ok_or("environment recording contract backing overflow")?,
            )
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<ObservationContract>()))
            .ok_or("environment recording factory preparation backing overflow")?;
        let factory = ImageWorkerFactory {
            budget: crate::NativeWorkerBudget {
                requested_stack_bytes: 2 * 1024 * 1024,
                maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
                capture_timeout: Duration::from_secs(3),
                cancellation_timeout: Duration::from_millis(250),
            },
            provider_preparation_bytes: preparation_bytes,
            prepare: Box::new(move || {
                Ok(Box::new(prepare_capture(&contract, &parameters, capture)?))
            }),
        };
        let worker = stage_native_worker(image, checked, definition, factory, &memory)?;
        let mut initial = InitialBases {
            worker: Some(worker),
            memory: Some(memory),
            ..InitialBases::default()
        };
        initial.quantity_face = Some(super::quantity_face::prepare(
            &config, image, catalog, checked, &initial,
        )?);
        Self::from_selected_initial_bases(config.clone(), image, catalog, initial)
    }
}
fn definition(
    config: &StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
    grant: AuthorityGrant,
    parameters: &CheckedEnvironmentRecordingHistory<'_>,
    capture: CapturedSnapshotSelection<'_>,
) -> Result<crate::NativeSourceDefinition, String> {
    crate::environment_recording_history::validate_capture_selection(&capture)?;
    if capture.selected.host != &config.host_id || capture.selected.boot != &config.boot_id {
        return Err(
            "environment recording requires an independently held current Host/Boot read selection"
                .into(),
        );
    }
    if parameters
        .reference
        .as_ref()
        .is_some_and(|reference| reference != capture.reference)
    {
        return Err(
            "environment recording reference differs from checked Host Source selection".into(),
        );
    }
    let mut selections = image
        .manifest
        .base_selections
        .iter()
        .filter(|base| base.kind == BASE && base.driver == IMPLEMENTATION);
    let selected = selections
        .next()
        .ok_or("environment recording Source absent from IMAGE")?;
    if selections.next().is_some() {
        return Err("environment recording Source IMAGE selection is ambiguous".into());
    }
    let pool = |id: &str, class: &str| {
        image
            .manifest
            .resource_budgets
            .iter()
            .find(|pool| pool.id == id && pool.class == class)
            .map(|pool| resource_offer(&pool.id, &pool.class, pool.slots))
            .ok_or_else(|| format!("environment recording lacks checked {id} pool"))
    };
    // Exact initialized generation identity, never a content/commit receipt.
    // The complete checked configuration binds interpretation parameters while
    // current Root scope and original reference bind actual capture selection.
    let reference = capture
        .reference
        .encode()
        .map_err(|why| format!("environment recording reference: {why:?}"))?;
    let mut digest = Sha256::new();
    digest.update(b"conduit/environment-recording-history-generation@1");
    for part in [
        checked.configuration_id().as_bytes(),
        config.host_id.as_str().as_bytes(),
        config.boot_id.as_str().as_bytes(),
        reference.as_slice(),
    ] {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    let generation = format!("sha256:{:x}", digest.finalize());
    let maximum = conduit_data::resolved_quantity_observation_snapshot_byte_bound()
        .map_err(|why| format!("environment recording output: {why:?}"))?;
    Ok(crate::NativeSourceDefinition {
        kind: conduit_semantic_catalog::environment_recording_history_kind()?,
        base: BaseProviderEntry {
            base_id: selected.id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/environment-recording-history",
                config.host_id.as_str(),
                config.boot_id.as_str()
            )
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: IMPLEMENTATION.into(),
            mechanism_family: BASE.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![conduit_std_offers::environment_recording_history_offer()?],
            resources: vec![
                pool(parameters.source_pool, RESOURCE)?,
                pool(parameters.memory_pool, RUNTIME_MEMORY_RESOURCE_CLASS)?,
            ],
        },
        grant,
        resource_generation: ResourceGenerationId(generation),
        configuration: vec![],
        input_bytes: conduit_time::TICK_ENCODED_LEN,
        output_bytes: maximum,
        resource_bytes: u64::from(maximum),
    })
}
