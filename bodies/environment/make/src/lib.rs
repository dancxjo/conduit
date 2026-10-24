//! Host-side construction over actual advertised offers, using the ordinary
//! Source checker, planner and embedded image producer. No provider is invented.
//! The result is construction material; Root must still authenticate it, admit
//! current possession and storage, and advance the real Body/Wake before Play.
extern crate alloc;
mod display_material;
mod display_output;
mod display_observation;
pub use display_material::DisplaySourceMaterial;
pub use display_output::{DisplayOutputSource, DisplayPlanningRefusal, PreparedDisplayOutput};
mod recording_recovery;
pub use recording_recovery::{
    PreparedRecordingRecovery, ReadyRecordingBatchSoftware, RecordingBatchRecoverySource,
    RecordingPlanningRefusal, RecordingRecoverySource, RecordingSoftwarePreparationRefusal,
};
mod source_recipe;
pub use source_recipe::NativeSoftwareSourceRecipe;
mod native_limits;
mod observed_offers;
mod observed_policy;
pub use observed_offers::NativeOfferRefusal;
mod software_recipe;
pub use software_recipe::{NativeSoftwareRecipe, SoftwareRecipeRefusal};
mod execution;
pub use execution::{NativeBodyBinding, NativeBodyBindingRefusal};
mod network;
mod network_planning;
mod network_source;
pub use network::{
    compile_network_setup, NetworkPlanningRefusal, PreparedNativeNetworkSetup, PreparedNetworkSetup,
};

use conduit_core as c;
use conduit_embedded_build as e;
pub use conduit_environment_runtime::construction::{
    compile_default as compile_appliance_source, ApplianceSource,
};
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
    SoftwareOffer,
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
    selected_offers: Vec<(u16, c::CapabilityOffer)>,
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
        let mut selected_offers = Vec::new();
        for gear in &fragment.placements {
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == gear.capability_id)
                .ok_or(E::SoftwareOffer)?;
            let node = lowered
                .identity
                .node_for_placement(&gear.placement_id)
                .ok_or(E::SoftwareOffer)?;
            selected_offers.push((node.0, offer.clone()));
        }
        Ok(Self {
            plan,
            lowered,
            image,
            assignment,
            resident_records,
            computations,
            selected_offers,
        })
    }
    /// Workset identity only; does not clone the rich Plot Plan.
    pub fn resident_plot(&self) -> conduit_body::ResidentPlot {
        conduit_body::ResidentPlot::new(
            self.plan.source_document_id.clone(),
            self.plan.checked_plot_id.clone(),
        )
    }
    /// Supply this checked Source/Plan partition to the ordinary Body owner.
    /// This host-side rich Plan clone confers no presence or execution claim.
    pub fn body_plot_plan(&self) -> conduit_body::BodyPlotPlan {
        conduit_body::BodyPlotPlan {
            plot: self.resident_plot(),
            plan: self.plan.clone(),
        }
    }
    /// Propose the single appliance workload through the existing lifecycle.
    /// The caller supplies its admitted Mask topology explicitly. The ordinary
    /// owner checks the complete workset and exact current Host/Boot membership
    /// before retaining Wake evidence. Refusal leaves that owner unchanged.
    /// Root must persist the proposal, admit actual MCU resources and obtain
    /// an attributable start result before calling its existing `started` API.
    pub fn propose_to_body<'a>(
        &self,
        owner: &'a mut conduit_body::BodyLifecycleSession,
        masks: Vec<conduit_body::BodyMaskTopology>,
    ) -> Result<&'a conduit_body::BodyLifecycleRealization, conduit_body::BodyLifecycleSessionError>
    {
        let fragment = &self.plan.fragments[0]; // prepare admits exactly one
        owner.propose_with_masks(
            vec![self.body_plot_plan()],
            masks,
            &fragment.host_id,
            &fragment.boot_id,
        )
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

mod catalog_seed;
pub use catalog_seed::{
    PreparedRecordingCatalogCheckpoint, RecordingCatalogSeedSource, MAXIMUM_CATALOG_SEED_BYTES,
};

mod recording_publication;
pub use recording_publication::RecordingPublicationSource;
