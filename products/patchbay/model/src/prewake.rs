//! Safe PREWAKE rehearsal through ordinary checking, planning, and std-host kernel execution.

use crate::{AuthoredEnvironment, FormEditor, MachineProfile};
use conduit_core::{
    ActivePlayId, BaseImplementationId, BootId, HostAdvertisement, HostId, OfferGeneration, Plan,
    PlanId,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical, plan_expanded_canonical_with_options,
    PlanningOptions,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

pub const MAX_PREWAKE_HISTORY: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrewakeProvenance {
    pub simulation_truth: bool,
    pub observed_live_truth: bool,
    pub physical_effect_authority: bool,
    pub promotable_to_physical_plan: bool,
}

impl Default for PrewakeProvenance {
    fn default() -> Self {
        Self {
            simulation_truth: true,
            observed_live_truth: false,
            physical_effect_authority: false,
            promotable_to_physical_plan: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrewakeBasis {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub environment_id: String,
    pub environment_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedPlay {
    pub plan_id: PlanId,
    pub active_play_ids: Vec<ActivePlayId>,
    pub output: Vec<u8>,
    pub kernel_sign: Vec<conduit_kernel::KernelEvent>,
    pub terminal: SimulatedPlayTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatedPlayTerminal {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrewakeState {
    Off,
    Auto {
        basis: PrewakeBasis,
        plan: Plan,
        play: SimulatedPlay,
    },
    Held {
        basis: PrewakeBasis,
        plan: Plan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrewakeError {
    NotEntered,
    InvalidForm,
    MissingEnvironmentPart,
    Planning(String),
    Simulation(String),
    NoHeldPlan,
    StaleHeldPlan { plan_id: PlanId },
}

impl std::fmt::Display for PrewakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PREWAKE refusal: {self:?}")
    }
}
impl std::error::Error for PrewakeError {}

#[derive(Clone)]
pub struct PrewakeController {
    adapter: Arc<dyn crate::PatchbayHostAdapter>,
    state: PrewakeState,
    hold: bool,
    provenance: PrewakeProvenance,
    history: VecDeque<(PrewakeBasis, PlanId, Option<SimulatedPlay>)>,
    last_refusal: Option<PrewakeError>,
    implementation_preferences:
        BTreeMap<conduit_core::GearId, (HostId, conduit_core::CapabilityId)>,
}

impl std::fmt::Debug for PrewakeController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrewakeController")
            .field("state", &self.state)
            .field("hold", &self.hold)
            .field("provenance", &self.provenance)
            .field("history", &self.history)
            .field("last_refusal", &self.last_refusal)
            .field(
                "implementation_preferences",
                &self.implementation_preferences,
            )
            .finish_non_exhaustive()
    }
}

impl PrewakeController {
    pub fn new(adapter: Arc<dyn crate::PatchbayHostAdapter>) -> Self {
        Self {
            adapter,
            state: PrewakeState::Off,
            hold: false,
            provenance: PrewakeProvenance::default(),
            history: VecDeque::with_capacity(MAX_PREWAKE_HISTORY),
            last_refusal: None,
            implementation_preferences: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> &PrewakeState {
        &self.state
    }
    pub fn provenance(&self) -> PrewakeProvenance {
        self.provenance
    }
    pub fn hold_enabled(&self) -> bool {
        self.hold
    }
    pub fn last_refusal(&self) -> Option<&PrewakeError> {
        self.last_refusal.as_ref()
    }
    pub fn history(&self) -> &VecDeque<(PrewakeBasis, PlanId, Option<SimulatedPlay>)> {
        &self.history
    }
    pub fn enter(
        &mut self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
    ) -> Result<(), PrewakeError> {
        self.rehearse(editor, environment)
    }
    pub fn set_hold(&mut self, enabled: bool) {
        self.hold = enabled;
    }

    pub fn rehearse(
        &mut self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
    ) -> Result<(), PrewakeError> {
        let result = self
            .prepare(editor, environment)
            .and_then(|(basis, plan, hosts)| {
                if self.hold {
                    Ok(PrewakeState::Held { basis, plan })
                } else {
                    let play = execute_simulated(self.adapter.as_ref(), &plan, &hosts)?;
                    Ok(PrewakeState::Auto { basis, plan, play })
                }
            });
        match result {
            Ok(next) => {
                self.retain_current();
                self.state = next;
                self.last_refusal = None;
                Ok(())
            }
            Err(error) => {
                self.last_refusal = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn release(
        &mut self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
    ) -> Result<(), PrewakeError> {
        let (held_basis, held_plan) = match &self.state {
            PrewakeState::Held { basis, plan } => (basis.clone(), plan.clone()),
            PrewakeState::Off => return Err(PrewakeError::NotEntered),
            PrewakeState::Auto { .. } => return Err(PrewakeError::NoHeldPlan),
        };
        let (current_basis, current_plan, hosts) = self.prepare(editor, environment)?;
        if held_basis != current_basis || held_plan.plan_id != current_plan.plan_id {
            let error = PrewakeError::StaleHeldPlan {
                plan_id: held_plan.plan_id,
            };
            self.last_refusal = Some(error.clone());
            return Err(error);
        }
        let play = execute_simulated(self.adapter.as_ref(), &held_plan, &hosts)?;
        self.retain_current();
        self.state = PrewakeState::Auto {
            basis: held_basis,
            plan: held_plan,
            play,
        };
        self.last_refusal = None;
        Ok(())
    }

    pub fn exit(&mut self) {
        self.retain_current();
        self.state = PrewakeState::Off;
        self.hold = false;
        self.last_refusal = None;
        self.implementation_preferences.clear();
    }

    pub fn realization_inspection(
        &self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
        subject: &crate::PatchbaySubjectRef,
    ) -> Result<crate::GearRealizationInspection, PrewakeError> {
        let expanded = editor
            .expand_form(&editor.view().open_form)
            .map_err(|_| PrewakeError::InvalidForm)?;
        let graph = crate::PatchbayGraph::from_expanded(&expanded)
            .map_err(|_| PrewakeError::InvalidForm)?;
        let plan = match &self.state {
            PrewakeState::Auto { plan, .. } | PrewakeState::Held { plan, .. } => Some(plan),
            PrewakeState::Off => None,
        };
        crate::GearRealizationInspection::inspect(
            &graph,
            subject,
            plan,
            &simulated_advertisements(self.adapter.as_ref(), environment),
        )
        .map_err(|error| PrewakeError::Planning(error.to_string()))
    }

    pub fn request_next_implementation(
        &mut self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
        subject: &crate::PatchbaySubjectRef,
    ) -> Result<(), PrewakeError> {
        let inspection = match self.realization_inspection(editor, environment, subject) {
            Ok(inspection) => inspection,
            Err(error) => {
                self.last_refusal = Some(error.clone());
                return Err(error);
            }
        };
        let gear_id = inspection.gear_id.clone();
        let alternative = inspection
            .alternatives
            .iter()
            .find(|candidate| candidate.disposition == crate::RealizationDisposition::Compatible)
            .ok_or_else(|| {
                PrewakeError::Planning(
                    "no other compatible simulated implementation is available".into(),
                )
            });
        let alternative = match alternative {
            Ok(alternative) => alternative,
            Err(error) => {
                self.last_refusal = Some(error.clone());
                return Err(error);
            }
        };
        let prior = self.implementation_preferences.insert(
            gear_id.clone(),
            (
                alternative.host_id.clone(),
                alternative.capability_id.clone(),
            ),
        );
        if let Err(error) = self.rehearse(editor, environment) {
            match prior {
                Some(value) => {
                    self.implementation_preferences.insert(gear_id, value);
                }
                None => {
                    self.implementation_preferences.remove(&gear_id);
                }
            }
            return Err(error);
        }
        Ok(())
    }

    fn prepare(
        &self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
    ) -> Result<(PrewakeBasis, Plan, Vec<HostAdvertisement>), PrewakeError> {
        environment
            .validate()
            .map_err(|error| PrewakeError::Planning(error.to_string()))?;
        if environment.parts.is_empty() {
            return Err(PrewakeError::MissingEnvironmentPart);
        }
        let view = editor.view();
        if view.checked.revision != view.revision || !view.checked.diagnostics.is_empty() {
            return Err(PrewakeError::InvalidForm);
        }
        let expanded = editor
            .expand_form(&view.open_form)
            .map_err(|_| PrewakeError::InvalidForm)?;
        let advertisements = simulated_advertisements(self.adapter.as_ref(), environment);
        let mut placements = default_expanded_placements(&expanded, &advertisements)
            .map_err(|error| PrewakeError::Planning(error.to_string()))?;
        for (gear_id, (host_id, capability_id)) in &self.implementation_preferences {
            placements.by_gear.insert(
                gear_id.clone(),
                conduit_planner::PlacementChoice {
                    host_id: host_id.clone(),
                    capability_id: capability_id.clone(),
                },
            );
        }
        let plan = if expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND)
        {
            plan_expanded_canonical_with_options(
                &expanded,
                &advertisements,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
                PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_BYTES,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
        } else {
            plan_expanded_canonical(
                &expanded,
                &advertisements,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
            )
        }
        .map_err(|error| PrewakeError::Planning(error.to_string()))?;
        let source_document_id = view
            .checked
            .source_document_id
            .clone()
            .ok_or(PrewakeError::InvalidForm)?;
        Ok((
            PrewakeBasis {
                source_document_id,
                environment_id: environment.environment_id.clone(),
                environment_revision: environment.revision,
            },
            plan,
            advertisements,
        ))
    }

    fn retain_current(&mut self) {
        let retained = match &self.state {
            PrewakeState::Off => None,
            PrewakeState::Held { basis, plan } => Some((basis.clone(), plan.plan_id.clone(), None)),
            PrewakeState::Auto { basis, plan, play } => {
                Some((basis.clone(), plan.plan_id.clone(), Some(play.clone())))
            }
        };
        if let Some(retained) = retained {
            if self.history.len() == MAX_PREWAKE_HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(retained);
        }
    }
}

#[cfg(test)]
impl Default for PrewakeController {
    fn default() -> Self {
        Self::new(crate::host_adapter::test_host_adapter_arc())
    }
}

pub fn simulated_advertisements(
    adapter: &dyn crate::PatchbayHostAdapter,
    environment: &AuthoredEnvironment,
) -> Vec<HostAdvertisement> {
    let mut hosts = environment
        .simulation_projection()
        .expect("validated environment projects")
        .hosts
        .into_iter()
        .map(|candidate| {
            let profile = match candidate.profile {
                MachineProfile::PicoW => crate::PatchbayHostProfile::PicoSimulation,
                MachineProfile::RaspberryPi5 | MachineProfile::LaptopLinux => {
                    crate::PatchbayHostProfile::Reference
                }
            };
            let mut advertisement = adapter
                .advertisement(
                    HostId::from(candidate.host_id),
                    BootId::from(candidate.boot_id),
                    OfferGeneration(environment.revision),
                    profile,
                )
                .expect("validated simulation profile has an application adapter");
            if candidate.profile == MachineProfile::LaptopLinux {
                advertisement.resources.push(conduit_core::resource_offer(
                    &format!("prewake/{}/input", advertisement.boot_id.as_str()),
                    conduit_core::INPUT_RESOURCE_CLASS,
                    1,
                ));
                advertisement
                    .resources
                    .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
                advertisement.capabilities.push(simulated_keyboard_offer());
                advertisement
                    .capabilities
                    .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
            }
            advertisement
        })
        .collect::<Vec<_>>();
    hosts.sort_by_key(|host| {
        (
            std::cmp::Reverse(host.capabilities.len()),
            host.host_id.clone(),
        )
    });
    hosts
}

fn simulated_keyboard_offer() -> conduit_core::CapabilityOffer {
    let contract = conduit_semantic_catalog::keyboard_contract();
    conduit_core::CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: "prewake/simulated-keyboard@1".into(),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: "prewake/simulation@1".into(),
            implementation_id: "prewake/simulated-keyboard@1".into(),
            artifact_id: "prewake/simulated-keyboard-model@1".into(),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: "proof/input-next-key-event@1".into(),
            target_kind: Some(conduit_core::kind_id(conduit_human::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        resource_requirements: vec![conduit_core::resource_requirement(
            conduit_core::INPUT_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn execute_simulated(
    adapter: &dyn crate::PatchbayHostAdapter,
    plan: &Plan,
    hosts: &[HostAdvertisement],
) -> Result<SimulatedPlay, PrewakeError> {
    let mut active_play_ids = Vec::with_capacity(plan.fragments.len());
    let mut output = Vec::with_capacity(4096);
    let mut kernel_sign = Vec::new();
    for fragment in &plan.fragments {
        let host = hosts
            .iter()
            .find(|host| host.host_id == fragment.host_id && host.boot_id == fragment.boot_id)
            .ok_or_else(|| {
                PrewakeError::Simulation("planned simulated Host/Boot is absent".into())
            })?;
        let report = adapter
            .run_fragment(host, fragment.clone())
            .map_err(PrewakeError::Simulation)?;
        output.extend(report.output);
        kernel_sign.extend(report.projection.kernel_sign);
        active_play_ids.push(report.projection.active_play_id);
    }
    Ok(SimulatedPlay {
        plan_id: plan.plan_id.clone(),
        active_play_ids,
        output,
        kernel_sign,
        terminal: SimulatedPlayTerminal::Completed,
    })
}
