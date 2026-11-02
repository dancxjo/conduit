//! Initial construction of the explicitly supported IMAGE-bound timer subset.
//! Serial and storage declarations remain uninitialized and publish no offers.
//! Bounds are logical admission limits, never a claim about process RSS.
use crate::{StdHost, StdHostConfig};
use conduit_core::*;
use conduit_host_make::{
    bind_runtime_offer, HostImage, MakeCatalog, RuntimeFacts, RuntimeOfferInputs,
};
use std::{collections::BTreeSet, io::Write};

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
        if !manifest.target.starts_with("std/")
            || manifest.implementations != [conduit_std_offers::TICK_IMPLEMENTATION]
        {
            return Err("selected IMAGE operation workset is unsupported: only explicit hosted time/tick is initialized".into());
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
                !matches!(
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
        if budgets.is_empty() || budgets.len() > MAXIMUM_TIMER_POOLS {
            return Err("selected timer requires 1..16 explicit timer pools".into());
        }
        let mut ids = BTreeSet::new();
        let mut slots = 0u32;
        let mut resources = Vec::with_capacity(budgets.len());
        for budget in budgets {
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
        if slots > manifest.bounds.timer_slots {
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
                candidate_bases: vec![entry],
                candidate_capabilities: vec![],
                planner_capabilities: vec![],
                facts: RuntimeFacts {
                    ready_resource_classes: BTreeSet::from([TIMER_RESOURCE_CLASS.into()]),
                    initialized_base_kinds: BTreeSet::from(["timer/monotonic".into()]),
                    initialized_driver_kinds: BTreeSet::from(["hosted/monotonic-clock@1".into()]),
                    available_facilities: BTreeSet::new(),
                    authority_ready: false,
                },
            },
        )
        .map_err(|error| format!("selected timer IMAGE binding: {error:?}"))?;
        let mut host = Self::from_image_binding(binding)?;
        if host.advertisement.capabilities.len() != 1
            || host.advertisement.resources.len() != budgets.len()
        {
            return Err(
                "selected timer did not produce its complete qualified offer subset".into(),
            );
        }
        host.base_registry = registry;
        host.kernel_resources.bind_image_bounds(&manifest.bounds)?;
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
