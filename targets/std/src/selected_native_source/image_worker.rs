//! Original IMAGE admission for the existing cooperative native worker.
use super::*;
use crate::{StdHost, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};

pub(super) struct WorkerFactory<'a> {
    pub(super) budget: NativeWorkerBudget,
    pub(super) prepare:
        Box<dyn FnOnce() -> Result<Box<dyn NativeSourceWorkerProvider>, String> + 'a>,
}
impl WorkerFactory<'_> {
    pub(super) fn prepare(self, input: u32, output: u32) -> Result<worker::NativeWorker, String> {
        let expected = self
            .budget
            .backing_bytes(input, output)
            .map_err(|e| format!("initial native worker profile: {e:?}"))?;
        let provider = (self.prepare)()?;
        let worker = worker::NativeWorker::spawn(provider, self.budget, input, output).map_err(
            |(provider, error)| {
                // Failed startup has already joined; drop the affine owner while
                // its complete IMAGE and Base reservations are still retained.
                drop(provider);
                format!("initial native worker startup: {error:?}")
            },
        )?;
        let observed = worker.observation();
        if observed.charged_bytes != expected
            || observed
                .stack_bytes
                .checked_add(observed.guard_bytes)
                .is_none_or(|bytes| bytes > self.budget.maximum_stack_mapping_bytes as usize)
        {
            // Drop joins before the caller releases either reservation.
            drop(worker);
            return Err("initial native worker backing observation differs from admission".into());
        }
        Ok(worker)
    }
}

pub(super) fn complete_budget(
    definition: &NativeSourceDefinition,
    provider: ImmediateSourcePreparationBudget,
    worker: NativeWorkerBudget,
) -> Result<ImmediateSourcePreparationBudget, String> {
    if definition.base.enforcement_class != BaseEnforcementClass::Cooperative
        || definition.base.capabilities.first().is_none_or(|offer| {
            offer.implementation.execution_profile_id.as_str() != NATIVE_SOURCE_WORKER_PROFILE
        })
        || provider.retained_bytes == 0
        || provider.retained_bytes > provider.preparation_bytes
    {
        return Err(
            "initial native worker requires its explicit profile and finite provider budget".into(),
        );
    }
    let backing = worker
        .backing_bytes(definition.input_bytes, definition.output_bytes)
        .map_err(|e| format!("initial native worker profile: {e:?}"))?;
    Ok(ImmediateSourcePreparationBudget {
        preparation_bytes: provider
            .preparation_bytes
            .checked_add(backing)
            .ok_or("initial native worker preparation overflow")?,
        retained_bytes: provider
            .retained_bytes
            .checked_add(backing)
            .ok_or("initial native worker retention overflow")?,
    })
}

// Streaming serialization refuses before any unbounded serialized clone. The
// caller still owns and independently funds its original definition.
pub(super) fn bound_definition(d: &NativeSourceDefinition) -> Result<(), String> {
    struct Frame(usize);
    impl std::io::Write for Frame {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len())
                .filter(|n| *n <= 64 * 1024)
                .ok_or_else(|| {
                    std::io::Error::other("initial Source exceeds finite IMAGE frame")
                })?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(
        &mut Frame(0),
        &(
            &(
                &d.kind.startup_parameters,
                &d.kind.shorthand,
                &d.kind.kind_id,
                &d.kind.kind_contract_revision,
                &d.kind.inputs,
                &d.kind.outputs,
                &d.kind.configuration,
                &d.kind.semantic_laws,
                &d.kind.limits,
            ),
            &d.base,
            &d.grant,
            &d.resource_generation,
            &d.configuration,
            d.input_bytes,
            d.output_bytes,
            d.resource_bytes,
        ),
    )
    .map_err(|e| format!("initial native worker definition: {e}"))
}
impl StdHost {
    /// Construct the original IMAGE's selected Source with the existing worker.
    /// Provider construction and worker backing share the original memory
    /// counter; the selected Base pool must cover their combined peak.
    ///
    /// # Safety
    /// Root must independently authorize the exact Host/Boot, definition and
    /// grant and qualify both metadata and provider budgets. Metadata covers
    /// configuration, definition and unfunded factory captures. The provider
    /// budget covers its actual preparation peak and retained backing, excluding
    /// only this worker's separately measured stack/control/buffers. Factories
    /// must return with temporary work quiescent and destruction must not unwind.
    /// The provider must satisfy NativeSourceWorkerProvider's cooperative bounds;
    /// these accounting observations do not establish hostile-code confinement.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_timer_and_worker_source_factory_image<'a, F, G>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        metadata_budget: ImmediateSourcePreparationBudget,
        creche_resource: Option<ResourceOffer>,
        #[cfg(target_os = "linux")] text_file: Option<crate::TextFileInitialization>,
        metadata: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(
            &StdHostConfig,
            &crate::body_execution::ImageRootPreparation<'_>,
        ) -> Result<
            (
                NativeSourceDefinition,
                ImmediateSourcePreparationBudget,
                NativeWorkerBudget,
                G,
            ),
            String,
        >,
        G: FnOnce() -> Result<Box<dyn NativeSourceWorkerProvider>, String> + 'a,
    {
        use crate::body_execution::ImageRootPreparation;
        use crate::kernel_preparation::StagedImageMemory;
        if metadata_budget.retained_bytes == 0
            || metadata_budget.retained_bytes > metadata_budget.preparation_bytes
            || config
                .host_id
                .as_str()
                .len()
                .checked_add(config.boot_id.as_str().len())
                .is_none_or(|bytes| bytes > 64 * 1024)
        {
            return Err("initial worker Source metadata has an invalid finite budget/frame".into());
        }
        let memory = StagedImageMemory::verified(&config, image, checked, catalog)?;
        #[cfg(target_os = "linux")]
        let text_file = text_file
            .map(|file| {
                crate::image_initialization::validate_checked_file_definition(
                    checked,
                    &file.definition,
                )?;
                crate::selected_text_file::InitialTextFile::prepare(&config, image, file)
            })
            .transpose()?;
        let prepared = {
            let mut reserve = |bytes| memory.reserve(bytes);
            let scope = ImageRootPreparation::new(&mut reserve);
            // SAFETY: caller qualifies complete captures and native destruction.
            unsafe {
                scope.prepare_value(
                    metadata_budget.preparation_bytes as usize,
                    metadata_budget.retained_bytes as usize,
                    || {
                        let result = metadata(&config, &scope)?;
                        bound_definition(&result.0)?;
                        Ok((config.clone(), result))
                    },
                )
            }?
        };
        let ((config, (definition, provider_budget, worker_budget, prepare)), metadata_memory) =
            prepared.into_parts();
        let budget = complete_budget(&definition, provider_budget, worker_budget)?;
        let source = image::InitialNativeSource {
            companion: None,
            definition,
            budget,
            prepare: image::Factory::Worker(WorkerFactory {
                budget: worker_budget,
                prepare: Box::new(prepare),
            }),
            metadata_memory: Some(metadata_memory),
        };
        source.validate_image(image)?;
        let cpu =
            crate::image_initialization::prepare_cpu_from_configuration(&config, image, checked)?;
        Self::from_selected_source_initial_bases(
            config,
            image,
            catalog,
            source,
            cpu,
            creche_resource,
            Some(memory),
            #[cfg(target_os = "linux")]
            text_file,
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "image_worker/image_tests.rs"]
mod image_tests;
