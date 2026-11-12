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
    pub fn encode_with_resident_computations(
        &self,
        maxima: c::AssignedPlanMaxima,
        resident: &[BoundResidentExpression],
    ) -> Result<Vec<u8>, GenerationError> {
        crate::assigned_plan::encode_resident_fore(&self.kernel, maxima, &self.fore, resident)
    }
    pub fn layout_requirements_with_resident_computations(
        &self,
        maxima: c::AssignedPlanMaxima,
        resident: &[BoundResidentExpression],
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        crate::assigned_layout::layout_records(
            self.encode_with_resident_computations(maxima, resident)?,
            c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
        )
    }
    pub fn layout_requirements(
        &self,
        maxima: c::AssignedPlanMaxima,
    ) -> Result<Vec<c::AssignedRecordRequirement>, GenerationError> {
        crate::assigned_layout::layout_records(
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
    let (kernel, ports) = prepare_fore(
        plan,
        fragment,
        bounds,
        computations,
        maximum_fore_ports,
        false,
    )?;
    let fore = ports.iter().map(requirement).collect();
    Ok(GeneratedForePlan { kernel, fore })
}
pub(crate) fn prepare_fore(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    bounds: EmbeddedImageBounds,
    computations: EmbeddedComputationBounds,
    maximum_fore_ports: usize,
    allow_lines: bool,
) -> Result<(GeneratedEmbeddedPlan, Vec<LoweredForePort>), GenerationError> {
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
        || (!allow_lines && lowered.fore_ports.iter().any(|p| p.selected_line.is_some()))
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
    Ok((kernel, lowered.fore_ports))
}
/// Project the exact Fore pins of one sealed Plan fragment during cold Line
/// selection, without generating a kernel image or claiming native readiness.
/// The same canonical lowering and ABI projection are used by both assigned
/// firmware schemas. The caller still owns carrier selection and admission.
pub fn assigned_fore_inventory(
    plan: &c::Plan,
    fragment: &c::FragmentId,
    maximum_fore_ports: usize,
) -> Result<Vec<c::AssignedForeRequirement>, GenerationError> {
    if !c::verify_plan(plan) {
        return Err(GenerationError::InvalidFragment);
    }
    if maximum_fore_ports > 16 {
        return Err(GenerationError::BoundExceeded {
            table: "local Fore profile capacity",
            actual: maximum_fore_ports as u64,
            maximum: 16,
        });
    }
    let (lowered, _) = lower_plan_fragment_from_plan(plan, fragment)
        .map_err(|_| GenerationError::InvalidFragment)?;
    if lowered.fore_ports.len() > maximum_fore_ports {
        return Err(GenerationError::BoundExceeded {
            table: "local Fore ports",
            actual: lowered.fore_ports.len() as u64,
            maximum: maximum_fore_ports as u64,
        });
    }
    Ok(lowered.fore_ports.iter().map(requirement).collect())
}

pub(crate) fn requirement(port: &LoweredForePort) -> c::AssignedForeRequirement {
    let abi = c::assigned_fore_abi_digest(c::AssignedForeAbi {
        value_kind: port.value_kind.as_str(),
        value_contract: port.value_contract.as_ref(),
        abnormal_kind: port.abnormal_kind.as_ref().map(|kind| kind.as_str()),
        temporal: port.temporal.as_str(),
        track: port.track,
        quantity_profile: port.quantity_profile,
        item_capacity: port.item_capacity,
        byte_capacity: port.byte_capacity,
    });
    c::AssignedForeRequirement::new(
        port.endpoint.0,
        port.cord.0,
        match port.direction {
            c::PortDirection::Input => c::AssignedForeDirection::Input,
            c::PortDirection::Output => c::AssignedForeDirection::Output,
        },
        c::AssignedIdentity::from_text(port.front_port_id.as_str()),
        abi,
    )
}

pub(crate) fn has_local_fore_endpoint(plan: &GeneratedEmbeddedPlan) -> bool {
    plan.cords.iter().any(|cord| {
        [cord.source, cord.sink].iter().any(|endpoint| {
            matches!(endpoint, GeneratedCordEndpoint::Remote { endpoint }
                if usize::from(*endpoint) >= plan.remote_endpoints.len())
        })
    })
}
