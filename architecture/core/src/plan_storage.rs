//! Allocation-free inventory for supported complete Plan owners.
//! This measures existing owned allocations; it does not bound construction,
//! allocator overhead, or shared Arc/Rc owner headers. Unsupported shapes refuse.
//! Unsupported nested owners refuse; empty Vec spare capacity is still charged.
use crate::*;
use alloc::vec::Vec;
use core::mem::size_of;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanStorageRefusal {
    Overflow,
    Unsupported,
}
struct Counter(usize);
impl Counter {
    fn add(&mut self, n: usize) -> Result<(), PlanStorageRefusal> {
        self.0 = self.0.checked_add(n).ok_or(PlanStorageRefusal::Overflow)?;
        Ok(())
    }
    fn vector<T>(&mut self, v: &Vec<T>) -> Result<(), PlanStorageRefusal> {
        self.add(
            v.capacity()
                .checked_mul(size_of::<T>())
                .ok_or(PlanStorageRefusal::Overflow)?,
        )
    }
    fn contract(&mut self, c: &CheckedValueContract) -> Result<(), PlanStorageRefusal> {
        let storage = c.clone_storage_reservation().map_err(|_| PlanStorageRefusal::Overflow)?;
        self.add(storage.retained_heap_bytes_bound)
    }

