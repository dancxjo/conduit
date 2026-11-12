//! Initial IMAGE construction of the existing immediate Source owner. This is
//! not a reconfiguration entrance for an already constructed IMAGE-bound Host.
use super::*;
use crate::{StdHost, StdHostConfig};
use conduit_host_make::{CheckedHostConfiguration, HostImage, MakeCatalog};

type Factory<'a> = Box<dyn FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String> + 'a>;
pub(crate) struct InitialNativeSource<'a> {
    pub definition: NativeSourceDefinition,
    pub budget: ImmediateSourcePreparationBudget,
    // Drop the factory's owned captures before releasing their metadata charge.
    prepare: Factory<'a>,
    metadata_memory: Option<crate::kernel_preparation::ImageMemoryLease>,
}
impl InitialNativeSource<'_> {
    pub(crate) fn validate_image(&self, image: &HostImage) -> Result<(), String> {
        let d = &self.definition;
        let selected = image
            .manifest
            .base_selections
            .iter()
            .filter(|base| base.id.as_str() == d.base.base_id.as_str())
            .collect::<Vec<_>>();
        if selected.len() != 1
            || selected[0].kind != d.base.mechanism_family.as_str()
            || selected[0].driver != d.base.implementation_id.as_str()
            || d.base.capabilities.len() != 1
            || d.base.resources.len() != 2
            || self.budget.retained_bytes == 0
            || self.budget.retained_bytes > self.budget.preparation_bytes
        {
            return Err("initial native Source differs from selected IMAGE equipment".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for (index, resource) in d.base.resources.iter().enumerate() {
            let budgets = image
                .manifest
                .resource_budgets
                .iter()
                .filter(|b| b.id == resource.pool_id.as_str())
                .collect::<Vec<_>>();
            if !ids.insert(&resource.pool_id)
                || budgets.len() != 1
                || budgets[0].class != resource.class_id.as_str()
                || budgets[0].slots != resource.capacity_units
                || budgets[0].bytes
                    < if index == 0 {
                        d.resource_bytes
                    } else {
                        u64::from(self.budget.preparation_bytes)
                    }
            {
                return Err("initial native Source pool differs from selected IMAGE budget".into());
            }
        }
        let memory = &d.base.resources[1];
        if memory.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
            || memory.capacity_units < self.budget.preparation_bytes
            || u64::from(memory.capacity_units) > image.manifest.bounds.heap_arena_bytes
        {
            return Err("initial native Source has insufficient IMAGE memory".into());
        }
        Ok(())
    }
    pub(crate) fn owns_pool(&self, id: &str) -> bool {
        self.definition
            .base
            .resources
            .iter()
            .any(|r| r.pool_id.as_str() == id)
    }
    pub(crate) fn initialize(self, host: &mut StdHost) -> Result<(), String> {
        // The provisional Host and Ready entry are private until this succeeds.
        // Existing native correspondence validation still checks exact grant,
        // Host/Boot, Type, Host Call, resource and provider identities.
        validate_definition(
            &self.definition,
            &host.advertisement,
            Some(self.budget.preparation_bytes),
        )?;
        let metadata = crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes();
        let preparation = (self.budget.preparation_bytes as usize)
            .checked_add(metadata)
            .ok_or("initial native Source preparation size overflow")?;
        let retained = (self.budget.retained_bytes as usize)
            .checked_add(metadata)
            .ok_or("initial native Source retention size overflow")?;
        let mut heap = host
            .kernel_resources
            .reserve_managed_base_memory(preparation)?;
        let mut backing = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &self.definition.base.provider_instance_id,
            self.definition.base.provider_generation,
            &self.definition.base.resources[1].pool_id,
            self.budget.preparation_bytes,
        )?;
        let provider = match (self.prepare)() {
            Ok(provider) => provider,
            Err(error) => {
                host.kernel_resources
                    .release_base_backing(backing)
                    .map_err(|(_, reason)| reason)?;
                return Err(error);
            }
        };
        if let Err(error) = host
            .kernel_resources
            .retain_base_preparation_bytes(&mut backing, self.budget.retained_bytes as usize)
        {
            drop(provider);
            host.kernel_resources
                .release_base_backing(backing)
                .map_err(|(_, reason)| reason)?;
            return Err(error);
        }
        heap.retain_result(retained);
        host.native_source = Some(InitializedNativeSource {
            definition: self.definition,
            provider: NativeProvider::Immediate(provider),
            backing: Some(backing),
            _image_memory: Some(heap),
            _metadata_memory: self.metadata_memory,
        });
        Ok(())
    }
}
impl StdHost {
    /// Construct selected equipment once; reserve backing before native effects
    /// and return a Host only after its immediate provider is initialized.
    ///
    /// # Safety
    /// The definition/grant must correspond to independent current Host and
    /// Source authorization, not received labels. The factory must perform the
    /// selected construction within its declared complete preparation ceiling,
    /// return only after temporary work quiesces, and satisfy the existing
    /// ImmediateNativeSource and finite non-unwinding retirement contracts.
    /// IMAGE intent alone supplies none of those native or authority facts.
    pub unsafe fn from_selected_timer_and_immediate_source_image<'a>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        definition: NativeSourceDefinition,
        budget: ImmediateSourcePreparationBudget,
        prepare: impl FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String> + 'a,
    ) -> Result<Self, String> {
        // SAFETY: the caller supplies the same independent native obligations.
        unsafe {
            Self::from_selected_timer_and_immediate_source_and_creche_image(
                config, image, catalog, checked, definition, budget, None, prepare,
            )
        }
    }

    /// Initial Source composition with an independently prepared encounter slot.
    /// The original encounter provider remains caller-owned.
    ///
    /// # Safety
    /// The same current Host/Source authorization, complete preparation ceiling,
    /// quiescence and immediate-provider retirement obligations as
    /// `from_selected_timer_and_immediate_source_image` apply. Encounter intent
    /// grants no authority and does not construct or replace its provider.
    // Keep the independent checked configuration, grant-bearing definition and
    // preparation budget explicit at this trusted construction boundary.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_timer_and_immediate_source_and_creche_image<'a>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        definition: NativeSourceDefinition,
        budget: ImmediateSourcePreparationBudget,
        creche_resource: Option<ResourceOffer>,
        prepare: impl FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String> + 'a,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        let source = InitialNativeSource {
            definition,
            budget,
            metadata_memory: None,
            prepare: Box::new(prepare),
        };
        source.validate_image(image)?;
        Self::from_selected_source_initial_bases(
            config,
            image,
            catalog,
            source,
            creche_resource,
            None,
            #[cfg(target_os = "linux")]
            None,
        )
    }
    /// Admit native-definition construction before its metadata factory runs.
    /// The verified IMAGE counter is transferred into the final Host unchanged;
    /// metadata retention and provider preparation share its one finite ceiling.
    /// The callback's borrowed Root scope can retain separately qualified values
    /// on that same counter before the Host exists; it confers no native rights.
    ///
    /// # Safety
    /// The original Source/Host grant obligations of the immediate constructor
    /// apply. Root must qualify both factory peaks and retained backing. The
    /// metadata envelope includes the cloned construction configuration, native
    /// definition and unfunded backing in the returned factory's captures.
    /// Separately charged nested values retain their own custody; their backing
    /// need not be charged twice. Borrowed configuration/catalog/schema and
    /// other caller stores remain independently funded. Each factory must return
    /// only after its temporary effects quiesce, including errors and unwinding.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_timer_and_source_factory_image<'a, F, G>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        metadata_budget: ImmediateSourcePreparationBudget,
        creche_resource: Option<ResourceOffer>,
        metadata: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(
            &StdHostConfig,
            &crate::body_execution::ImageRootPreparation<'_>,
        )
            -> Result<(NativeSourceDefinition, ImmediateSourcePreparationBudget, G), String>,
        G: FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String> + 'a,
    {
        use crate::body_execution::ImageRootPreparation;
        use crate::kernel_preparation::StagedImageMemory;
        if metadata_budget.retained_bytes == 0
            || metadata_budget.retained_bytes > metadata_budget.preparation_bytes
        {
            return Err("initial Source metadata has an invalid finite budget".into());
        }
        let memory = StagedImageMemory::verified(&config, image, checked, catalog)?;
        let prepared = {
            let mut reserve = |bytes| memory.reserve(bytes);
            let scope = ImageRootPreparation::new(&mut reserve);
            // SAFETY: caller qualifies all metadata, returned captures and
            // destruction; nested Root values keep their own original charges.
            unsafe {
                scope.prepare_value(
                    metadata_budget.preparation_bytes as usize,
                    metadata_budget.retained_bytes as usize,
                    || Ok((config.clone(), metadata(&config, &scope)?)),
                )
            }?
        };
        let ((final_config, (definition, budget, prepare)), metadata_memory) =
            prepared.into_parts();
        let source = InitialNativeSource {
            definition,
            budget,
            metadata_memory: Some(metadata_memory),
            prepare: Box::new(prepare),
        };
        source.validate_image(image)?;
        Self::from_selected_source_initial_bases(
            final_config,
            image,
            catalog,
            source,
            creche_resource,
            Some(memory),
            #[cfg(target_os = "linux")]
            None,
        )
    }
}
