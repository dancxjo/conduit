//! Allocation-free complete storage accounting for the admitted fixed mixed Plan.
//! Unsupported nested owners refuse; empty Vec spare capacity is still charged.
use alloc::vec::Vec;
use conduit_core::*;
use core::mem::{align_of, size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericPlanStorageRefusal {
    Overflow,
    Unsupported,
    Pressure,
}
struct Counter(usize);
impl Counter {
    fn add(&mut self, n: usize) -> Result<(), NumericPlanStorageRefusal> {
        self.0 = self
            .0
            .checked_add(n)
            .ok_or(NumericPlanStorageRefusal::Overflow)?;
        Ok(())
    }
    fn vector<T>(&mut self, v: &Vec<T>) -> Result<(), NumericPlanStorageRefusal> {
        self.add(
            v.capacity()
                .checked_mul(size_of::<T>())
                .ok_or(NumericPlanStorageRefusal::Overflow)?,
        )
    }
    fn contract(&mut self, c: &CheckedValueContract) -> Result<(), NumericPlanStorageRefusal> {
        self.add(c.value_kind.0.capacity())?;
        self.vector(&c.constraints)?;
        if !c.constraints.is_empty() {
            return Err(NumericPlanStorageRefusal::Unsupported);
        }
        Ok(())
    }
    fn resource(&mut self, r: &ResourceBinding) -> Result<(), NumericPlanStorageRefusal> {
        self.add(r.pool_id.0.capacity())?;
        self.add(r.class_id.0.capacity())?;
        if r.protected.is_some() {
            return Err(NumericPlanStorageRefusal::Unsupported);
        }
        if let Some(c) = &r.compute {
            self.add(c.architecture_base_id.0.capacity())?;
            self.add(c.topology_group_id.as_ref().map_or(0, |v| v.0.capacity()))?;
            self.add(c.performance_class.as_ref().map_or(0, |v| v.0.capacity()))?;
        }
        if let Some(c) = &r.content {
            for n in [
                c.contract.content_profile.0.capacity(),
                c.owner_host.0.capacity(),
                c.owner_boot.0.capacity(),
                c.base_id.0.capacity(),
                c.residence_profile.0.capacity(),
            ] {
                self.add(n)?;
            }
        }
        Ok(())
    }
    fn ports(&mut self, ports: &Vec<PortDescriptor>) -> Result<(), NumericPlanStorageRefusal> {
        self.vector(ports)?;
        for p in ports {
            self.add(p.port_id.0.capacity())?;
            self.add(p.value_kind.0.capacity())?;
            self.add(p.abnormal_kind.as_ref().map_or(0, |v| v.0.capacity()))?;
        }
        Ok(())
    }
    fn configuration_value(
        &mut self,
        value: &ConfigurationValue,
    ) -> Result<(), NumericPlanStorageRefusal> {
        match value {
            ConfigurationValue::Text(v) => self.add(v.capacity()),
            ConfigurationValue::Bool(_)
            | ConfigurationValue::U64(_)
            | ConfigurationValue::I64(_) => Ok(()),
            _ => Err(NumericPlanStorageRefusal::Unsupported),
        }
    }
    fn semantic(&mut self, c: &KindSemanticContract) -> Result<(), NumericPlanStorageRefusal> {
        self.vector(&c.configuration)?;
        self.vector(&c.laws)?;
        for field in &c.configuration {
            self.add(field.key.capacity())?;
            self.configuration_value(&field.default_value)?;
            if !matches!(field.rule, KindConfigurationRule::TextBytes { .. }) {
                return Err(NumericPlanStorageRefusal::Unsupported);
            }
        }
        for law in &c.laws {
            match law {
                KindSemanticLaw::ExternalEffects(_)
                | KindSemanticLaw::TemporalState(_)
                | KindSemanticLaw::TimeDependence(_)
                | KindSemanticLaw::RandomDependence(_)
                | KindSemanticLaw::ResourceDependence(_)
                | KindSemanticLaw::Suspension(_)
                | KindSemanticLaw::Variability(_)
                | KindSemanticLaw::Replay(ReplayBehavior::Exact | ReplayBehavior::Ineligible)
                | KindSemanticLaw::Terminal(_) => {}
                KindSemanticLaw::ValueContracts(values) => {
                    self.vector(values)?;
                    for value in values {
                        match &value.location {
                            FrontValueLocation::Input(id) | FrontValueLocation::Output(id) => {
                                self.add(id.0.capacity())?
                            }
                            _ => return Err(NumericPlanStorageRefusal::Unsupported),
                        }
                        self.contract(&value.contract)?;
                    }
                }
                _ => return Err(NumericPlanStorageRefusal::Unsupported),
            }
        }
        Ok(())
    }
}

/// Counts the original complete Rc<Plan>, every allocation and every spare Vec
/// slot in the supported fixed structure. Caller-owned target state is separate.
pub(crate) fn numeric_plan_retained_bytes(plan: &Plan) -> Result<usize, NumericPlanStorageRefusal> {
    use NumericPlanStorageRefusal as R;
    let Plan {
        plan_id,
        source_document_id,
        checked_plot_id,
        expanded_plot_id,
        completion_policy: _,
        realization_backs,
        activations,
        activation_preparations,
        fragments,
    } = plan;
    let mut c = Counter(size_of::<Plan>() + 2 * size_of::<usize>() + 4 * align_of::<Plan>());
    for n in [
        plan_id.0.capacity(),
        source_document_id.0.capacity(),
        checked_plot_id.0.capacity(),
        expanded_plot_id.0.capacity(),
    ] {
        c.add(n)?;
    }
    c.vector(realization_backs)?;
    c.vector(activations)?;
    c.vector(activation_preparations)?;
    c.vector(fragments)?;
    if !realization_backs.is_empty()
        || !activations.is_empty()
        || !activation_preparations.is_empty()
        || fragments.len() != 1
    {
        return Err(R::Unsupported);
    }
    let f = &fragments[0];
    let PlanFragment {
        plan_id: _,
        fragment_id: _,
        source_document_id: _,
        checked_plot_id: _,
        expanded_plot_id: _,
        completion_policy: _,
        realization_backs: _,
        host_id: _,
        boot_id: _,
        offer_generation: _,
        placements: _,
        execution_regions: _,
        execution_fusions: _,
        states: _,
        connections: _,
        fore_ports: _,
        shared_pools: _,
        startup_dependencies: _,
        startup_order: _,
        cancellation_policy: _,
        terminal_policy: _,
        expected_terminals: _,
        expected_sign: _,
        sign_storage_budget: _,
        plan_fragments: _,
    } = f;
    for n in [
        f.plan_id.0.capacity(),
        f.fragment_id.0.capacity(),
        f.source_document_id.0.capacity(),
        f.checked_plot_id.0.capacity(),
        f.expanded_plot_id.0.capacity(),
        f.host_id.0.capacity(),
        f.boot_id.0.capacity(),
    ] {
        c.add(n)?;
    }
    c.vector(&f.realization_backs)?;
    c.vector(&f.execution_fusions)?;
    c.vector(&f.states)?;
    c.vector(&f.shared_pools)?;
    if !f.realization_backs.is_empty()
        || !f.execution_fusions.is_empty()
        || !f.states.is_empty()
        || !f.shared_pools.is_empty()
    {
        return Err(R::Unsupported);
    }
    c.vector(&f.placements)?;
    for g in &f.placements {
        let PlannedGear {
            placement_id: _,
            gear_id: _,
            kind_id: _,
            kind_contract_revision: _,
            source_span: _,
            execution_profile_id: _,
            configuration: _,
            host_id: _,
            boot_id: _,
            offer_generation: _,
            capability_id: _,
            implementation_id: _,
            artifact_id: _,
            base: _,
            realization_characteristics: _,
            realization_properties: _,
            limits: _,
            inputs: _,
            outputs: _,
            semantic_contract: _,
            terminal_transductions: _,
            host_calls: _,
            resources: _,
            authority: _,
            pool_references: _,
        } = g;
        for n in [
            g.placement_id.0.capacity(),
            g.gear_id.0.capacity(),
            g.kind_id.0.capacity(),
            g.kind_contract_revision.0.capacity(),
            g.execution_profile_id.0.capacity(),
            g.host_id.0.capacity(),
            g.boot_id.0.capacity(),
            g.capability_id.0.capacity(),
            g.implementation_id.0.capacity(),
            g.artifact_id.0.capacity(),
        ] {
            c.add(n)?;
        }
        c.vector(&g.configuration)?;
        for v in &g.configuration {
            c.add(v.key.capacity())?;
            c.configuration_value(&v.value)?;
        }
        c.ports(&g.inputs)?;
        c.ports(&g.outputs)?;
        c.semantic(&g.semantic_contract)?;
        c.vector(&g.realization_characteristics)?;
        c.vector(&g.realization_properties)?;
        c.vector(&g.terminal_transductions)?;
        c.vector(&g.authority)?;
        c.vector(&g.pool_references)?;
        if g.base.is_some()
            || !g.realization_characteristics.is_empty()
            || !g.realization_properties.is_empty()
            || !g.terminal_transductions.is_empty()
            || !g.authority.is_empty()
            || !g.pool_references.is_empty()
        {
            return Err(R::Unsupported);
        }
        c.vector(&g.host_calls)?;
        for h in &g.host_calls {
            c.add(h.contract_id.0.capacity())?;
            c.add(h.target_kind.as_ref().map_or(0, |v| v.0.capacity()))?;
        }
        c.vector(&g.resources)?;
        for r in &g.resources {
            c.resource(r)?;
        }
    }
    c.vector(&f.execution_regions)?;
    for r in &f.execution_regions {
        for n in [
            r.region_id.0.capacity(),
            r.execution_profile_id.0.capacity(),
            r.lane_base_id.0.capacity(),
        ] {
            c.add(n)?;
        }
        c.vector(&r.admitted_placements)?;
        for p in &r.admitted_placements {
            c.add(p.0.capacity())?;
        }
        c.resource(&r.lane_resource)?;
    }
    c.vector(&f.connections)?;
    for v in &f.connections {
        for n in [
            v.connection_id.0.capacity(),
            v.source_placement_id.0.capacity(),
            v.source_port_id.0.capacity(),
            v.sink_placement_id.0.capacity(),
            v.sink_port_id.0.capacity(),
            v.value_kind.0.capacity(),
        ] {
            c.add(n)?;
        }
        c.add(v.abnormal_kind.as_ref().map_or(0, |v| v.0.capacity()))?;
        c.vector(&v.admitted_lines)?;
        if v.resource.is_some() || v.selected_line.is_some() || !v.admitted_lines.is_empty() {
            return Err(R::Unsupported);
        }
    }
    c.vector(&f.fore_ports)?;
    for v in &f.fore_ports {
        for n in [
            v.front_port_id.0.capacity(),
            v.placement_id.0.capacity(),
            v.gear_port_id.0.capacity(),
            v.value_kind.0.capacity(),
        ] {
            c.add(n)?;
        }
        c.add(v.abnormal_kind.as_ref().map_or(0, |v| v.0.capacity()))?;
        if v.selected_line.is_some() {
            return Err(R::Unsupported);
        }
        if let Some(contract) = &v.value_contract {
            c.contract(contract)?;
        }
    }
    c.vector(&f.startup_dependencies)?;
    for v in &f.startup_dependencies {
        c.add(v.prerequisite_placement_id.0.capacity())?;
        c.add(v.dependent_placement_id.0.capacity())?;
    }
    c.vector(&f.startup_order)?;
    for p in &f.startup_order {
        c.add(p.0.capacity())?;
    }
    c.vector(&f.expected_terminals)?;
    for v in &f.expected_terminals {
        match v {
            ExpectedTerminal::PlacementCompleted(v) => c.add(v.0.capacity())?,
            ExpectedTerminal::ConnectionCompleted(v) => c.add(v.0.capacity())?,
            ExpectedTerminal::PlanCompleted => {}
        }
    }
    c.vector(&f.expected_sign)?;
    for v in &f.expected_sign {
        match v {
            ExpectedSign::PlacementPrepared(v) | ExpectedSign::PlacementTerminal(v) => {
                c.add(v.0.capacity())?
            }
            ExpectedSign::ConnectionTerminal(v) => c.add(v.0.capacity())?,
            ExpectedSign::PlanFragmentReceived | ExpectedSign::PlanTerminal => {}
        }
    }
    c.vector(&f.plan_fragments)?;
    for v in &f.plan_fragments {
        c.add(v.host_id.0.capacity())?;
        c.add(v.fragment_id.0.capacity())?;
    }
    Ok(c.0)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericPlanPreparationReservation {
    pub(crate) original_plan_retained_bytes: usize,
    pub(crate) temporary_requested_bytes_bound: usize,
}
/// Preflight for the fixed three-Gear verifier, not target preparation. The
/// caller separately reserves the original Source evaluators and model owners.
/// All shape rejection and arithmetic precede the ordinary allocating verifiers.
pub(crate) fn numeric_plan_preparation_reservation(
    plan: &Plan,
    model_retained_bytes: usize,
    source_preparation_bytes: [usize; 2],
    maximum_temporary_requested_bytes: usize,
) -> Result<NumericPlanPreparationReservation, NumericPlanStorageRefusal> {
    use NumericPlanStorageRefusal as R;
    let original = numeric_plan_retained_bytes(plan)?;
    let f = &plan.fragments[0];
    if f.placements.len() != 3
        || f.connections.len() != 2
        || f.fore_ports.len() != 2
        || f.execution_regions.len() > 3
        || f.startup_dependencies.len() > 6
        || f.startup_order.len() != 3
        || f.expected_sign.len() > 16
        || f.expected_terminals.len() > 8
        || f.plan_fragments.len() != 1
        || f.execution_regions
            .iter()
            .any(|r| r.admitted_placements.len() > 3)
        || f.placements
            .iter()
            .any(|g| g.host_calls.len() > 16 || g.resources.len() > 1)
    {
        return Err(R::Unsupported);
    }
    // One fragment fingerprint and one Plan fingerprint, fixed-size temporary
    // sorting/commitment owners, and their geometric canonical Vec requests.
    // Complete Plan allocation sizes cover every string and encoded field;
    // 128x admits repeated canonical fields, <=8x primitive framing expansion
    // and <=4x cumulative Vec growth with independent overlap allowance.
    // The two fixed pure definitions re-encode their complete programs/Types;
    // their already measured structural preparation envelopes are charged64x.
    // Model offer reconstruction is charged independently from the full model
    // owner's retained receipt. The fixed 8MiB covers fixed-layout scaffolding.
    let temporary = original
        .checked_mul(128)
        .and_then(|n| {
            model_retained_bytes
                .checked_mul(64)
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| {
            source_preparation_bytes[0]
                .checked_add(source_preparation_bytes[1])
                .and_then(|m| m.checked_mul(64))
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(8 * 1024 * 1024))
        .ok_or(R::Overflow)?;
    if temporary > maximum_temporary_requested_bytes {
        return Err(R::Pressure);
    }
    Ok(NumericPlanPreparationReservation {
        original_plan_retained_bytes: original,
        temporary_requested_bytes_bound: temporary,
    })
}
