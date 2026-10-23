//! Trusted Make construction material kept separate from an incoming assignment.
//! No Host/Boot/Plan identity, native readiness or capability is created here.
mod rust;
use crate::PreparedNativeAppliance;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::assigned::{ResidentAppliance, ResidentComputation, ResidentRoles},
    assigned::ResidentLayout,
};
pub(crate) use rust::write_material;
#[derive(Debug, PartialEq, Eq)]
pub enum SoftwareRecipeRefusal {
    Roles,
    Records(c::AssignedPlanRefusal),
    Capacity,
    Offers,
    Source,
}
#[derive(Debug, PartialEq, Eq)]
pub struct NativeSoftwareRecipe {
    records: Vec<c::AssignedRecordRequirement>,
    source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity,
    cadence: conduit_time::TickConfiguration,
    texts: Vec<(u16, c::AssignedIdentity, String, Vec<u8>)>,
    computations: Vec<c::AssignedComputationRequirement>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    roles: ResidentRoles,
    largest: u16,
    total: u32,
    offers: Vec<(u16, Vec<u8>)>,
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
        let offers = self
            .selected_offers
            .iter()
            .filter(|(node, _)| *node != roles.pulse.0 && *node != roles.tick.0)
            .map(|(node, offer)| {
                use conduit_environment_runtime::appliance::software::{ARTIFACT, PROFILE};
                if offer.implementation.execution_profile_id.as_str() != PROFILE
                    || offer.implementation.artifact_id.as_str() != ARTIFACT
                    || !offer.resource_requirements.is_empty()
                    || !offer.authority_requirements.is_empty()
                {
                    return Err(E::Offers);
                }
                Ok((*node, serde_json::to_vec(offer).map_err(|_| E::Offers)?))
            })
            .collect::<Result<Vec<_>, E>>()?;
        Ok(NativeSoftwareRecipe {
            cadence: conduit_time::parse_tick_configuration(&self.plan().fragments[0].placements.iter().find(|g|g.kind_id.as_str()==conduit_time::TICK_KIND).ok_or(E::Source)?.configuration).map_err(|_|E::Source)?,
            source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity::from_expanded(&self.plan().fragments[0].expanded_plot_id),
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
                        text.as_bytes()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| {
                                u8::from_str_radix(
                                    core::str::from_utf8(pair).expect("checked hex"),
                                    16,
                                )
                                .expect("checked hex")
                            })
                            .collect(),
                    ))
                })
                .collect(),
            computations: self.computations().to_vec(),
            counts,
            roles,
            largest,
            total,
            offers,
        })
    }
}
impl NativeSoftwareRecipe {
    /// Build-side views over independently owned computation data. Firmware can
    /// instead borrow the static array emitted by `write_resident_rust`.
    pub fn resident_computations(&self) -> impl ExactSizeIterator<Item = ResidentComputation<'_>> {
        self.texts
            .iter()
            .map(|(node, key, text, bytes)| ResidentComputation {
                node: *node,
                key: *key,
                text,
                canonical_bytes: Some(bytes),
            })
    }

    /// This independent owned recipe survives dropping rich Make objects. These
    /// digests are compilation constraints, never possession or storage admission.
    pub fn resident(&self) -> ResidentAppliance<'_> {
        ResidentAppliance {
            cadence: self.cadence.clone(),
            source: self.source,
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

impl NativeSoftwareRecipe {
    /// Host-free descriptors, not readiness. Only the corresponding initialized
    /// owner can lend an offer view; native pulse/timer descriptors are excluded.
    pub fn resident_offers(
        &self,
    ) -> Vec<conduit_environment_runtime::appliance::assigned::ResidentSoftwareOffer<'_>> {
        self.offers.iter().map(|(node, json)| {
            // SAFETY: these exact serialized CapabilityOffers were selected from
            // supplied current offers for the same checked Source and retained
            // with this independently owned Make construction recipe.
            unsafe { conduit_environment_runtime::appliance::assigned::ResidentSoftwareOffer::from_make(*node, json) }
        }).collect()
    }
}
