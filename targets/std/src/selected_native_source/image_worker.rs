//! Startup of the existing native worker under original IMAGE funding.
use super::{
    worker::NativeWorker, NativeSourceDefinition, NativeSourceWorkerProvider, NativeWorkerBudget,
};
use crate::kernel_preparation::{ImageMemoryLease, StagedImageMemory};

pub(crate) struct PreparedImageWorker<'a> {
    definition: NativeSourceDefinition,
    factory: ImageWorkerFactory<'a>,
    memory: ImageMemoryLease,
}

impl PreparedImageWorker<'_> {
    pub(crate) fn definition(&self) -> &NativeSourceDefinition {
        &self.definition
    }

    pub(crate) fn finish(self, host: &mut crate::StdHost) -> Result<(), String> {
        let definition = &self.definition;
        let bytes = self.factory.preparation_bytes(definition)?;
        let bytes = u32::try_from(bytes).map_err(|_| "initial worker Base backing exceeds u32")?;
        super::validate_definition(definition, &host.advertisement, Some(bytes))?;
        let backing = host.kernel_resources.reserve_base_backing(
            &host.advertisement,
            &host.base_registry,
            &definition.base.provider_instance_id,
            definition.base.provider_generation,
            &definition.base.resources[1].pool_id,
            bytes,
        )?;
        let worker = self.factory.start(definition, &self.memory)?;
        host.native_source = Some(super::InitializedNativeSource {
            definition: self.definition,
            provider: super::NativeProvider::Worker(worker),
            backing: Some(backing),
            _image_memory: Some(self.memory),
        });
        Ok(())
    }
}

pub(crate) struct ImageWorkerFactory<'a> {
    pub(crate) budget: NativeWorkerBudget,
    pub(crate) provider_preparation_bytes: usize,
    pub(crate) prepare:
        Box<dyn FnOnce() -> Result<Box<dyn NativeSourceWorkerProvider>, String> + 'a>,
}

impl<'a> ImageWorkerFactory<'a> {
    pub(crate) fn preparation_bytes(
        &self,
        definition: &NativeSourceDefinition,
    ) -> Result<usize, String> {
        let worker = self
            .budget
            .backing_bytes(definition.input_bytes, definition.output_bytes)
            .map_err(|error| format!("initial native worker budget: {error:?}"))?;
        self.provider_preparation_bytes
            .checked_add(worker as usize)
            .ok_or_else(|| "initial native worker preparation overflow".into())
    }

    pub(crate) fn prepare_for_image(
        self,
        definition: NativeSourceDefinition,
        memory: &StagedImageMemory<'_>,
    ) -> Result<PreparedImageWorker<'a>, String> {
        let lease = memory.reserve(self.preparation_bytes(&definition)?)?;
        Ok(PreparedImageWorker {
            definition,
            factory: self,
            memory: lease,
        })
    }

    fn start(
        self,
        definition: &NativeSourceDefinition,
        lease: &ImageMemoryLease,
    ) -> Result<NativeWorker, String> {
        if self.provider_preparation_bytes == 0
            || lease.reserved_bytes() < self.preparation_bytes(definition)?
        {
            return Err(
                "initial native worker requires its complete IMAGE preparation lease".into(),
            );
        }
        let expected = self
            .budget
            .backing_bytes(definition.input_bytes, definition.output_bytes)
            .map_err(|error| format!("initial native worker budget: {error:?}"))?;
        let provider = (self.prepare)()?;
        let worker = NativeWorker::spawn(
            provider,
            self.budget,
            definition.input_bytes,
            definition.output_bytes,
        )
        .map_err(|(_, error)| format!("initial native worker startup: {error:?}"))?;
        let observed = worker.observation();
        if observed.charged_bytes != expected
            || observed
                .stack_bytes
                .checked_add(observed.guard_bytes)
                .is_none_or(|bytes| bytes > self.budget.maximum_stack_mapping_bytes as usize)
        {
            return Err("initial native worker backing differs from its admission".into());
        }
        Ok(worker)
    }
}

fn validate_image_definition(
    image: &conduit_host_make::HostImage,
    definition: &NativeSourceDefinition,
    preparation_bytes: usize,
) -> Result<(), String> {
    let manifest = &image.manifest;
    let base = &definition.base;
    let [offer] = base.capabilities.as_slice() else {
        return Err("selected worker requires exactly one capability".into());
    };
    let [source, memory] = base.resources.as_slice() else {
        return Err("selected worker requires separately checked source and memory pools".into());
    };
    if !manifest.base_selections.iter().any(|selected| {
        selected.id == base.base_id.as_str()
            && selected.kind == base.mechanism_family.as_str()
            && selected.driver == base.implementation_id.as_str()
    }) || !manifest
        .implementations
        .iter()
        .any(|id| id == offer.implementation.implementation_id.as_str())
    {
        return Err("native worker definition differs from the checked IMAGE selection".into());
    }
    for (resource, minimum_bytes) in [
        (source, definition.resource_bytes),
        (
            memory,
            u64::try_from(preparation_bytes).map_err(|_| "worker preparation exceeds u64")?,
        ),
    ] {
        if !manifest.resource_budgets.iter().any(|pool| {
            pool.id == resource.pool_id.as_str()
                && pool.class == resource.class_id.as_str()
                && pool.slots == resource.capacity_units
                && pool.bytes >= minimum_bytes
        }) {
            return Err("native worker resource differs from its checked IMAGE budget".into());
        }
    }
    Ok(())
}

pub(crate) fn stage_native_worker<'a>(
    image: &conduit_host_make::HostImage,
    checked: &conduit_host_make::CheckedHostConfiguration,
    definition: NativeSourceDefinition,
    factory: ImageWorkerFactory<'a>,
    memory: &StagedImageMemory<'_>,
) -> Result<PreparedImageWorker<'a>, String> {
    if definition.base.enforcement_class != conduit_core::BaseEnforcementClass::Cooperative
        || !definition.base.capabilities.first().is_some_and(|offer| {
            offer.implementation.execution_profile_id.as_str()
                == super::NATIVE_SOURCE_WORKER_PROFILE
        })
        || factory.provider_preparation_bytes == 0
    {
        return Err("selected native worker requires its finite cooperative profile".into());
    }
    if matches!(
        definition.base.implementation_id.as_str(),
        conduit_std_offers::AMBIENT_HOME_SOURCE_IMPLEMENTATION
            | conduit_std_offers::AMBIENT_CLOCK_SOURCE_IMPLEMENTATION
    ) {
        let parameters =
            crate::ambient_home_observation::CheckedAmbientHomeSource::from_configuration(checked)?;
        let [source, memory] = definition.base.resources.as_slice() else {
            return Err("Ambient source requires its checked resource pools".into());
        };
        let [timezone] = definition.configuration.as_slice() else {
            return Err("Ambient source requires its exact checked timezone".into());
        };
        if definition.base.implementation_id.as_str() != parameters.implementation
            || source.pool_id.as_str() != parameters.source_pool
            || memory.pool_id.as_str() != parameters.memory_pool
            || timezone.key != "timezone"
            || !matches!(&timezone.value, conduit_core::ConfigurationValue::Text(value) if value == parameters.timezone)
        {
            return Err(
                "Ambient worker definition differs from the checked Base configuration".into(),
            );
        }
    }
    validate_image_definition(image, &definition, factory.preparation_bytes(&definition)?)?;
    factory.prepare_for_image(definition, memory)
}
