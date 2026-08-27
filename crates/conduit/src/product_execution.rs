use conduit_core::{
    BaseImplementationId, HostAdvertisement, LineOffer, Observation, Plan, PlanFragment,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::{ConnectionQueueLimits, PlacementChoices, PlanningOptions};
use conduit_std_host::{load_placements, StdHost, ThreadTimer};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Write;

const MAXIMUM_PRODUCT_HOSTS: usize = 16;
const MAXIMUM_PRODUCT_CONNECTION_BASES: usize = 8;

pub(crate) struct ProductExecution {
    pub(crate) advertisements: Vec<HostAdvertisement>,
    pub(crate) line_offers: Vec<LineOffer>,
    pub(crate) plan: Plan,
    pub(crate) observations: Vec<Observation>,
}

pub(crate) enum ProductRuntime {
    Std(Box<StdHost>),
}

pub(crate) trait ProductLineRuntime {
    fn supports(&self, plan: &Plan) -> bool;
    fn execute(&mut self, plan: &Plan, output: &mut dyn Write) -> Result<Vec<Observation>, String>;
}

impl ProductRuntime {
    pub(crate) fn std(host: StdHost) -> Self {
        Self::Std(Box::new(host))
    }

    fn advertisement(&self) -> &HostAdvertisement {
        match self {
            Self::Std(host) => host.advertisement(),
        }
    }

    fn execute<W: Write>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
    ) -> Result<Vec<Observation>, String> {
        match self {
            Self::Std(host) => host
                .run_fragment_to(fragment, output, &mut ThreadTimer)
                .map(|report| report.observations),
        }
    }
}

/// The finite current product world used to plan and dispatch one installed run.
///
/// Advertisements are planning truth. Runtime handles are exact local execution
/// capabilities and need not exist for every advertised Host. Connection Bases
/// are admitted explicitly; no Base or Host is reconstructed from Form meaning.
pub(crate) struct ProductExecutionContext {
    advertisements: Vec<HostAdvertisement>,
    runtimes: Vec<ProductRuntime>,
    connection_bases: Vec<BaseImplementationId>,
    line_offers: Vec<LineOffer>,
    line_runtimes: Vec<Box<dyn ProductLineRuntime>>,
}

impl ProductExecutionContext {
    #[cfg(test)]
    pub(crate) fn advertisements(&self) -> &[HostAdvertisement] {
        &self.advertisements
    }

    #[cfg(test)]
    pub(crate) fn line_offers(&self) -> &[LineOffer] {
        &self.line_offers
    }
    pub(crate) fn local_std() -> Result<Self, String> {
        let host = StdHost::new();
        let advertisement = host.advertisement().clone();
        Self::new(
            vec![advertisement],
            vec![ProductRuntime::std(host)],
            vec![BaseImplementationId::from("conduit.base/local@1")],
            Vec::new(),
            Vec::new(),
        )
    }

    pub(crate) fn new(
        advertisements: Vec<HostAdvertisement>,
        runtimes: Vec<ProductRuntime>,
        connection_bases: Vec<BaseImplementationId>,
        line_offers: Vec<LineOffer>,
        line_runtimes: Vec<Box<dyn ProductLineRuntime>>,
    ) -> Result<Self, String> {
        if advertisements.is_empty() {
            return Err(
                "product execution context requires at least one Host advertisement".into(),
            );
        }
        if advertisements.len() > MAXIMUM_PRODUCT_HOSTS {
            return Err(format!(
                "product execution context exceeds the {MAXIMUM_PRODUCT_HOSTS}-Host limit"
            ));
        }
        if connection_bases.is_empty() {
            return Err("product execution context requires an admitted connection Base".into());
        }
        if connection_bases.len() > MAXIMUM_PRODUCT_CONNECTION_BASES {
            return Err(format!(
                "product execution context exceeds the {MAXIMUM_PRODUCT_CONNECTION_BASES}-Base limit"
            ));
        }

        let mut host_ids = BTreeSet::new();
        for advertisement in &advertisements {
            if !host_ids.insert(advertisement.host_id.clone()) {
                return Err(format!(
                    "duplicate HostId '{}' in product execution context",
                    advertisement.host_id.as_str()
                ));
            }
        }

        let mut runtime_hosts = BTreeSet::new();
        for runtime in &runtimes {
            let runtime_advertisement = runtime.advertisement();
            if !runtime_hosts.insert(runtime_advertisement.host_id.clone()) {
                return Err(format!(
                    "duplicate runtime for HostId '{}'",
                    runtime_advertisement.host_id.as_str()
                ));
            }
            let advertised = advertisements
                .iter()
                .find(|candidate| candidate.host_id == runtime_advertisement.host_id)
                .ok_or_else(|| {
                    format!(
                        "runtime HostId '{}' has no current advertisement",
                        runtime_advertisement.host_id.as_str()
                    )
                })?;
            if advertised != runtime_advertisement {
                return Err(format!(
                    "runtime for HostId '{}' does not match its current advertisement",
                    runtime_advertisement.host_id.as_str()
                ));
            }
        }

        let mut bases = BTreeSet::new();
        for base in &connection_bases {
            if !bases.insert(base.as_str()) {
                return Err(format!("duplicate admitted connection Base '{base:?}'"));
            }
        }

        Ok(Self {
            advertisements,
            runtimes,
            connection_bases,
            line_offers,
            line_runtimes,
        })
    }

