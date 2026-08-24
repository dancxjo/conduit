use conduit_core::{ConnectionBase, HostAdvertisement, Observation, Plan, PlanFragment};
use conduit_form::ExpandedCanonicalForm;
use conduit_planner::PlacementChoices;
use conduit_std_host::{load_placements, StdHost, ThreadTimer};
use std::collections::BTreeSet;
use std::io::Write;

const MAXIMUM_PRODUCT_HOSTS: usize = 16;
const MAXIMUM_PRODUCT_CONNECTION_BASES: usize = 8;

pub(crate) struct ProductExecution {
    pub(crate) advertisements: Vec<HostAdvertisement>,
    pub(crate) plan: Plan,
    pub(crate) observations: Vec<Observation>,
}

pub(crate) enum ProductRuntime {
    Std(StdHost),
}

impl ProductRuntime {
    pub(crate) fn std(host: StdHost) -> Self {
        Self::Std(host)
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
    connection_bases: Vec<ConnectionBase>,
}

impl ProductExecutionContext {
    pub(crate) fn local_std() -> Result<Self, String> {
        let host = StdHost::new();
        let advertisement = host.advertisement().clone();
        Self::new(
            vec![advertisement],
            vec![ProductRuntime::std(host)],
            vec![ConnectionBase::Local],
        )
    }

    pub(crate) fn new(
        advertisements: Vec<HostAdvertisement>,
        runtimes: Vec<ProductRuntime>,
        connection_bases: Vec<ConnectionBase>,
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
            if matches!(
                base,
                ConnectionBase::InMemory
                    | ConnectionBase::FixtureFrame
                    | ConnectionBase::FixtureDatagram
            ) {
                return Err(format!(
                    "connection Base '{base:?}' is not supported by installed product execution"
                ));
            }
            if !bases.insert(base.canonical_code()) {
                return Err(format!("duplicate admitted connection Base '{base:?}'"));
            }
        }

        Ok(Self {
            advertisements,
            runtimes,
            connection_bases,
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
        conduit_planner::plan_expanded_canonical(
            form,
            &self.advertisements,
            placements,
            &self.connection_bases,
        )
        .map_err(|error| error.to_string())
    }

    pub(crate) fn execute<W: Write>(
        &mut self,
        plan: Plan,
        output: &mut W,
    ) -> Result<ProductExecution, String> {
        if !conduit_core::verify_plan(&plan) {
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
            plan,
            observations,
        })
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
