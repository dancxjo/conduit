pub use super::network_source::compile_network_setup;

pub use super::network_planning::{NetworkPlanningRefusal, PreparedNetworkSetup};

impl PreparedNetworkSetup {
    /// Compile authored meaning on the build host, then use the same preparation
    /// path that can run against fresh offers and grants on a constrained Root.
    pub fn prepare(
        host: &conduit_core::HostAdvertisement,
        bases: &[conduit_core::BaseImplementationId],
        authority: &[conduit_core::AuthorityGrant],
    ) -> Result<Self, String> {
        let source = compile_network_setup()?;
        Self::from_checked(&source, host, bases, authority)
            .map_err(|error| format!("network preparation: {error:?}"))
    }
    /// Ordinary Body workset material, not a proposal, Wake or start receipt.
    pub fn body_plot_plan(&self) -> conduit_body::BodyPlotPlan {
        conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                self.plan.source_document_id.clone(),
                self.plan.checked_plot_id.clone(),
            ),
            plan: self.plan.clone(),
        }
    }
}

/// Host-produced native network installation material. This is neither a Root
/// admission nor a Body start. Deliver the assignment and resident layout through
/// authenticated installation; never derive trusted layout from received bytes.
pub struct PreparedNativeNetworkSetup {
    checked: PreparedNetworkSetup,
    image: conduit_embedded_build::GeneratedEmbeddedPlan,
    assignment: Vec<u8>,
    resident_records: Vec<conduit_core::AssignedRecordRequirement>,
}
impl PreparedNetworkSetup {
    pub fn prepare_native(
        self,
        bounds: conduit_embedded_build::EmbeddedImageBounds,
        maxima: conduit_core::AssignedPlanMaxima,
    ) -> Result<PreparedNativeNetworkSetup, conduit_embedded_build::GenerationError> {
        use conduit_embedded_build as e;
        let image = e::generate_embedded_plan(&self.plan.fragments[0], &self.lowered, bounds)?;
        let assignment = e::encode_assigned_native_plan(&image, maxima)?;
        let resident_records = e::assigned_native_layout_requirements(&image, maxima)?;
        Ok(PreparedNativeNetworkSetup {
            checked: self,
            image,
            assignment,
            resident_records,
        })
    }
}
impl PreparedNativeNetworkSetup {
    pub fn checked(&self) -> &PreparedNetworkSetup {
        &self.checked
    }
    pub fn image(&self) -> &conduit_embedded_build::GeneratedEmbeddedPlan {
        &self.image
    }
    pub fn assignment(&self) -> &[u8] {
        &self.assignment
    }
    pub fn resident_records(&self) -> &[conduit_core::AssignedRecordRequirement] {
        &self.resident_records
    }
}