    pub(crate) fn plan(
        &self,
        form: &ExpandedCanonicalForm,
        placements_path: Option<&str>,
    ) -> Result<Plan, String> {
        let placements = load_placements(placements_path).map_err(|error| error.to_string())?;
        let placements = match placements {
            Some(placements) => placements,
            None => self.default_placements(form)?,
        };
        self.plan_with_placements(form, &placements)
    }

    pub(crate) fn default_placements(
        &self,
        form: &ExpandedCanonicalForm,
    ) -> Result<PlacementChoices, String> {
        conduit_planner::default_expanded_placements(form, &self.advertisements)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn plan_with_placements(
        &self,
        form: &ExpandedCanonicalForm,
        placements: &PlacementChoices,
    ) -> Result<Plan, String> {
        let connection_limits = form
            .connections
            .iter()
            .map(|connection| -> Result<_, String> {
                let source = placements
                    .by_gear
                    .get(&connection.source_gear_id)
                    .ok_or_else(|| {
                        format!(
                            "missing placement for '{}'",
                            connection.source_gear_id.as_str()
                        )
                    })?;
                let sink = placements
                    .by_gear
                    .get(&connection.sink_gear_id)
                    .ok_or_else(|| {
                        format!(
                            "missing placement for '{}'",
                            connection.sink_gear_id.as_str()
                        )
                    })?;
                let source_capability = self.capability(source)?;
                let sink_capability = self.capability(sink)?;
                let realization_item_capacity = self
                    .line_item_capacity(source, sink)
                    .unwrap_or(DEFAULT_CONNECTION_ITEM_CAPACITY);
                Ok((
                    (
                        connection.source_gear_id.clone(),
                        connection.source_port_id.clone(),
                        connection.sink_gear_id.clone(),
                        connection.sink_port_id.clone(),
                    ),
                    ConnectionQueueLimits {
                        item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY
                            .min(source_capability.limits.max_queue_items)
                            .min(sink_capability.limits.max_queue_items)
                            .min(realization_item_capacity),
                        byte_capacity: Self::maximum_output_value_bytes(source_capability)
                            .min(Self::maximum_input_value_bytes(sink_capability)),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        conduit_planner::plan_expanded_canonical_with_connection_limits(
            form,
            &self.advertisements,
            placements,
            &self.connection_bases,
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
                connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &self.line_offers,
            },
            &connection_limits,
        )
        .map_err(|error| error.to_string())
    }

    fn capability(
        &self,
        placement: &conduit_planner::PlacementChoice,
    ) -> Result<&conduit_core::CapabilityOffer, String> {
        let host = self
            .advertisements
            .iter()
            .find(|host| host.host_id == placement.host_id)
            .ok_or_else(|| format!("unknown Host '{}'", placement.host_id.as_str()))?;
        host.capabilities
            .iter()
            .find(|capability| capability.capability_id == placement.capability_id)
            .ok_or_else(|| {
                format!(
                    "Host '{}' does not offer capability '{}'",
                    placement.host_id.as_str(),
                    placement.capability_id.as_str()
                )
            })
    }

    fn maximum_output_value_bytes(capability: &conduit_core::CapabilityOffer) -> u32 {
        Self::directional_value_bytes(capability, |operation| operation.maximum_output_bytes)
    }

    fn maximum_input_value_bytes(capability: &conduit_core::CapabilityOffer) -> u32 {
        Self::directional_value_bytes(capability, |operation| operation.maximum_input_bytes)
    }

    /// Finite value storage from the exact implementation offer.
    ///
    /// A directional host-operation bound is authoritative when present. Pure
    /// kernel directions fall back to the capability's finite per-item share;
    /// the opposite endpoint independently narrows the connection intersection.
    fn directional_value_bytes(
        capability: &conduit_core::CapabilityOffer,
        operation_bytes: impl Fn(&conduit_core::HostOperationRequirement) -> u32,
    ) -> u32 {
        let directional = capability
            .host_operations
            .iter()
            .map(operation_bytes)
            .filter(|bytes| *bytes > 0)
            .max();
        directional.unwrap_or_else(|| {
            capability.limits.max_queue_bytes / u32::from(capability.limits.max_queue_items.max(1))
        })
    }

    fn line_item_capacity(
        &self,
        source: &conduit_planner::PlacementChoice,
        sink: &conduit_planner::PlacementChoice,
    ) -> Option<u16> {
        if source.host_id == sink.host_id {
            return None;
        }
        let source_host = self
            .advertisements
            .iter()
            .find(|host| host.host_id == source.host_id)?;
        let sink_host = self
            .advertisements
            .iter()
            .find(|host| host.host_id == sink.host_id)?;
        self.line_offers
            .iter()
            .filter(|offer| {
                offer.binding.source.host_id == source_host.host_id
                    && offer.binding.source.boot_id == source_host.boot_id
                    && offer.binding.sink.host_id == sink_host.host_id
                    && offer.binding.sink.boot_id == sink_host.boot_id
                    && self.connection_bases.contains(&offer.binding.base)
            })
            .map(|offer| offer.binding.limits.maximum_in_flight_items)
            .min()
    }

    pub(crate) fn execute<W: Write>(
        &mut self,
        plan: Plan,
        output: &mut W,
    ) -> Result<ProductExecution, String> {
        self.validate_plan(&plan)?;
        let has_remote_line = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .any(|connection| connection.selected_line.is_some());
        if has_remote_line {
            let mut matching = self
                .line_runtimes
                .iter_mut()
                .filter(|runtime| runtime.supports(&plan))
                .collect::<Vec<_>>();
            if matching.len() != 1 {
                return Err(format!(
                    "planned remote connections require one exact admitted Line runtime; found {}",
                    matching.len()
                ));
            }
            let observations = matching[0].execute(&plan, output)?;
            return Ok(ProductExecution {
                advertisements: self.advertisements.clone(),
                line_offers: self.line_offers.clone(),
                plan,
                observations,
            });
        }
        let mut observations = Vec::new();
        for fragment in plan.fragments.iter().cloned() {
            let runtime = self
                .runtimes
                .iter_mut()
                .find(|runtime| runtime.advertisement().host_id == fragment.host_id)
                .expect("every runtime was admitted before execution");
            observations.extend(runtime.execute(fragment, output)?);
        }
        Ok(ProductExecution {
            advertisements: self.advertisements.clone(),
            line_offers: self.line_offers.clone(),
            plan,
            observations,
        })
    }

    pub(crate) fn validate_plan(&self, plan: &Plan) -> Result<(), String> {
        if !conduit_core::verify_plan(plan) {
            return Err("product execution refused an invalid sealed Plan".into());
        }
        for line in plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .flat_map(|connection| &connection.admitted_lines)
        {
            if !self.connection_bases.contains(&line.binding.base) {
                return Err(format!(
                    "Plan selected connection Base '{:?}' outside the execution context",
                    line.binding.base
                ));
            }
        }
        for fragment in &plan.fragments {
            self.validate_fragment_runtime(fragment)?;
        }
        Ok(())
    }

    fn validate_fragment_runtime(&self, fragment: &PlanFragment) -> Result<(), String> {
        let runtime = self
            .runtimes
            .iter()
            .find(|runtime| runtime.advertisement().host_id == fragment.host_id)
            .ok_or_else(|| {
                format!(
                    "planned local fragment for HostId '{}' has no runtime handle",
                    fragment.host_id.as_str()
                )
            })?;
        let advertisement = runtime.advertisement();
        if advertisement.boot_id != fragment.boot_id
            || advertisement.offer_generation != fragment.offer_generation
        {
            return Err(format!(
                "planned fragment for HostId '{}' has stale Boot/offer identity",
                fragment.host_id.as_str()
            ));
        }
        Ok(())
    }
}
