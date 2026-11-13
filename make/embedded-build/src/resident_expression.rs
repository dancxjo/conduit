//! Immutable preparation before planning, followed by exact selected-Plan binding.
use conduit_core as c;
use conduit_plot::{
    rust_binding::PreparedNativeBounds, CheckedSyntaxDocument, PortableExpressionProgram,
};
use sha2::{Digest, Sha256};

#[cfg(test)]
#[path = "resident_offer_image_tests.rs"]
mod offer_image_tests;
pub const RESIDENT_EXPRESSION_PROFILE: &str = "conduit.expression/resident-structured@1";
pub const RESIDENT_EXPRESSION_IMPLEMENTATION: &str =
    "conduit.expression/kernel-resident-structured@1";
/// Physical execution storage selected by fabrication, never execution authority.
/// SharedWorkspace requires the target's `execution-workspace` feature and a
/// separately funded scheduler workspace before any Step may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidentExecutionStorage {
    Owned,
    SharedWorkspace,
}
#[derive(Clone, Copy)]
pub struct ResidentExpressionBounds {
    pub native: PreparedNativeBounds,
    pub maximum_occurrences: usize,
    pub maximum_program_bytes: u32,
    pub maximum_generated_bytes: u32,
    /// Declared conservative private work requirement, independently reserved by planning.
    pub private_work_units: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidentExpressionRefusal {
    Source,
    GenerationMetadata,
    Program,
    Generation(conduit_plot::StaticStructuredGenerationRefusal),
    Bounds,
    ProgramBytes { required: usize, maximum: u32 },
    GeneratedBytes { required: usize, maximum: u32 },
    Plan,
    Selection,
    Contract,
    Resource,
}
/// Prepared code is eligibility, never a Ready offer or execution grant.
pub struct PreparedResidentExpression {
    storage: ResidentExecutionStorage,
    source_document: c::SourceDocumentId,
    maximum_input: u32,
    maximum_output: u32,
    maximum_generated_bytes: u32,
    text: String,
    temporal: c::PortTemporal,
    code: String,
    artifact: c::ArtifactId,
    kind: c::KindId,
    offer: c::CapabilityOffer,
}
/// This private construction path correlates emitted code with one exact sealed
/// selection. Host/Boot authority and initialization remain independent gates.
pub struct BoundResidentExpression {
    pub(crate) plan: c::PlanId,
    pub(crate) fragment: c::FragmentId,
    pub(crate) node: u16,
    pub(crate) kind: c::KindId,
    pub(crate) artifact: c::ArtifactId,
    pub(crate) pin: c::AssignedComputationRequirement,
    pub(crate) step_fuel: u16,
    code: String,
}
impl PreparedResidentExpression {
    pub fn prepare(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_instance(document, program, temporal, bounds, None)
    }
    /// Prepare an explicitly distinct initialized owner for shared checked code.
    /// The instance label describes cold construction; it grants no custody.
    pub fn prepare_instance(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
        instance: Option<&str>,
    ) -> Result<Self, ResidentExpressionRefusal> {
        Self::prepare_instance_with_storage(
            document,
            program,
            temporal,
            bounds,
            instance,
            ResidentExecutionStorage::Owned,
        )
    }
    /// Select explicit physical storage before sealing the emitted artifact.
    pub fn prepare_instance_with_storage(
        document: &CheckedSyntaxDocument,
        program: &PortableExpressionProgram,
        temporal: c::PortTemporal,
        bounds: ResidentExpressionBounds,
        instance: Option<&str>,
        storage: ResidentExecutionStorage,
    ) -> Result<Self, ResidentExpressionRefusal> {
        if instance.is_some_and(|name| name.is_empty() || name.len() > 1024) {
            return Err(ResidentExpressionRefusal::Bounds);
        }
        use ResidentExpressionRefusal as R;
        if !matches!(
            temporal,
            c::PortTemporal::Value | c::PortTemporal::Flow { closes: true }
        ) {
            return Err(R::Contract);
        }
        if bounds.private_work_units == 0 || bounds.private_work_units > u16::MAX - 3 {
            return Err(R::Bounds);
        }
        let binary = program.canonical_bytes().map_err(|_| R::Program)?;
        if binary.len() as u64 > u64::from(bounds.maximum_program_bytes) {
            return Err(R::ProgramBytes {
                required: binary.len(),
                maximum: bounds.maximum_program_bytes,
            });
        }
        let definition = conduit_plot::portable_expression_definition(program, temporal)
            .map_err(|_| R::Contract)?;
        let mut code = conduit_plot::generate_static_structured_expression(
            document,
            program,
            bounds.native,
            bounds.maximum_occurrences,
        )
        .map_err(R::Generation)?;
        let maximum_input = program
            .maximum_prepared_input_bytes()
            .map_err(|_| R::Bounds)?;
        let temporal_rust = match temporal {
            c::PortTemporal::Value => "conduit_core::PortTemporal::Value",
            _ => "conduit_core::PortTemporal::Flow {closes:true}",
        };
        let output = program
            .maximum_prepared_output_bytes()
            .map_err(|_| R::Bounds)?;
        crate::resident_back::append(&mut code, output, temporal_rust, storage);
        code.push_str(&format!(
            "pub const PRIVATE_WORK_UNITS:u16={};\n",
            bounds.private_work_units
        ));
        if code.len() as u64 > u64::from(bounds.maximum_generated_bytes) {
            return Err(R::GeneratedBytes {
                required: code.len(),
                maximum: bounds.maximum_generated_bytes,
            });
        }
        if let Some(instance) = instance {
            let digest: [u8; 32] = Sha256::digest(instance.as_bytes()).into();
            // The instance marker belongs to the generated realization, not the
            // pure program's Kind. Assigned-node artifact pins distinguish the
            // initialized owners even though they execute identical code.
            code.push_str(&format!(
                "pub const INITIALIZED_INSTANCE:[u8;32]={digest:?};\n"
            ));
        }
        let digest = Sha256::digest(code.as_bytes());
        let artifact = format!("conduit.expression/resident-source@1/{digest:x}").into();
        let offer = crate::resident_offer::append(
            &mut code,
            program,
            temporal,
            &artifact,
            bounds.private_work_units,
            instance,
        )?;
        if code.len() as u64 > u64::from(bounds.maximum_generated_bytes) {
            return Err(R::GeneratedBytes {
                required: code.len(),
                maximum: bounds.maximum_generated_bytes,
            });
        }
        Ok(Self {
            storage,
            source_document: document.source_document_id.clone(),
            maximum_input,
            maximum_output: output,
            maximum_generated_bytes: bounds.maximum_generated_bytes,
            text: program.canonical_hex().map_err(|_| R::Program)?,
            temporal,
            code,
            artifact,
            kind: definition.kind_id,
            offer,
        })
    }
    /// Cold physical storage selection, not an allocation or resource grant.
    pub fn execution_storage(&self) -> ResidentExecutionStorage {
        self.storage
    }
    /// Identity of the emitted realization source, not the linked Seed's content
    /// hash, proof of flash placement, or authority to install it.
    pub fn artifact_id(&self) -> &c::ArtifactId {
        &self.artifact
    }
    pub fn kind_id(&self) -> &c::KindId {
        &self.kind
    }
    /// Immutable preparation metadata, not evidence of an initialized, Ready or
    /// authorized runtime provider. Current publication is independently checked.
    pub fn offer_description(&self) -> &c::CapabilityOffer {
        &self.offer
    }
    pub fn source(&self) -> &str {
        &self.code
    }
    pub fn bind(
        &self,
        plan: &c::Plan,
        fragment: &c::FragmentId,
        placement: &c::PlacementId,
    ) -> Result<BoundResidentExpression, ResidentExpressionRefusal> {
        use ResidentExpressionRefusal as R;
        if !c::verify_plan(plan) {
            return Err(R::Plan);
        }
        if plan.source_document_id != self.source_document {
            return Err(R::Source);
        }
        let fragment = plan
            .fragments
            .iter()
            .find(|f| &f.fragment_id == fragment)
            .ok_or(R::Plan)?;
        let gear = fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == placement)
            .ok_or(R::Selection)?;
        if gear.kind_id != self.kind
            || gear.implementation_id.as_str() != RESIDENT_EXPRESSION_IMPLEMENTATION
            || gear.execution_profile_id.as_str() != RESIDENT_EXPRESSION_PROFILE
            || gear.artifact_id != self.artifact
            || gear.capability_id != self.offer.capability_id
            || gear.limits != self.offer.limits
        {
            return Err(R::Selection);
        }
        let [entry] = gear.configuration.as_slice() else {
            return Err(R::Contract);
        };
        if entry.key != "program"
            || entry.value != c::ConfigurationValue::Text(self.text.clone())
            || gear
                .inputs
                .first()
                .is_none_or(|p| p.temporal != self.temporal)
        {
            return Err(R::Contract);
        }
        crate::configuration::validate_expression(gear, "program", &self.text)
            .map_err(|_| R::Contract)?;
        if !gear.host_calls.is_empty()
            || gear.base.is_some()
            || !gear.authority.is_empty()
            || !gear.terminal_transductions.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.realization_properties.is_empty()
            || !gear.realization_characteristics.is_empty()
        {
            return Err(R::Selection);
        }
        let [resource] = gear.resources.as_slice() else {
            return Err(R::Resource);
        };
        let expected = &self.offer.resource_requirements[0];
        if resource.class_id != expected.class_id
            || resource.units != expected.units
            || resource.protected.is_some()
            || resource.compute.is_some()
            || resource.content.is_some()
        {
            return Err(R::Resource);
        }
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            plan,
            &fragment.fragment_id,
        )
        .map_err(|_| R::Plan)?;
        let node = lowered
            .nodes
            .iter()
            .find(|n| &n.placement_id == placement)
            .ok_or(R::Plan)?;
        let pin = c::AssignedComputationRequirement::from_text(node.node.0, "program", &self.text)
            .map_err(|_| R::Bounds)?;
        let mut code = self.code.clone();
        code.push_str(&format!("pub const SELECTED_NODE:u16={};\npub const SELECTED_STEP_FUEL:u16={};\npub fn prepare_selected_back()->Result<Back,PreparationRefusal>{{let back=prepare_back()?;if back.required_step_fuel()>SELECTED_STEP_FUEL {{return Err(PreparationRefusal::Back(conduit_semantic_catalog::StaticExpressionBackRefusal::Work));}}Ok(back)}}\n",node.node.0,node.maximum_step_fuel));
        if code.len() as u64 > u64::from(self.maximum_generated_bytes) {
            return Err(R::Bounds);
        }
        Ok(BoundResidentExpression {
            plan: plan.plan_id.clone(),
            fragment: fragment.fragment_id.clone(),
            node: node.node.0,
            kind: self.kind.clone(),
            artifact: self.artifact.clone(),
            pin,
            step_fuel: node.maximum_step_fuel,
            code,
        })
    }
    /// Canonical envelope of this prepared program, not a resource grant.
    pub fn maximum_output_bytes(&self) -> u32 {
        self.maximum_output
    }
    pub fn maximum_input_bytes(&self) -> u32 {
        self.maximum_input
    }
}
impl BoundResidentExpression {
    pub fn source(&self) -> &str {
        &self.code
    }
    pub fn computation_requirement(&self) -> c::AssignedComputationRequirement {
        self.pin
    }
}

