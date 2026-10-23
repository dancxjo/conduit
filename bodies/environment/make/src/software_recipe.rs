//! Trusted Make construction material kept separate from an incoming assignment.
//! No Host/Boot/Plan identity, native readiness or capability is created here.
mod rust;
use crate::PreparedNativeAppliance;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::assigned::{ResidentAppliance, ResidentComputation, ResidentRoles},
    assigned::ResidentLayout,
};
#[derive(Debug, PartialEq, Eq)]
pub enum SoftwareRecipeRefusal {
    Roles,
    Records(c::AssignedPlanRefusal),
    Capacity,
}
#[derive(Debug, PartialEq, Eq)]
pub struct NativeSoftwareRecipe {
    records: Vec<c::AssignedRecordRequirement>,
    texts: Vec<(u16, c::AssignedIdentity, String)>,
    computations: Vec<c::AssignedComputationRequirement>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    roles: ResidentRoles,
    largest: u16,
    total: u32,
}
impl PreparedNativeAppliance {
    /// Extract from this checked, ordinarily planned Make result. Authenticate
    /// and pin it through Root provisioning independently of received assignment
    /// bytes; deriving expectations from those bytes would confer no trust.
    pub fn software_recipe(&self) -> Result<NativeSoftwareRecipe, SoftwareRecipeRefusal> {
        use SoftwareRecipeRefusal as E;
        let roles = ResidentRoles::from_nodes(
            self.image()
                .nodes
                .iter()
                .map(|n| (n.node, n.kind_id.as_str())),
        )
        .map_err(|_| E::Roles)?;
        let mut counts = [0_u8; c::ASSIGNED_PLAN_COUNT_KINDS];
        for record in c::AssignedPlanRecords::new(self.assignment()).map_err(E::Records)? {
            let record = record.map_err(E::Records)?;
            let count = counts
                .get_mut(usize::from(record.tag).checked_sub(1).ok_or(E::Capacity)?)
                .ok_or(E::Capacity)?;
            *count = count.checked_add(1).ok_or(E::Capacity)?;
        }
        let largest = self
            .computations()
            .iter()
            .map(|r| r.text_bytes)
            .max()
            .ok_or(E::Capacity)?;
        let total = self
            .computations()
            .iter()
            .try_fold(0_u32, |sum, r| sum.checked_add(u32::from(r.text_bytes)))
            .ok_or(E::Capacity)?;
        Ok(NativeSoftwareRecipe {
            records: self.resident_records().to_vec(),
            texts: self
                .image()
                .configuration
                .iter()
                .filter_map(|entry| {
                    if !matches!(entry.key.as_str(), "program" | "selector") {
                        return None;
                    }
                    let conduit_embedded_build::GeneratedConfigurationValue::Text(text) =
                        &entry.value
                    else {
                        return None;
                    };
                    Some((
                        entry.node,
                        c::AssignedIdentity::from_text(&entry.key),
                        text.clone(),
                    ))
                })
                .collect(),
            computations: self.computations().to_vec(),
            counts,
            roles,
            largest,
            total,
        })
    }
}
impl NativeSoftwareRecipe {
    /// Build-side views over independently owned computation data. Firmware can
    /// instead borrow the static array emitted by `write_resident_rust`.
    pub fn resident_computations(&self) -> impl ExactSizeIterator<Item = ResidentComputation<'_>> {
        self.texts
            .iter()
            .map(|(node, key, text)| ResidentComputation {
                node: *node,
                key: *key,
                text,
            })
    }

    /// This independent owned recipe survives dropping rich Make objects. These
    /// digests are compilation constraints, never possession or storage admission.
    pub fn resident(&self) -> ResidentAppliance<'_> {
        ResidentAppliance {
            layout: ResidentLayout {
                records: &self.records,
                counts: self.counts,
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: self.largest,
                maximum_total_text_bytes: self.total,
                requirements: &self.computations,
            },
            roles: self.roles,
        }
    }
}
