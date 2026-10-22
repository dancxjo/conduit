//! Host-side construction over actual advertised offers, using the ordinary
//! Source checker, planner and embedded image producer. No provider is invented.
//! The result is construction material; Root must still authenticate it, admit
//! current possession and storage, and advance the real Body/Wake before Play.
mod network;
pub use network::compile_network_setup;

use conduit_core as c;
use conduit_embedded_build as e;
use conduit_environment_runtime::construction::ApplianceSource;
use conduit_plan_lowering::lowering as l;

/// Caller-selected artifact ceilings, distinct from the runtime's memory budget.
#[derive(Clone, Copy, Debug)]
pub struct NativeApplianceLimits {
    pub image: e::EmbeddedImageBounds,
    pub computations: e::EmbeddedComputationBounds,
    pub assignment: c::AssignedPlanMaxima,
}

#[derive(Debug)]
pub enum NativePreparationRefusal {
    Planning(conduit_planner::PlannerError),
    Lowering(l::LoweringError),
    Projection(e::GenerationError),
    LocalFragment,
    Records(c::AssignedPlanRefusal),
}

/// Prepared on the planning host. Keep the rich Plan here for the real Body's
/// lifecycle and evidence. Installation must deliver the bounded assignment
/// and trusted resident metadata through authenticated Root admission.
pub struct PreparedNativeAppliance {
    plan: c::Plan,
    lowered: l::LoweredPlanFragment,
    image: e::GeneratedEmbeddedPlan,
    assignment: Vec<u8>,
    resident_records: Vec<c::AssignedRecordRequirement>,
    computations: Vec<c::AssignedComputationRequirement>,
}
impl PreparedNativeAppliance {
    /// The caller supplies authenticated, current offers, chosen Bases and Root
    /// authority. Historical descriptions do not establish current availability.
    pub fn prepare(
        source: &ApplianceSource,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
        limits: NativeApplianceLimits,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let plan = source
            .plan_local(host, bases, authority)
            .map_err(E::Planning)?;
        if plan.fragments.len() != 1
            || plan.fragments[0].host_id != host.host_id
            || plan.fragments[0].boot_id != host.boot_id
        {
            return Err(E::LocalFragment);
        }
        let fragment = &plan.fragments[0];
        let lowered = l::lower_plan_fragment(fragment).map_err(E::Lowering)?;
        let image = e::generate_embedded_plan_with_computations(
            fragment,
            &lowered,
            limits.image,
            limits.computations,
        )
        .map_err(E::Projection)?;
        let assignment =
            e::encode_assigned_native_plan(&image, limits.assignment).map_err(E::Projection)?;
        let mut resident_records =
            e::assigned_native_layout_requirements(&image, limits.assignment)
                .map_err(E::Projection)?;
        // Numeric cadence/count are Source too. Pin them alongside programs;
        // the projection's re-computable digest provides integrity, not authority.
        for record in c::AssignedPlanRecords::new(&assignment).map_err(E::Records)? {
            let record = record.map_err(E::Records)?;
            if record.tag == c::ASSIGNED_CONFIGURATION {
                resident_records.push(c::AssignedRecordRequirement::from_record(record));
            }
        }
        let computations = e::assigned_computation_requirements(&image).map_err(E::Projection)?;
        Ok(Self {
            plan,
            lowered,
            image,
            assignment,
            resident_records,
            computations,
        })
    }
    pub fn plan(&self) -> &c::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &l::LoweredPlanFragment {
        &self.lowered
    }
    pub fn image(&self) -> &e::GeneratedEmbeddedPlan {
        &self.image
    }
    pub fn assignment(&self) -> &[u8] {
        &self.assignment
    }
    pub fn resident_records(&self) -> &[c::AssignedRecordRequirement] {
        &self.resident_records
    }
    pub fn computations(&self) -> &[c::AssignedComputationRequirement] {
        &self.computations
    }
}
