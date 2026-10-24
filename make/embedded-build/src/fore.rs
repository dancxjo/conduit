//! Trusted Make projection of a whole checked Plan with local external Fore.
use crate::*;
use conduit_core as c;
use conduit_plan_lowering::lowering::{lower_plan_fragment_from_plan, LoweredForePort};
/// The typed lowering remains available to the caller for value admission.
/// Neither numeric tables nor ABI pins grant a Fore caller possession.
pub struct GeneratedForePlan {
    kernel: GeneratedEmbeddedPlan,
    fore: Vec<c::AssignedForeRequirement>,
}
impl GeneratedForePlan {
    pub fn kernel(&self) -> &GeneratedEmbeddedPlan {
        &self.kernel
    }
    pub fn fore(&self) -> &[c::AssignedForeRequirement] {
        &self.fore
    }
    pub fn encode(&self, maxima: c::AssignedPlanMaxima) -> Result<Vec<u8>, GenerationError> {
        crate::assigned_plan::encode_fore(&self.kernel, maxima, &self.fore)
    }
    pub fn layout_requirements(
        &self,
        maxima: c::AssignedPlanMaxima,
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        crate::assigned_plan::layout_records(
            self.encode(maxima)?,
            c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
        )
    }
}
/// This opt-in local profile independently lowers the complete sealed Plan.
/// It refuses network Lines and fused activations, rather than relabeling them.
pub fn generate_fore_native_plan(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    bounds: EmbeddedImageBounds,
    computations: EmbeddedComputationBounds,
    maximum_fore_ports: usize,
) -> Result<GeneratedForePlan, GenerationError> {
    if !c::verify_plan(plan) {
        return Err(GenerationError::InvalidFragment);
    }
    let fragment = plan
        .fragments
        .iter()
        .find(|f| &f.fragment_id == fragment)
        .ok_or(GenerationError::InvalidFragment)?;
    let (lowered, activations) = lower_plan_fragment_from_plan(plan, &fragment.fragment_id)
        .map_err(|_| GenerationError::InvalidFragment)?;
    if !activations.entries.is_empty()
        || !lowered.remote_endpoints.is_empty()
        || lowered.fore_ports.iter().any(|p| p.selected_line.is_some())
    {
        return Err(GenerationError::Unsupported(
            UnsupportedPlanFeature::LocalFore,
        ));
    }
    if maximum_fore_ports > 16 {
        return Err(GenerationError::BoundExceeded {
            table: "local Fore profile capacity",
            actual: maximum_fore_ports as u64,
            maximum: 16,
        });
    }
    if lowered.fore_ports.len() > maximum_fore_ports {
        return Err(GenerationError::BoundExceeded {
            table: "local Fore ports",
            actual: lowered.fore_ports.len() as u64,
            maximum: maximum_fore_ports as u64,
        });
    }
    let kernel =
        crate::generate::generate_profile(fragment, &lowered, bounds, Some(computations), true)?;
    let fore = lowered.fore_ports.iter().map(requirement).collect();
    Ok(GeneratedForePlan { kernel, fore })
}
fn requirement(port: &LoweredForePort) -> c::AssignedForeRequirement {
    let mut abi = Vec::from(&b"conduit.assigned/local-fore-abi@1\0"[..]);
    let mut bytes = |value: &[u8]| {
        abi.extend_from_slice(&(value.len() as u64).to_le_bytes());
        abi.extend_from_slice(value);
    };
    bytes(port.value_kind.as_str().as_bytes());
    bytes(
        &port
            .value_contract
            .as_ref()
            .map(|c| c.identity_bytes())
            .unwrap_or_default(),
    );
    bytes(
        port.abnormal_kind
            .as_ref()
            .map_or(&[][..], |k| k.as_str().as_bytes()),
    );
    bytes(port.temporal.as_str().as_bytes());
    abi.extend_from_slice(&[
        match port.track {
            c::ConnectionTrack::Payload => 0,
            c::ConnectionTrack::NormalClose => 1,
            c::ConnectionTrack::AbnormalTerminal => 2,
            c::ConnectionTrack::Quiescence => 3,
        },
        match port.quantity_profile {
            c::QuantityValueProfile::Decimal => 0,
            c::QuantityValueProfile::Exact => 1,
        },
    ]);
    abi.extend_from_slice(&port.item_capacity.to_le_bytes());
    abi.extend_from_slice(&port.byte_capacity.to_le_bytes());
    c::AssignedForeRequirement::new(
        port.endpoint.0,
        port.cord.0,
        match port.direction {
            c::PortDirection::Input => c::AssignedForeDirection::Input,
            c::PortDirection::Output => c::AssignedForeDirection::Output,
        },
        c::AssignedIdentity::from_text(port.front_port_id.as_str()),
        c::assigned_plan_payload_digest(&abi),
    )
}
