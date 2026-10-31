//! Initialize and retain each software Back before publishing its offer.
//! Root supplies actual storage offers and choices; no grant or start is issued.
use super::*;
use conduit_environment_runtime::observation::ObservationContract;
use conduit_planner::{PlacementChoice, PlacementChoices};

#[derive(Debug, PartialEq, Eq)]
pub enum RecordingSoftwarePreparationRefusal {
    Source,
    ReadSelection,
    Software(r::RecoveryInstallationRefusal),
}
impl From<r::RecoveryInstallationRefusal> for RecordingSoftwarePreparationRefusal {
    fn from(value: r::RecoveryInstallationRefusal) -> Self {
        Self::Software(value)
    }
}
/// Root retains these actual initialized owners through planning and transfers
/// them once into the selected production kernel. Choices describe availability,
/// not authority; ordinary planning still checks grants and resource provenance.
pub struct ReadyRecordingBatchSoftware {
    pub choices: PlacementChoices,
    pub generations: Vec<r::ReadyRecordingGenerations>,
    pub singletons: Vec<r::ReadyRecordingSingleton>,
    pub concats: Vec<r::ReadyRecordingConcat>,
    pub recovery: r::ReadyRecordingRecovery,
}
impl ReadyRecordingBatchSoftware {
    /// All construction allocation precedes Play. Failure leaves the supplied
    /// Host advertisement untouched. Read choices must cover only and exactly
    /// this Source's reads on this Host; store eligibility remains the planner's
    /// responsibility, including the exact current Base and independent grants.
    pub fn prepare(
        source: &RecordingBatchRecoverySource,
        host: &mut c::HostAdvertisement,
        read_choices: &PlacementChoices,
        context: r::RecordingContext<'_>,
        observation: &ObservationContract,
    ) -> Result<Self, RecordingSoftwarePreparationRefusal> {
        use RecordingSoftwarePreparationRefusal as E;
        let expanded = source.expanded();
        let count = source.references().len();
        if read_choices.by_gear.len() != count {
            return Err(E::ReadSelection);
        }
        for (gear, choice) in &read_choices.by_gear {
            if choice.host_id != host.host_id
                || !expanded.gears.iter().any(|g| {
                    &g.gear_id == gear && g.kind_id.as_str() == "resource/read-json-snapshot"
                })
                || !host.capabilities.iter().any(|offer| {
                    offer.capability_id == choice.capability_id
                        && offer.kind_id.as_str() == "resource/read-json-snapshot"
                })
            {
                return Err(E::ReadSelection);
            }
        }
        let mut next = host.clone();
        let mut choices = read_choices.clone();
        let mut generations = Vec::with_capacity(count);
        let mut singletons = Vec::with_capacity(count);
        let mut concats = Vec::with_capacity(count - 1);
        for reference in source.references() {
            let encoded = reference.encode().map_err(|_| E::Source)?;
            let hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
            let read = expanded
                .gears
                .iter()
                .find(|g| {
                    g.kind_id.as_str() == "resource/read-json-snapshot"
                        && g.configuration.iter().any(|entry| {
                            entry.key == "reference"
                                && matches!(&entry.value, c::ConfigurationValue::Text(value) if value.as_str() == hex)
                        })
                })
                .ok_or(E::Source)?;
            let generation = &expanded
                .connections
                .iter()
                .find(|cord| cord.sink_gear_id == read.gear_id)
                .ok_or(E::Source)?
                .source_gear_id;
            let mut owner = r::ReadyRecordingGenerations::prepare_generation_for(
                expanded,
                host.host_id.clone(),
                host.boot_id.clone(),
                generation,
                reference,
            )?;
            publish(&mut owner, &mut next, &mut choices)?;
            generations.push(owner);
        }
        for gear in &expanded.gears {
            match gear.kind_id.as_str() {
                "flow/singleton" => {
                    let mut owner = r::ReadyRecordingSingleton::prepare_singleton_for(
                        expanded,
                        host.host_id.clone(),
                        host.boot_id.clone(),
                        &gear.gear_id,
                    )?;
                    publish(&mut owner, &mut next, &mut choices)?;
                    singletons.push(owner);
                }
                "flow/concat/finite" => {
                    let mut owner = r::ReadyRecordingConcat::prepare_concat_for(
                        expanded,
                        host.host_id.clone(),
                        host.boot_id.clone(),
                        &gear.gear_id,
                    )?;
                    publish(&mut owner, &mut next, &mut choices)?;
                    concats.push(owner);
                }
                _ => {}
            }
        }
        let mut recovery = r::ReadyRecordingRecovery::prepare(
            expanded,
            host.host_id.clone(),
            host.boot_id.clone(),
            context,
            observation,
            count as u16,
        )?;
        publish(&mut recovery, &mut next, &mut choices)?;
        if generations.len() != count
            || singletons.len() != count
            || concats.len() + 1 != count
            || choices.by_gear.len() != expanded.gears.len()
        {
            return Err(E::Source);
        }
        *host = next;
        Ok(Self {
            choices,
            generations,
            singletons,
            concats,
            recovery,
        })
    }
}
fn publish<B>(
    owner: &mut r::ReadyRecordingSoftware<B>,
    host: &mut c::HostAdvertisement,
    choices: &mut PlacementChoices,
) -> Result<(), RecordingSoftwarePreparationRefusal> {
    owner.append_to_advertisement(host)?;
    let (gear, selected_host, capability) = owner
        .current_selection()
        .ok_or(RecordingSoftwarePreparationRefusal::Source)?;
    if choices
        .by_gear
        .insert(
            gear.clone(),
            PlacementChoice {
                host_id: selected_host.clone(),
                capability_id: capability.clone(),
            },
        )
        .is_some()
    {
        return Err(RecordingSoftwarePreparationRefusal::Source);
    }
    Ok(())
}
