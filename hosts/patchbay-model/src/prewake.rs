//! Safe PREWAKE rehearsal through ordinary checking, planning, and std-host kernel execution.

use crate::{AuthoredEnvironment, FormEditor, MachineProfile};
use conduit_core::{
    ActivePlayId, BootId, ConnectionBase, HostAdvertisement, HostId, OfferGeneration, Plan, PlanId,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use std::collections::VecDeque;

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

#[derive(Debug, Clone)]
pub struct PrewakeController {
    state: PrewakeState,
    hold: bool,
    provenance: PrewakeProvenance,
    history: VecDeque<(PrewakeBasis, PlanId, Option<SimulatedPlay>)>,
    last_refusal: Option<PrewakeError>,
}

impl Default for PrewakeController {
    fn default() -> Self {
        Self {
            state: PrewakeState::Off,
            hold: false,
            provenance: PrewakeProvenance::default(),
            history: VecDeque::with_capacity(MAX_PREWAKE_HISTORY),
            last_refusal: None,
        }
    }
}

impl PrewakeController {
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
            .and_then(|(basis, plan, mut hosts)| {
                if self.hold {
                    Ok(PrewakeState::Held { basis, plan })
                } else {
                    let play = execute_simulated(&plan, &mut hosts)?;
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
        let (current_basis, current_plan, mut hosts) = self.prepare(editor, environment)?;
        if held_basis != current_basis || held_plan.plan_id != current_plan.plan_id {
            let error = PrewakeError::StaleHeldPlan {
                plan_id: held_plan.plan_id,
            };
            self.last_refusal = Some(error.clone());
            return Err(error);
        }
        let play = execute_simulated(&held_plan, &mut hosts)?;
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
    }

    fn prepare(
        &self,
        editor: &FormEditor,
        environment: &AuthoredEnvironment,
    ) -> Result<(PrewakeBasis, Plan, Vec<StdHost>), PrewakeError> {
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
        let hosts = simulated_hosts(environment);
        let advertisements = hosts
            .iter()
            .map(|host| host.advertisement().clone())
            .collect::<Vec<HostAdvertisement>>();
        let placements = default_expanded_placements(&expanded, &advertisements)
            .map_err(|error| PrewakeError::Planning(error.to_string()))?;
        let plan = plan_expanded_canonical(
            &expanded,
            &advertisements,
            &placements,
            &[ConnectionBase::Local],
        )
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
            hosts,
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

fn simulated_hosts(environment: &AuthoredEnvironment) -> Vec<StdHost> {
    let mut hosts = environment
        .simulation_projection()
        .expect("validated environment projects")
        .hosts
        .into_iter()
        .map(|candidate| {
            let composition = match candidate.profile {
                MachineProfile::PicoW => StdHostComposition::minimal()
                    .with_signal()
                    .with_time()
                    .with_state()
                    .with_logic()
                    .with_math()
                    .with_robotics(),
                MachineProfile::RaspberryPi5 | MachineProfile::LaptopLinux => {
                    StdHostComposition::reference()
                }
            };
            StdHost::new_with_composition(
                StdHostConfig {
                    host_id: HostId::from(candidate.host_id),
                    boot_id: BootId::from(candidate.boot_id),
                    offer_generation: OfferGeneration(environment.revision),
                },
                composition,
            )
        })
        .collect::<Vec<_>>();
    hosts.sort_by_key(|host| {
        (
            std::cmp::Reverse(host.advertisement().capabilities.len()),
            host.advertisement().host_id.clone(),
        )
    });
    hosts
}

fn execute_simulated(plan: &Plan, hosts: &mut [StdHost]) -> Result<SimulatedPlay, PrewakeError> {
    let mut active_play_ids = Vec::with_capacity(plan.fragments.len());
    let mut output = Vec::with_capacity(4096);
    let mut kernel_sign = Vec::new();
    for fragment in &plan.fragments {
        let host = hosts
            .iter_mut()
            .find(|host| {
                host.advertisement().host_id == fragment.host_id
                    && host.advertisement().boot_id == fragment.boot_id
            })
            .ok_or_else(|| {
                PrewakeError::Simulation("planned simulated Host/Boot is absent".into())
            })?;
        let report = host
            .run_fragment_to(fragment.clone(), &mut output, &mut ThreadTimer)
            .map_err(PrewakeError::Simulation)?;
        let kernel = report.kernel.ok_or_else(|| {
            PrewakeError::Simulation("simulated kernel omitted its terminal report".into())
        })?;
        kernel_sign.extend(kernel.kernel_sign);
        active_play_ids.push(kernel.active_play_id);
    }
    Ok(SimulatedPlay {
        plan_id: plan.plan_id.clone(),
        active_play_ids,
        output,
        kernel_sign,
        terminal: SimulatedPlayTerminal::Completed,
    })
}
