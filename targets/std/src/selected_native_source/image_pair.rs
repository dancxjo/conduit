//! Original IMAGE composition of two distinct affine native owners.
use super::*;
use crate::{StdHost, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};
impl StdHost {
    /// Admit the original immediate compiler and one independently authorized
    /// worker together. Both retain their actual factories, guards and backing
    /// on the same original IMAGE counter; no provider replacement is available.
    ///
    /// # Safety
    /// The caller independently authorizes both exact definitions and native
    /// providers. Metadata covers both factories, captures, definitions and the
    /// configuration clone. Provider budgets cover their complete preparation
    /// and retained backing, excluding only the worker's separately charged
    /// observed stack/control/buffers. Each factory must quiesce temporary work
    /// before returning, including failure/unwind. Captured ownership guards
    /// required for continued execution must remain in the returned providers.
    /// Immediate and cooperative worker contracts continue to apply unchanged.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_timer_and_immediate_and_worker_source_factories_image<
        'a,
        F,
        J,
        G,
        H,
    >(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        metadata_budget: ImmediateSourcePreparationBudget,
        creche_resource: Option<ResourceOffer>,
        #[cfg(target_os = "linux")] text_file: Option<crate::TextFileInitialization>,
        metadata: F,
        worker_metadata: J,
    ) -> Result<Self, String>
    where
        F: FnOnce(
            &StdHostConfig,
            &crate::body_execution::ImageRootPreparation<'_>,
        )
            -> Result<(NativeSourceDefinition, ImmediateSourcePreparationBudget, G), String>,
        J: FnOnce(
            &StdHostConfig,
            &crate::body_execution::ImageRootPreparation<'_>,
        ) -> Result<
            (
                NativeSourceDefinition,
                ImmediateSourcePreparationBudget,
                NativeWorkerBudget,
                H,
            ),
            String,
        >,
        G: FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String> + 'a,
        H: FnOnce() -> Result<Box<dyn NativeSourceWorkerProvider>, String> + 'a,
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
                .is_none_or(|n| n > 64 * 1024)
        {
            return Err(
                "initial Source pair requires finite original metadata/frame admission".into(),
            );
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
            // SAFETY: independently qualified complete pair metadata envelope.
            unsafe {
                scope.prepare_value(
                    metadata_budget.preparation_bytes as usize,
                    metadata_budget.retained_bytes as usize,
                    || {
                        let pair = (
                            metadata(&config, &scope)?,
                            worker_metadata(&config, &scope)?,
                        );
                        image_worker::bound_definition(&pair.0.0)?;
                        image_worker::bound_definition(&pair.1.0)?;
                        Ok((config.clone(), pair))
                    },
                )
            }?
        };
        let (
            (
                config,
                (
                    (first, first_budget, first_factory),
                    (second, provider_budget, worker_budget, second_factory),
                ),
            ),
            metadata_memory,
        ) = prepared.into_parts();
        let second_budget = image_worker::complete_budget(&second, provider_budget, worker_budget)?;
        let second = image::InitialNativeSource {
            companion: None,
            definition: second,
            budget: second_budget,
            prepare: image::Factory::Worker(image_worker::WorkerFactory {
                budget: worker_budget,
                prepare: Box::new(second_factory),
            }),
            metadata_memory: Some(metadata_memory.clone()),
        };
        let source = image::InitialNativeSource {
            companion: Some(Box::new(second)),
            definition: first,
            budget: first_budget,
            prepare: image::Factory::Immediate(Box::new(first_factory)),
            metadata_memory: Some(metadata_memory),
        };
        source.validate_composition(image)?;
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