    fn resource(&mut self, r: &ResourceBinding) -> Result<(), PlanStorageRefusal> {
        self.add(r.pool_id.0.capacity())?;
        self.add(r.class_id.0.capacity())?;
        if r.protected.is_some() {
            return Err(PlanStorageRefusal::Unsupported);
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
    fn terminal(&mut self, v: &TerminalTransductionProfile) -> Result<(), PlanStorageRefusal> {
        self.add(v.input_port_id.owned_heap_bytes())?;
        self.add(v.output_port_id.owned_heap_bytes())?;
        if let NormalCloseTransduction::DomainSpecific { law } = &v.normal_close {
            self.add(law.owned_heap_bytes())?;
        }
        if let AbnormalTerminalTransduction::DomainSpecific { law } = &v.abnormal {
            self.add(law.owned_heap_bytes())?;
        }
        match &v.cancellation {
            CancellationTransduction::NotCancellable => {}
            CancellationTransduction::Request { disposition_kind } => {
                self.add(disposition_kind.owned_heap_bytes())?
            }
            CancellationTransduction::DomainSpecific { law } => self.add(law.owned_heap_bytes())?,
        }
        Ok(())
    }
    fn ports(&mut self, ports: &Vec<PortDescriptor>) -> Result<(), PlanStorageRefusal> {
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
    ) -> Result<(), PlanStorageRefusal> {
        match value {
            ConfigurationValue::Text(v) => self.add(v.capacity()),
            ConfigurationValue::Bool(_)
            | ConfigurationValue::U64(_)
            | ConfigurationValue::I64(_) => Ok(()),
            ConfigurationValue::Quantity(_) => Ok(()),
            ConfigurationValue::Structured(v) => {
                self.add(v.profile().owned_heap_bytes())?;
                self.add(v.canonical_storage_capacity())
            }
        }
    }
    fn semantic(&mut self, c: &KindSemanticContract) -> Result<(), PlanStorageRefusal> {
        self.semantic_fields(&c.configuration, &c.laws)
    }
    fn semantic_fields(
        &mut self,
        configuration: &Vec<KindConfigurationField>,
        laws: &Vec<KindSemanticLaw>,
    ) -> Result<(), PlanStorageRefusal> {
        self.vector(configuration)?;
        self.vector(laws)?;
        for field in configuration {
            self.add(field.key.capacity())?;
            self.configuration_value(&field.default_value)?;
            match &field.rule {
                KindConfigurationRule::TextOneOf { values } => {
                    self.vector(values)?;
                    for value in values {
                        self.add(value.capacity())?;
                    }
                }
                KindConfigurationRule::Structured { profile } => {
                    self.add(profile.owned_heap_bytes())?
                }
                KindConfigurationRule::Any
                | KindConfigurationRule::U64Range { .. }
                | KindConfigurationRule::I64Range { .. }
                | KindConfigurationRule::DurationMillis { .. }
                | KindConfigurationRule::QuantityRange { .. }
                | KindConfigurationRule::TextBytes { .. } => {}
            }
        }
        for law in laws {
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
                KindSemanticLaw::TerminalTransduction(v) => self.terminal(v)?,
                KindSemanticLaw::ValueContracts(values) => {
                    self.vector(values)?;
                    for value in values {
                        match &value.location {
                            FrontValueLocation::Input(id) | FrontValueLocation::Output(id) => {
                                self.add(id.0.capacity())?
                            }
                            _ => return Err(PlanStorageRefusal::Unsupported),
                        }
                        self.contract(&value.contract)?;
                    }
                }
                _ => return Err(PlanStorageRefusal::Unsupported),
            }
        }
        Ok(())
    }
}

/// Counts every owned allocation and spare Vec slot for a supported complete
/// single-fragment Plan. Inline Plan storage and outer shared ownership are
/// separate. No allocation is performed; unsupported nested owners refuse.
pub fn plan_owned_heap_bytes(plan: &Plan) -> Result<usize, PlanStorageRefusal> {
    use PlanStorageRefusal as R;
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
    let mut c = Counter(0);
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
        for terminal in &g.terminal_transductions {
            c.terminal(terminal)?;
        }
        c.vector(&g.authority)?;
        c.vector(&g.pool_references)?;
        if g.base.is_some()
            || !g.realization_characteristics.is_empty()
            || !g.realization_properties.is_empty()
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

impl Counter {
    fn startup(
        &mut self,
        values: &Vec<FrontStartupParameter>,
        shorthand: &Option<(PortId, PortId)>,
    ) -> Result<(), PlanStorageRefusal> {
        self.vector(values)?;
        for value in values {
            self.add(value.name.capacity())?;
            self.add(value.value_type.owned_heap_bytes())?;
        }
        if let Some((input, output)) = shorthand {
            self.add(input.owned_heap_bytes())?;
            self.add(output.owned_heap_bytes())?;
        }
        Ok(())
    }
}

/// Actual owned allocations of a supported complete Kind. Inline storage and
/// any outer Arc/Rc are separate. Unsupported semantic owners refuse.
pub fn kind_owned_heap_bytes(kind: &Kind) -> Result<usize, PlanStorageRefusal> {
    let Kind {
        startup_parameters,
        shorthand,
        kind_id,
        kind_contract_revision,
        inputs,
        outputs,
        configuration,
        semantic_laws,
        limits: _,
    } = kind;
    let mut c = Counter(0);
    c.startup(startup_parameters, shorthand)?;
    c.add(kind_id.owned_heap_bytes())?;
    c.add(kind_contract_revision.owned_heap_bytes())?;
    c.ports(inputs)?;
    c.ports(outputs)?;
    c.semantic_fields(configuration, semantic_laws)?;
    Ok(c.0)
}

/// Actual owned allocations of a supported complete CapabilityOffer. Inline
/// storage and outer shared ownership are separate; this is not a preparation
/// allocation bound or admission authority.
pub fn capability_offer_owned_heap_bytes(
    offer: &CapabilityOffer,
) -> Result<usize, PlanStorageRefusal> {
    let CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id,
        kind_id,
        kind_contract_revision,
        inputs,
        outputs,
        semantic_contract,
        implementation,
        state_retention: _,
        realization_properties,
        host_calls,
        resource_requirements,
        authority_requirements,
        limits: _,
    } = offer;
    let mut c = Counter(0);
    c.startup(startup_parameters, shorthand)?;
    for n in [
        capability_id.owned_heap_bytes(),
        kind_id.owned_heap_bytes(),
        kind_contract_revision.owned_heap_bytes(),
        implementation.execution_profile_id.owned_heap_bytes(),
        implementation.implementation_id.owned_heap_bytes(),
        implementation.artifact_id.owned_heap_bytes(),
    ] {
        c.add(n)?;
    }
    c.ports(inputs)?;
    c.ports(outputs)?;
    c.semantic(semantic_contract)?;
    c.vector(realization_properties)?;
    for value in realization_properties {
        c.add(value.profile().owned_heap_bytes())?;
        c.add(value.canonical_storage_capacity())?;
    }
    c.vector(host_calls)?;
    for call in host_calls {
        c.add(call.contract_id.owned_heap_bytes())?;
        if let Some(kind) = &call.target_kind {
            c.add(kind.owned_heap_bytes())?;
        }
    }
    c.vector(resource_requirements)?;
    for resource in resource_requirements {
        c.add(resource.class_id.owned_heap_bytes())?;
        if let Some(role) = &resource.protected_role {
            c.add(role.owned_heap_bytes())?;
        }
        if let Some(compute) = &resource.compute {
            if let Some(topology) = &compute.topology {
                if let Some(class) = &topology.performance_class {
                    c.add(class.owned_heap_bytes())?;
                }
            }
        }
        if let Some(content) = &resource.content {
            c.add(content.content_profile.owned_heap_bytes())?;
        }
    }
    c.vector(authority_requirements)?;
    for authority in authority_requirements {
        c.add(authority.contract_id.owned_heap_bytes())?;
        c.add(authority.host_call_contract_id.owned_heap_bytes())?;
        c.add(authority.subject_kind.owned_heap_bytes())?;
    }
    Ok(c.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};

    #[test]
    fn kind_inventory_counts_empty_vector_slack_and_text_capacity() {
        let mut name = String::with_capacity(91);
        name.push_str("a");
        let name_capacity = name.capacity();
        let mut choices = Vec::with_capacity(7);
        choices.push(name);
        let choice_capacity = choices.capacity();
        let mut configuration = Vec::with_capacity(3);
        configuration.push(KindConfigurationField {
            key: String::new(),
            default_value: ConfigurationValue::Bool(false),
            rule: KindConfigurationRule::TextOneOf { values: choices },
        });
        let kind = Kind {
            startup_parameters: Vec::with_capacity(5),
            shorthand: None,
            kind_id: KindId(String::new()),
            kind_contract_revision: KindIdentity(String::new()),
            inputs: Vec::with_capacity(11),
            outputs: Vec::new(),
            configuration,
            semantic_laws: Vec::with_capacity(13),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        };
        let expected = kind.startup_parameters.capacity() * size_of::<FrontStartupParameter>()
            + kind.inputs.capacity() * size_of::<PortDescriptor>()
            + kind.configuration.capacity() * size_of::<KindConfigurationField>()
            + kind.semantic_laws.capacity() * size_of::<KindSemanticLaw>()
            + choice_capacity * size_of::<String>()
            + name_capacity;
        assert_eq!(kind_owned_heap_bytes(&kind), Ok(expected));
    }

    #[test]
    fn unsupported_semantic_owner_refuses_and_overflow_is_checked() {
        let semantic = KindSemanticContract {
            configuration: vec![],
            laws: vec![KindSemanticLaw::ResourcePorts(vec![])],
        };
        assert_eq!(
            Counter(0).semantic(&semantic),
            Err(PlanStorageRefusal::Unsupported)
        );
        assert_eq!(
            Counter(usize::MAX).add(1),
            Err(PlanStorageRefusal::Overflow)
        );
    }
}

/// Requested allocations of one derived clone of a supported owner. The source
/// remains live; its storage and inline/Arc owner storage are separately charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerCloneStorageReservation {
    pub retained_heap_bytes_bound: usize,
    pub preparation_requested_bytes_bound: usize,
}

/// Reserves all allocations of cloning a supported complete Kind before cloning.
/// Derived Vec/String clones request at most each source allocation's capacity.
pub fn kind_clone_storage_reservation(
    kind: &Kind,
) -> Result<OwnerCloneStorageReservation, PlanStorageRefusal> {
    let bytes = kind_owned_heap_bytes(kind)?;
    Ok(OwnerCloneStorageReservation {
        retained_heap_bytes_bound: bytes,
        preparation_requested_bytes_bound: bytes,
    })
}

/// Reserves one derived CapabilityOffer clone, excluding inline/shared headers.
pub fn capability_offer_clone_storage_reservation(
    offer: &CapabilityOffer,
) -> Result<OwnerCloneStorageReservation, PlanStorageRefusal> {
    let bytes = capability_offer_owned_heap_bytes(offer)?;
    Ok(OwnerCloneStorageReservation {
        retained_heap_bytes_bound: bytes,
        preparation_requested_bytes_bound: bytes,
    })
}
