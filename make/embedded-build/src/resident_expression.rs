//! Immutable preparation before planning, followed by exact selected-Plan binding.
use conduit_core as c;
use conduit_plot::{
    rust_binding::PreparedNativeBounds, CheckedSyntaxDocument, PortableExpressionProgram,
};
use sha2::{Digest, Sha256};
pub const RESIDENT_EXPRESSION_PROFILE: &str = "conduit.expression/resident-structured@1";
pub const RESIDENT_EXPRESSION_IMPLEMENTATION: &str =
    "conduit.expression/kernel-resident-structured@1";
#[derive(Clone, Copy)]
pub struct ResidentExpressionBounds {
    pub native: PreparedNativeBounds,
    pub maximum_occurrences: usize,
    pub maximum_program_bytes: u32,
    pub maximum_generated_bytes: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidentExpressionRefusal {
    Source,
    Program,
    Generation(conduit_plot::StaticStructuredGenerationRefusal),
    Bounds,
    Plan,
    Selection,
    Contract,
    Resource,
}
/// Prepared code is eligibility, never a Ready offer or execution grant.
pub struct PreparedResidentExpression {
    source_document: c::SourceDocumentId,
    maximum_input: u32,
    maximum_generated_bytes: u32,
    text: String,
    temporal: c::PortTemporal,
    code: String,
    artifact: c::ArtifactId,
    kind: c::KindId,
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
        use ResidentExpressionRefusal as R;
        if !matches!(
            temporal,
            c::PortTemporal::Value | c::PortTemporal::Flow { closes: true }
        ) {
            return Err(R::Contract);
        }
        let binary = program.canonical_bytes().map_err(|_| R::Program)?;
        if binary.len() as u64 > u64::from(bounds.maximum_program_bytes) {
            return Err(R::Bounds);
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
        code.push_str(&format!("#[derive(Debug)] pub enum PreparationRefusal {{Image(conduit_plot::StaticStructuredRefusal),Back(conduit_semantic_catalog::StaticExpressionBackRefusal)}}\npub type Back=conduit_semantic_catalog::StaticStructuredExpressionBack<{{input_native::CONTRACT_COUNT}},{{output_native::CONTRACT_COUNT}},{{CHILD_COUNT}},{output}>;\npub fn prepare_back()->Result<Back,PreparationRefusal>{{let image=prepare().map_err(PreparationRefusal::Image)?; Back::from_resident_structured(image,{temporal_rust}).map_err(PreparationRefusal::Back)}}\n"));
        if code.len() as u64 > u64::from(bounds.maximum_generated_bytes) {
            return Err(R::Bounds);
        }
        let digest = Sha256::digest(code.as_bytes());
        let artifact = format!("conduit.expression/resident-source@1/{digest:x}").into();
        Ok(Self {
            source_document: document.source_document_id.clone(),
            maximum_input,
            maximum_generated_bytes: bounds.maximum_generated_bytes,
            text: program.canonical_hex().map_err(|_| R::Program)?,
            temporal,
            code,
            artifact,
            kind: definition.kind_id,
        })
    }
    /// Identity of the emitted realization source, not the linked Seed's content
    /// hash, proof of flash placement, or authority to install it.
    pub fn artifact_id(&self) -> &c::ArtifactId {
        &self.artifact
    }
    pub fn kind_id(&self) -> &c::KindId {
        &self.kind
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
        if !gear.resources.iter().any(|r| {
            r.class_id.as_str() == c::COOPERATIVE_STEP_WORK_CLASS_ID
                && r.units > 0
                && r.protected.is_none()
                && r.compute.is_none()
                && r.content.is_none()
        }) {
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
