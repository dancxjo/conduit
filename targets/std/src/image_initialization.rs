//! Initial construction of explicitly supported IMAGE-bound timer/native owners.
//! Serial and unsupported storage declarations publish no offers.
//! Bounds are logical admission limits, never a claim about process RSS.
use crate::{StdHost, StdHostConfig};
use conduit_body_make::deployment_runtime_catalog::{CRECHE_IMPLEMENTATION, CRECHE_RESOURCE};
use conduit_core::*;
use conduit_host_make::{
    bind_runtime_offer, HostImage, MakeCatalog, RuntimeFacts, RuntimeOfferInputs,
};
use std::{collections::BTreeSet, io::Write};

mod creche;
#[cfg(target_os = "linux")]
mod text_file;
use creche::prepare_creche_entry;

const MAXIMUM_IMAGE_BYTES: usize = 64 * 1024;
const MAXIMUM_TIMER_POOLS: usize = 16;

#[derive(Default)]
struct InitialBases<'a> {
    creche: Option<ResourceOffer>,
    source: Option<crate::selected_native_source::InitialNativeSource<'a>>,
    memory: Option<crate::kernel_preparation::StagedImageMemory<'a>>,
    #[cfg(target_os = "linux")]
    text_file: Option<crate::selected_text_file::InitialTextFile>,
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    scanout: Option<crate::selected_scanout::SelectedScanoutBase>,
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    native_mask: Option<crate::selected_native_mask::InitialNativeMask>,
}
impl InitialBases<'_> {
    fn has_text_file(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.text_file.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
    fn matches_text_file_base(&self, id: &str, kind: &str, driver: &str) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.text_file.as_ref().is_some_and(|f| {
                f.definition.base.base_id.as_str() == id
                    && f.definition.base.mechanism_family.as_str() == kind
                    && f.definition.base.implementation_id.as_str() == driver
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (id, kind, driver);
            false
        }
    }
    fn owns_text_file_pool(&self, id: &str) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.text_file.as_ref().is_some_and(|f| f.owns_pool(id))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = id;
            false
        }
    }
    fn has_native_mask(&self) -> bool {
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.native_mask.is_some()
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
    fn has_scanout(&self) -> bool {
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.scanout.is_some()
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
}

impl StdHost {
    /// Initialize the built-in hosted monotonic timer, retaining its actual
    /// registry and creating the final resource ledger once. No physical file,
    /// serial endpoint, authority, membership or Play is inferred from IMAGE.
    pub fn from_selected_timer_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
    ) -> Result<Self, String> {
        Self::from_selected_initial_bases(config, image, catalog, InitialBases::default())
    }

    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub fn from_selected_timer_and_scanout_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
        parameters: conduit_linear_framebuffer_make::CheckedScanoutInitialization,
        initialized: conduit_linux_display::InitializedDisplay,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected scanout parameters belong to another checked Source".into());
        }
        let scanout = crate::selected_scanout::SelectedScanoutBase::prepare(
            &config,
            image,
            parameters,
            initialized,
        )?;
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                scanout: Some(scanout),
                ..InitialBases::default()
            },
        )
    }

    // Explicit CPU diagnostic only. Production entry never accepts dummy SDL.
    #[cfg(all(test, target_os = "linux", feature = "linux-display"))]
    pub(crate) fn from_selected_scanout_diagnostic(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
        parameters: conduit_linear_framebuffer_make::CheckedScanoutInitialization,
        initialized: conduit_linux_display::InitializedDisplay,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        if checked.configuration_id() != parameters.configuration_id() {
            return Err("selected scanout parameters belong to another checked Source".into());
        }
        let scanout = crate::selected_scanout::SelectedScanoutBase::prepare_diagnostic(
            &config,
            image,
            parameters,
            initialized,
        )?;
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                scanout: Some(scanout),
                ..InitialBases::default()
            },
        )
    }

    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub(crate) fn from_selected_native_initial_bases(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        scanout: crate::selected_scanout::SelectedScanoutBase,
        native_mask: crate::selected_native_mask::InitialNativeMask,
    ) -> Result<Self, String> {
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                scanout: Some(scanout),
                native_mask: Some(native_mask),
                source: None,
                creche: None,
                memory: None,
                #[cfg(target_os = "linux")]
                text_file: None,
            },
        )
    }

    pub(crate) fn from_selected_source_initial_bases<'a>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        source: crate::selected_native_source::InitialNativeSource<'a>,
        creche: Option<ResourceOffer>,
        memory: Option<crate::kernel_preparation::StagedImageMemory<'a>>,
        #[cfg(target_os = "linux")] text_file: Option<crate::selected_text_file::InitialTextFile>,
    ) -> Result<Self, String> {
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                source: Some(source),
                creche,
                memory,
                #[cfg(target_os = "linux")]
                text_file,
                #[cfg(all(target_os = "linux", feature = "linux-display"))]
                scanout: None,
                #[cfg(all(target_os = "linux", feature = "linux-display"))]
                native_mask: None,
            },
        )
    }

    fn from_selected_initial_bases(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        initial: InitialBases<'_>,
    ) -> Result<Self, String> {
        let manifest = &image.manifest;
        macro_rules! same {
            ($($field:ident),+ $(,)?) => { $(if manifest.$field != image.payload.$field {
                return Err(format!("selected IMAGE manifest/payload mismatch: {}", stringify!($field)));
            })+ };
        }
        same!(
            declared_name,
            source_configuration_id,
            target,
            make_descriptor,
            profile_fragments,
            implementations,
            host_calls,
            resources,
            resource_budgets,
            bases,
            base_selections,
            drivers,
            driver_selections,
            lines,
            presenters,
            facilities,
            bounds
        );
        if config.offer_generation.0 == 0 {
            return Err("selected timer requires a positive Boot offer generation".into());
        }
        let creche_entry = initial
            .creche
            .as_ref()
            .map(|resource| prepare_creche_entry(&config, image, resource))
            .transpose()?;
        #[allow(unused_mut)]
        let mut expected_implementations =
            BTreeSet::from([conduit_std_offers::TICK_IMPLEMENTATION.to_owned()]);
        if initial.creche.is_some() {
            expected_implementations.insert(CRECHE_IMPLEMENTATION.into());
        }
        if let Some(source) = &initial.source {
            expected_implementations.insert(
                source.definition.base.capabilities[0]
                    .implementation
                    .implementation_id
                    .as_str()
                    .to_owned(),
            );
        }
        #[cfg(target_os = "linux")]
        if let Some(file) = &initial.text_file {
            file.validate_image(image)?;
            expected_implementations.insert(
                file.definition.base.capabilities[0]
                    .implementation
                    .implementation_id
                    .as_str()
                    .to_owned(),
            );
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if initial.has_native_mask() {
            expected_implementations.extend(
                conduit_conduitos_mask_offer::native_read_only_implementation_closure(
                    conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
                )
                .iter()
                .map(|offer| offer.implementation.implementation_id.as_str().to_owned()),
            );
        }
        if !manifest.target.starts_with("std/")
            || manifest
                .implementations
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected_implementations
        {
            return Err(
                "selected IMAGE operation workset has no selected initialized timer/native owner"
                    .into(),
            );
        }
        let timer = manifest
            .base_selections
            .iter()
            .filter(|base| {
                base.kind == "timer/monotonic" && base.driver == "hosted/monotonic-clock@1"
            })
            .collect::<Vec<_>>();
        if timer.len() != 1
            || manifest.base_selections.iter().any(|base| {
                !creche_entry.as_ref().is_some_and(|entry| {
                    entry.base_id.as_str() == base.id
                        && entry.mechanism_family.as_str() == base.kind
                        && entry.implementation_id.as_str() == base.driver
                }) && !initial.source.as_ref().is_some_and(|source| {
                    source.definition.base.base_id.as_str() == base.id
                        && source.definition.base.mechanism_family.as_str() == base.kind
                        && source.definition.base.implementation_id.as_str() == base.driver
                }) && !initial.matches_text_file_base(&base.id, &base.kind, &base.driver)
                    && !(initial.has_scanout()
                        && base.kind == "display/scanout"
                        && base.driver == "display/linear-framebuffer@1")
                    && !matches!(
                        (base.kind.as_str(), base.driver.as_str()),
                        ("timer/monotonic", "hosted/monotonic-clock@1")
                            | (
                                "serial/text",
                                "hosted/serial@1" | "raspberry-pi-os/serial@1"
                            )
                            | ("storage/protected-file", "hosted/protected-file@1")
                    )
            })
        {
            return Err(
                "selected IMAGE has an unsupported or ambiguous Base initialization".into(),
            );
        }
        let budgets = &manifest.resource_budgets;
        if budgets.is_empty()
            || budgets.len()
                > MAXIMUM_TIMER_POOLS
                    + usize::from(initial.creche.is_some())
                    + usize::from(initial.has_text_file())
                    + usize::from(initial.has_scanout())
                    + 2 * usize::from(initial.has_native_mask())
                    + initial
                        .source
                        .as_ref()
                        .map_or(0, |s| s.definition.base.resources.len())
        {
            return Err("selected timer requires 1..16 explicit timer pools".into());
        }
        let mut ids = BTreeSet::new();
        let mut slots = 0u32;
        let mut resources = Vec::with_capacity(budgets.len());
        for budget in budgets {
            if initial.owns_text_file_pool(&budget.id) {
                continue;
            }

            if initial
                .creche
                .as_ref()
                .is_some_and(|resource| resource.pool_id.as_str() == budget.id)
            {
                continue;
            }
            if initial
                .source
                .as_ref()
                .is_some_and(|source| source.owns_pool(&budget.id))
            {
                continue;
            }
            if (initial.has_scanout() && budget.class == "face/surface")
                || (initial.has_native_mask()
                    && matches!(
                        budget.class.as_str(),
                        conduit_face::SHOW_RESOURCE_CLASS | RUNTIME_MEMORY_RESOURCE_CLASS
                    ))
            {
                continue;
            }
            if budget.class != TIMER_RESOURCE_CLASS
                || budget.slots == 0
                || budget.bytes == 0
                || budget.id.trim().is_empty()
                || !ids.insert(&budget.id)
            {
                return Err("selected IMAGE resource budget is unsupported or duplicated; only positive explicit timer pools are initialized".into());
            }
            slots = slots
                .checked_add(budget.slots)
                .ok_or("selected timer slot sum overflow")?;
            resources.push(resource_offer(
                &budget.id,
                TIMER_RESOURCE_CLASS,
                budget.slots,
            ));
        }
        if slots == 0 || slots > manifest.bounds.timer_slots {
            return Err("aggregate selected timer pools exceed IMAGE timer_slots".into());
        }
        // std::time::Instant is this concrete hosted monotonic mechanism. It
        // needs no device discovery or caller-supplied physical bearer.
        let _current = std::time::Instant::now();
        let entry = BaseProviderEntry {
            base_id: timer[0].id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/monotonic",
                config.host_id.as_str(),
                config.boot_id.as_str()
            )
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: "hosted/monotonic-clock@1".into(),
            mechanism_family: "timer/monotonic".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![conduit_std_offers::tick_capability_offer()],
            resources,
        };
        let mut registry = crate::empty_base_registry();
        registry
            .register(entry.clone())
            .map_err(|error| format!("selected timer registry: {error:?}"))?;
        #[allow(unused_mut)]
        let mut candidates = vec![entry.clone()];
        #[allow(unused_mut)]
        let mut ready_resource_classes = BTreeSet::from([TIMER_RESOURCE_CLASS.into()]);
        #[allow(unused_mut)]
        let mut initialized_base_kinds = BTreeSet::from(["timer/monotonic".into()]);
        #[allow(unused_mut)]
        let mut initialized_driver_kinds = BTreeSet::from(["hosted/monotonic-clock@1".into()]);
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(scanout) = &initial.scanout {
            registry
                .register(scanout.entry().clone())
                .map_err(|error| format!("selected scanout registry: {error:?}"))?;
            candidates.push(scanout.entry().clone());
            ready_resource_classes.insert("face/surface".into());
            initialized_base_kinds.insert("display/scanout".into());
            initialized_driver_kinds.insert("display/linear-framebuffer@1".into());
        }
        #[cfg(target_os = "linux")]
        if let Some(file) = &initial.text_file {
            let entry = &file.definition.base;
            registry
                .register(entry.clone())
                .map_err(|error| format!("selected file registry: {error:?}"))?;
            candidates.push(entry.clone());
            ready_resource_classes.extend(
                entry
                    .resources
                    .iter()
                    .map(|r| r.class_id.as_str().to_owned()),
            );
            initialized_base_kinds.insert(entry.mechanism_family.as_str().into());
            initialized_driver_kinds.insert(entry.implementation_id.as_str().into());
        }
        if let Some(source) = &initial.source {
            let entry = &source.definition.base;
            registry
                .register(entry.clone())
                .map_err(|error| format!("selected Source registry: {error:?}"))?;
            candidates.push(entry.clone());
            ready_resource_classes.extend(
                entry
                    .resources
                    .iter()
                    .map(|r| r.class_id.as_str().to_owned()),
            );
            initialized_base_kinds.insert(entry.mechanism_family.as_str().into());
            initialized_driver_kinds.insert(entry.implementation_id.as_str().into());
        }
        if let Some(entry) = creche_entry {
            registry
                .register(entry.clone())
                .map_err(|error| format!("selected Creche registry: {error:?}"))?;
            ready_resource_classes.insert(CRECHE_RESOURCE.into());
            initialized_base_kinds.insert(entry.mechanism_family.as_str().into());
            initialized_driver_kinds.insert(CRECHE_IMPLEMENTATION.into());
            candidates.push(entry);
        }
        #[allow(unused_mut)]
        let mut mask_capabilities = Vec::new();
        #[allow(unused_mut)]
        let mut facilities = BTreeSet::new();
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(mask) = &initial.native_mask {
            let offer = mask.offers(&config);
            mask_capabilities = offer
                .capabilities
                .into_iter()
                .filter(|capability| {
                    !mask
                        .provider
                        .entry
                        .capabilities
                        .iter()
                        .any(|owned| owned.capability_id == capability.capability_id)
                })
                .collect();
            ready_resource_classes.insert(conduit_face::SHOW_RESOURCE_CLASS.into());
            ready_resource_classes.insert(RUNTIME_MEMORY_RESOURCE_CLASS.into());
            facilities.insert("compositor/native@1".into());
        }
        #[allow(unused_mut)]
        let mut expected_capabilities = 1
            + usize::from(initial.has_text_file())
            + usize::from(initial.creche.is_some())
            + mask_capabilities.len()
            + initial
                .source
                .as_ref()
                .map_or(0, |s| s.definition.base.capabilities.len());
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(mask) = &initial.native_mask {
            expected_capabilities += mask.provider.entry.capabilities.len();
        }
        let has_native_mask = initial.has_native_mask();
        let mut bytes = ImageBytes(Vec::with_capacity(MAXIMUM_IMAGE_BYTES));
        serde_json::to_writer(&mut bytes, image)
            .map_err(|error| format!("selected IMAGE preparation: {error}"))?;
        let binding = bind_runtime_offer(
            manifest,
            image,
            &bytes.0,
            catalog,
            RuntimeOfferInputs {
                host_id: config.host_id,
                boot_id: config.boot_id,
                offer_generation: config.offer_generation,
                offer_sign_id: format!(
                    "{}/timer-ready/{}",
                    entry.provider_instance_id.as_str(),
                    config.offer_generation.0
                )
                .into(),
                host_profile: manifest.profile_id.as_str().into(),
                candidate_bases: candidates,
                candidate_capabilities: mask_capabilities,
                planner_capabilities: vec![],
                facts: RuntimeFacts {
                    ready_resource_classes,
                    initialized_base_kinds,
                    initialized_driver_kinds,
                    available_facilities: facilities,
                    authority_ready: has_native_mask
                        || initial.source.is_some()
                        || initial.has_text_file(),
                },
            },
        )
        .map_err(|error| format!("selected timer IMAGE binding: {error:?}"))?;
        let mut host = Self::from_image_binding(binding)?;
        if host.advertisement.capabilities.len() != expected_capabilities
            || host.advertisement.resources.len() != budgets.len()
        {
            return Err(format!("selected IMAGE did not produce its complete qualified offer subset: capabilities {}/{expected_capabilities}, resources {}/{}",
                host.advertisement.capabilities.len(), host.advertisement.resources.len(), budgets.len()));
        }
        host.base_registry = registry;
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            host.selected_scanout = initial.scanout;
        }
        if let Some(memory) = initial.memory {
            memory.bind(&mut host.kernel_resources, &host.advertisement, image)?;
        } else {
            host.kernel_resources.bind_image_bounds(&manifest.bounds)?;
        }
        if let Some(source) = initial.source {
            source.initialize(&mut host)?;
        }
        #[cfg(target_os = "linux")]
        if let Some(file) = initial.text_file {
            file.initialize(&mut host)?;
        }

        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(intent) = initial.native_mask {
            let mask = intent.initialize(&mut host)?;
            host.selected_scanout
                .as_mut()
                .ok_or("selected native Mask has no physical owner")?
                .native_mask = Some(mask);
        }
        Ok(host)
    }
}

struct ImageBytes(Vec<u8>);
impl Write for ImageBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAXIMUM_IMAGE_BYTES - self.0.len() {
            return Err(std::io::Error::other(
                "selected IMAGE exceeds finite preparation frame",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