/// Bind the exact resident images selected by this sealed fragment against an
/// independently current offer observation. This neither initializes a Back
/// nor proves resource possession. The receiving Root must admit it separately.
pub fn bind_current_resident_expressions(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    actual: &c::HostAdvertisement,
    images: &[&PreparedResidentExpression],
) -> Result<Vec<BoundResidentExpression>, ResidentExpressionRefusal> {
    bind_current_mixed_resident_expressions(plan, fragment, actual, images, &[])
}

/// Bind resident images while accounting for independently selected native offers.
/// Native metadata is descriptive: this grants no provider possession, resource
/// reservation, Host Call authority, or permission to Start. Every supplied native
/// offer must identify a selected capability and equal its current offer.
pub fn bind_current_mixed_resident_expressions(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    actual: &c::HostAdvertisement,
    images: &[&PreparedResidentExpression],
    native: &[&c::CapabilityOffer],
) -> Result<Vec<BoundResidentExpression>, ResidentExpressionRefusal> {
    use ResidentExpressionRefusal as R;
    if !c::verify_plan(plan) {
        return Err(R::Plan);
    }
    let selected = plan
        .fragments
        .iter()
        .find(|f| &f.fragment_id == fragment)
        .ok_or(R::Plan)?;
    if selected.host_id != actual.host_id
        || selected.boot_id != actual.boot_id
        || selected.offer_generation != actual.offer_generation
    {
        return Err(R::Selection);
    }
    // One explicit selection per native capability; duplicate IDs are ambiguous
    // even when the supplied offers happen to be byte-for-byte equal.
    for (index, offer) in native.iter().enumerate() {
        if native[..index]
            .iter()
            .any(|prior| prior.capability_id == offer.capability_id)
            || !selected
                .placements
                .iter()
                .any(|p| p.capability_id == offer.capability_id)
        {
            return Err(R::Selection);
        }
    }
    let mut bound = Vec::new();
    for placement in &selected.placements {
        let mut offers = actual
            .capabilities
            .iter()
            .filter(|offer| offer.capability_id == placement.capability_id);
        let offer = offers.next().ok_or(R::Selection)?;
        if offers.next().is_some() {
            return Err(R::Selection);
        }
        if placement.host_id != actual.host_id
            || placement.boot_id != actual.boot_id
            || placement.offer_generation != actual.offer_generation
        {
            return Err(R::Selection);
        }
        let mut candidates = images.iter().filter(|image| image.offer == *offer);
        let image = candidates.next();
        let native_offer = native
            .iter()
            .find(|n| n.capability_id == offer.capability_id);
        if candidates.next().is_some() || (image.is_some() && native_offer.is_some()) {
            return Err(R::Selection);
        }
        if let Some(image) = image {
            bound.push(image.bind(plan, fragment, &placement.placement_id)?);
        } else if native_offer.is_none_or(|selected| **selected != *offer)
            || placement.kind_id != offer.kind_id
            || placement.kind_contract_revision != offer.kind_contract_revision
            || placement.execution_profile_id != offer.implementation.execution_profile_id
            || placement.implementation_id != offer.implementation.implementation_id
            || placement.artifact_id != offer.implementation.artifact_id
            || placement.limits != offer.limits
            || placement.inputs != offer.inputs
            || placement.outputs != offer.outputs
            || placement.semantic_contract != offer.semantic_contract
            || placement.host_calls != offer.host_calls
            || !native_bindings_correspond(placement, offer)
        {
            return Err(R::Selection);
        }
    }
    Ok(bound)
}

