//! Initial construction of the explicitly supported IMAGE-bound timer subset.
//! Serial and storage declarations remain uninitialized and publish no offers.
//! Bounds are logical admission limits, never a claim about process RSS.
use crate::{StdHost, StdHostConfig};
use conduit_core::*;
use conduit_host_make::{
    bind_runtime_offer, HostImage, MakeCatalog, RuntimeFacts, RuntimeOfferInputs,
};
use std::{collections::BTreeSet, io::Write};

mod ambient_face;
mod initial_bases;
mod playback;
mod recognition;
pub(crate) use initial_bases::InitialBases;
pub(crate) use playback::prepare_playback_on;
pub(crate) use recognition::InitialWhisper;

const MAXIMUM_IMAGE_BYTES: usize = 64 * 1024;
const MAXIMUM_TIMER_POOLS: usize = 16;

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
    pub fn from_selected_timer_and_playback_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
        selected: &crate::hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        if !cfg!(conduit_prepared_process_layout) {
            return Err(env!("CONDUIT_PREPARED_PROCESS_LAYOUT").into());
        }
        bounded_image_preflight(&config, image)?;
        let (playback, memory) =
            playback::prepare_playback(&config, image, catalog, checked, selected)?;
        Self::from_selected_initial_bases(
            config.clone(),
            image,
            catalog,
            InitialBases {
                playback: Some(playback),
                memory: Some(memory),
                ..InitialBases::default()
            },
        )
    }
    pub(crate) fn from_selected_initial_bases<'a>(
        config: StdHostConfig,
        image: &'a HostImage,
        catalog: &MakeCatalog,
        mut initial: InitialBases<'a>,
    ) -> Result<Self, String> {
        let has_playback = initial.playback.is_some();
        let has_native = initial.has_native();
        let bytes = image_frame(&config, image)?;
        let manifest = &image.manifest;
        let ambient_face = ambient_face::prepare(&config, image, &initial)?;
        let supported = |id: &str| {
            matches!(
                id,
                conduit_std_offers::TICK_IMPLEMENTATION
                    | conduit_std_offers::TIME_EVERY_IMPLEMENTATION
                    | conduit_std_offers::PATCHBAY_INTERACTION_IMPLEMENTATION
                    | conduit_std_offers::BODY_WAKE_IMPLEMENTATION
                    | conduit_std_offers::SILENT_STARTUP_CHIME_IMPLEMENTATION
            ) || ambient_face.as_ref().is_some_and(|base| {
                base.capabilities
                    .iter()
                    .any(|offer| offer.implementation.implementation_id.as_str() == id)
            }) || initial.supports_native_implementation(id)
                || (has_playback && matches!(id, conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION))
        };
        if !manifest.target.starts_with("std/")
            || manifest.implementations.is_empty()
            || manifest.implementations.iter().any(|id| !supported(id))
        {
            return Err(
                "selected IMAGE operation workset includes an uninitialized implementation".into(),
            );
        }
        let mut timers = manifest.base_selections.iter().filter(|base| {
            base.kind == "timer/monotonic" && base.driver == "hosted/monotonic-clock@1"
        });
        let timer = timers.next();
        if timer.is_none()
            || timers.next().is_some()
            || manifest.base_selections.iter().any(|base| {
                !(has_playback
                    && base.kind == "audio/speaker"
                    && base.driver == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION)
                    && !initial.supports_native_base(&base.id, &base.kind, &base.driver)
                    && !ambient_face
                        .as_ref()
                        .is_some_and(|entry| entry.base_id.as_str() == base.id)
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
        let timer = timer.ok_or("selected IMAGE has no initialized timer")?;
        let budgets = &manifest.resource_budgets;
        if budgets.is_empty() || budgets.len() > MAXIMUM_TIMER_POOLS {
            return Err("selected timer requires 1..16 explicit timer pools".into());
        }
        let mut ids = BTreeSet::new();
        let mut slots = 0u32;
        let mut resources = Vec::with_capacity(budgets.len());
        for budget in budgets {
            if initial.owns_native_pool(&budget.id)
                || ambient_face.as_ref().is_some_and(|entry| {
                    entry
                        .resources
                        .iter()
                        .any(|pool| pool.pool_id.as_str() == budget.id)
                })
            {
                continue;
            }
            if has_playback && budget.class == conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS {
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
        if resources.is_empty() {
            return Err("selected timer requires an explicit timer pool".into());
        }
        if slots > manifest.bounds.timer_slots {
            return Err("aggregate selected timer pools exceed IMAGE timer_slots".into());
        }
        let playback = initial.playback.take();
        // std::time::Instant is this concrete hosted monotonic mechanism. It
        // needs no device discovery or caller-supplied physical bearer.
        let _current = std::time::Instant::now();
        let capabilities = [
            conduit_std_offers::tick_capability_offer(),
            conduit_std_offers::time_every_offer(),
        ]
        .into_iter()
        .filter(|offer| {
            manifest
                .implementations
                .iter()
                .any(|id| id == offer.implementation.implementation_id.as_str())
        })
        .collect();
        let entry = BaseProviderEntry {
            base_id: timer.id.as_str().into(),
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
            capabilities,
            resources,
        };
        let mut registry = crate::empty_base_registry();
        registry
            .register(entry.clone())
            .map_err(|error| format!("selected timer registry: {error:?}"))?;
        let mut candidates = vec![entry.clone()];
        let mut candidate_capabilities = Vec::new();
        let mut ready_resource_classes = BTreeSet::from([TIMER_RESOURCE_CLASS.into()]);
        let mut initialized_base_kinds = BTreeSet::from(["timer/monotonic".into()]);
        let mut initialized_driver_kinds = BTreeSet::from(["hosted/monotonic-clock@1".into()]);
        if let Some(face) = ambient_face {
            registry
                .register(face.clone())
                .map_err(|error| format!("Home Face registry: {error:?}"))?;
            ready_resource_classes.extend(
                face.resources
                    .iter()
                    .map(|pool| pool.class_id.as_str().to_owned()),
            );
            initialized_base_kinds.insert(face.mechanism_family.as_str().into());
            initialized_driver_kinds.insert(face.implementation_id.as_str().into());
            candidates.push(face);
        }
        if let Some((selected, _)) = &playback {
            let mut speaker = selected
                .checked_base_entry()
                .ok_or("selected speaker lacks exact provider identity")?;
            speaker.capabilities.retain(|offer| {
                manifest
                    .implementations
                    .iter()
                    .any(|id| id == offer.implementation.implementation_id.as_str())
            });
            if speaker.capabilities.is_empty() {
                return Err("selected speaker has no built supported audio implementation".into());
            }
            registry
                .register(speaker.clone())
                .map_err(|error| format!("selected speaker registry: {error:?}"))?;
            ready_resource_classes.insert(conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS.into());
            initialized_base_kinds.insert("audio/speaker".into());
            initialized_driver_kinds
                .insert(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION.into());
            candidates.push(speaker);
        }
        if manifest
            .implementations
            .iter()
            .any(|id| id == conduit_std_offers::BODY_WAKE_IMPLEMENTATION)
        {
            candidate_capabilities.push(conduit_std_offers::body_wake_offer());
        }
        if manifest
            .implementations
            .iter()
            .any(|id| id == conduit_std_offers::SILENT_STARTUP_CHIME_IMPLEMENTATION)
        {
            candidate_capabilities.push(conduit_std_offers::silent_startup_chime_offer());
        }
        if manifest
            .implementations
            .iter()
            .any(|id| id == conduit_std_offers::PATCHBAY_INTERACTION_IMPLEMENTATION)
        {
            candidate_capabilities.push(conduit_std_offers::patchbay_interaction_offer());
        }
        initial.project_native(
            &config,
            &mut registry,
            &mut candidates,
            &mut candidate_capabilities,
            &mut ready_resource_classes,
            &mut initialized_base_kinds,
            &mut initialized_driver_kinds,
        )?;
        let binding = bind_runtime_offer(
            manifest,
            image,
            &bytes.bytes,
            catalog,
            RuntimeOfferInputs {
                host_id: config.host_id.clone(),
                boot_id: config.boot_id.clone(),
                offer_generation: config.offer_generation,
                offer_sign_id: format!(
                    "{}/timer-ready/{}",
                    entry.provider_instance_id.as_str(),
                    config.offer_generation.0
                )
                .into(),
                host_profile: manifest.profile_id.as_str().into(),
                candidate_bases: candidates,
                candidate_capabilities,
                planner_capabilities: vec![],
                facts: RuntimeFacts {
                    ready_resource_classes,
                    initialized_base_kinds,
                    initialized_driver_kinds,
                    available_facilities: if has_native {
                        BTreeSet::from(["compositor/native@1".into()])
                    } else {
                        BTreeSet::new()
                    },
                    // The checked provider can offer grant-requiring Backs.
                    // Requirements remain on the offer; this creates no grant.
                    authority_ready: playback.is_some() || has_native || initial.worker.is_some(),
                },
            },
        )
        .map_err(|error| format!("selected timer IMAGE binding: {error:?}"))?;
        let mut host = Self::from_image_binding(binding)?;
        // Presenters are separate initialized realization facts, not additional
        // executable CapabilityOffers. Their prerequisite implementations must
        // each appear exactly once in this complete current offer set.
        let offered_implementations = host
            .advertisement
            .capabilities
            .iter()
            .map(|offer| offer.implementation.implementation_id.as_str())
            .collect::<BTreeSet<_>>();
        let expected_implementations = manifest
            .implementations
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if offered_implementations != expected_implementations
            || host.advertisement.capabilities.len() != manifest.implementations.len()
            || host.advertisement.resources.len() != budgets.len()
        {
            return Err(
                "selected timer did not produce its complete qualified offer subset".into(),
            );
        }
        host.base_registry = registry;
        if let Some(memory) = initial.memory.take() {
            memory.bind(&mut host.kernel_resources, &host.advertisement, image)?;
        } else {
            host.kernel_resources.bind_image_bounds(&manifest.bounds)?;
        }
        if let Some((selected, lease)) = playback {
            host.playback = Some(selected);
            host._playback_memory = Some(lease);
        }
        initial.finish_native(&mut host)?;
        Ok(host)
    }
}

struct ImageBytes {
    bytes: Vec<u8>,
    maximum_bytes: usize,
}
impl Write for ImageBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.maximum_bytes - self.bytes.len() {
            return Err(std::io::Error::other(
                "selected IMAGE exceeds finite preparation frame",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn bounded_image_preflight(
    config: &StdHostConfig,
    image: &HostImage,
) -> Result<(), String> {
    image_frame(config, image).map(drop)
}
fn image_frame(config: &StdHostConfig, image: &HostImage) -> Result<ImageBytes, String> {
    // Bound caller identities before formatting derived provider identities.
    // Reserve their largest fixed suffix in the same finite input frame.
    const IDENTITY_SUFFIX_BYTES: usize = 1 + "/monotonic".len() + "/timer-ready/".len() + 20;
    let identity_bytes = config
        .host_id
        .as_str()
        .len()
        .checked_add(config.boot_id.as_str().len())
        .and_then(|size| size.checked_add(IDENTITY_SUFFIX_BYTES))
        .ok_or("selected timer identity exceeds finite preparation frame")?;
    let maximum_bytes = MAXIMUM_IMAGE_BYTES
        .checked_sub(identity_bytes)
        .filter(|remaining| *remaining > 0)
        .ok_or("selected timer identity exceeds finite preparation frame")?;
    if config.offer_generation.0 == 0
        || config.host_id.as_str().trim().is_empty()
        || config.boot_id.as_str().trim().is_empty()
    {
        return Err(
            "selected timer requires nonempty Host/Boot and a positive offer generation".into(),
        );
    }
    // Validate the finite serialized input BEFORE cloning any authored pool,
    // Base or provider metadata. This is not a managed-heap receipt.
    let mut bytes = ImageBytes {
        bytes: Vec::with_capacity(maximum_bytes),
        maximum_bytes,
    };
    serde_json::to_writer(&mut bytes, image)
        .map_err(|error| format!("selected IMAGE preparation: {error}"))?;
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
    Ok(bytes)
}
