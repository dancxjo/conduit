//! Ordinary portable presentation execution on the ConduitOS display Base.

mod bool_play;
mod flow_state_operation;
mod flow_state_plan;
mod flow_state_play;
#[cfg(test)]
mod flow_state_tests;
mod logic_multi_plan;
mod logic_multi_play;
#[cfg(test)]
mod logic_multi_tests;
mod logic_not_play;
mod math_clamp_play;
#[cfg(test)]
mod math_clamp_play_tests;
mod operation;
mod plan;
mod play;
mod portable_state_input_operation;
mod portable_state_input_plan;
mod portable_state_input_play;
#[cfg(test)]
mod portable_state_input_tests;
mod robotics_operation;
mod robotics_plan;
mod robotics_play;
#[cfg(test)]
mod robotics_tests;
mod state_select_operation;
mod state_select_plan;
mod state_select_play;
#[cfg(test)]
mod state_select_tests;

pub use crate::presentation_offers::{
    CONDUITOS_PRESENTATION_ARTIFACT, CONDUITOS_PRESENTATION_PROFILE, presentation_nucleus_offers,
};
pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const TEXT_SOURCE_KIND: &str = "conduitos/fixture-text-source";
pub use bool_play::{BoolPresentationError, BoolPresentationProof, prepare_bool, run_bool};
pub use flow_state_plan::{PreparedFlowState, prepare_flow_state};
pub use flow_state_play::{FlowStateError, FlowStateProof, run_flow_state};
pub use logic_multi_plan::{PreparedLogicMulti, prepare_logic_multi};
pub use logic_multi_play::{LogicMultiError, LogicMultiProof, run_logic_multi};
pub use logic_not_play::{LogicNotError, LogicNotProof, prepare_not, run_not};
pub use math_clamp_play::{MathClampError, MathClampProof, prepare_clamp, run_clamp};
pub use portable_state_input_plan::{PreparedPortableStateInput, prepare_portable_state_input};
pub use portable_state_input_play::{
    PortableStateInputError, PortableStateInputProof, run_portable_state_input,
};
pub use robotics_operation::RoboticsDriveEffect;
pub use robotics_plan::{PreparedRobotics, prepare_robotics};
pub use robotics_play::{RoboticsError, RoboticsProof, run_robotics};
pub use state_select_plan::{PreparedStateSelect, StateSelectSequence, prepare_state_select};
pub use state_select_play::{StateSelectError, StateSelectProof, run_state_select};