// Descriptive correspondence only. Current grants, opaque provider possession,
// aggregate pool capacity and actual reservation remain receiving Root gates.
pub(crate) fn native_bindings_correspond(
    placement: &c::PlannedGear,
    offer: &c::CapabilityOffer,
) -> bool {
    if placement.resources.len() != offer.resource_requirements.len()
        || placement.authority.len() != offer.authority_requirements.len()
    {
        return false;
    }
    for requirement in &offer.resource_requirements {
        if requirement.units == 0
            || requirement.protected_role.is_some()
            || requirement.compute.is_some()
            || requirement.content.is_some()
            || offer
                .resource_requirements
                .iter()
                .filter(|r| r.class_id == requirement.class_id)
                .count()
                != 1
        {
            return false;
        }
        let mut bindings = placement
            .resources
            .iter()
            .filter(|r| r.class_id == requirement.class_id);
        let Some(binding) = bindings.next() else {
            return false;
        };
        if bindings.next().is_some()
            || binding.pool_id.as_str().is_empty()
            || binding.units != requirement.units
            || binding.protected.is_some()
            || binding.compute.is_some()
            || binding.content.is_some()
        {
            return false;
        }
    }
    for requirement in &offer.authority_requirements {
        if offer
            .authority_requirements
            .iter()
            .filter(|r| *r == requirement)
            .count()
            != 1
        {
            return false;
        }
        let mut bindings = placement.authority.iter().filter(|a| {
            a.contract_id == requirement.contract_id
                && a.host_call_contract_id == requirement.host_call_contract_id
                && a.subject_kind == requirement.subject_kind
        });
        let Some(binding) = bindings.next() else {
            return false;
        };
        if bindings.next().is_some()
            || binding.grant_id.as_str().is_empty()
            || binding.host_id != placement.host_id
            || binding.boot_id != placement.boot_id
            || binding.capability_id != placement.capability_id
        {
            return false;
        }
    }
    true
}
