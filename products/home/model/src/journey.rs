use alloc::vec::Vec;

use crate::{
    FORM_RUN_STEP_ID, FORM_SELECTED_STEP_ID, FORMS_OPENED_STEP_ID, HOME_ARRIVED_STEP_ID,
    HOME_RETURNED_STEP_ID, HomeAction, HomeModel, HomeView, JOURNEY_STEP_IDS,
    PATCHBAY_OPENED_STEP_ID, PLAY_OBSERVED_STEP_ID, PROMPT_OPENED_STEP_ID,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeJourneyRefusal {
    OutOfOrder,
    WrongTransition,
    AlreadyComplete,
}

/// Renderer-neutral observations of the exact accepted Home journey.
///
/// This tracker observes application transitions and confirmed effects. It
/// never turns a checkpoint into the transition or effect being observed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeJourney {
    observed: Vec<&'static str>,
    patchbay_requested: bool,
}

impl HomeJourney {
    pub fn arrived(model: &HomeModel) -> Result<Self, HomeJourneyRefusal> {
        if model.view() != HomeView::Launcher {
            return Err(HomeJourneyRefusal::WrongTransition);
        }
        Ok(Self {
            observed: alloc::vec![HOME_ARRIVED_STEP_ID],
            patchbay_requested: false,
        })
    }

    pub fn observed_step_ids(&self) -> &[&'static str] {
        &self.observed
    }

    pub fn is_complete(&self) -> bool {
        self.observed.as_slice() == JOURNEY_STEP_IDS
    }

    pub fn observe_action(
        &mut self,
        model: &HomeModel,
        action: &HomeAction,
    ) -> Result<(), HomeJourneyRefusal> {
        let next = self.next()?;
        match next {
            FORMS_OPENED_STEP_ID
                if model.view() == HomeView::Forms && matches!(action, HomeAction::Changed) =>
            {
                self.push(FORMS_OPENED_STEP_ID)
            }
            FORM_SELECTED_STEP_ID if matches!(action, HomeAction::OpenForm(_)) => {
                self.push(FORM_SELECTED_STEP_ID)
            }
            PROMPT_OPENED_STEP_ID
                if model.view() == HomeView::Prompt && matches!(action, HomeAction::Changed) =>
            {
                self.push(PROMPT_OPENED_STEP_ID)
            }
            FORM_RUN_STEP_ID if matches!(action, HomeAction::RunForm(_)) => {
                self.push(FORM_RUN_STEP_ID)
            }
            PATCHBAY_OPENED_STEP_ID if matches!(action, HomeAction::OpenPatchbay) => {
                self.patchbay_requested = true;
                Ok(())
            }
            HOME_RETURNED_STEP_ID
                if model.view() == HomeView::Launcher && matches!(action, HomeAction::Changed) =>
            {
                self.push(HOME_RETURNED_STEP_ID)
            }
            _ => Err(HomeJourneyRefusal::WrongTransition),
        }
    }

    pub fn observe_play_completed(&mut self) -> Result<(), HomeJourneyRefusal> {
        if self.next()? != PLAY_OBSERVED_STEP_ID {
            return Err(HomeJourneyRefusal::OutOfOrder);
        }
        self.push(PLAY_OBSERVED_STEP_ID)
    }

    pub fn observe_patchbay_opened(&mut self) -> Result<(), HomeJourneyRefusal> {
        if self.next()? != PATCHBAY_OPENED_STEP_ID || !self.patchbay_requested {
            return Err(HomeJourneyRefusal::OutOfOrder);
        }
        self.patchbay_requested = false;
        self.push(PATCHBAY_OPENED_STEP_ID)
    }

    fn next(&self) -> Result<&'static str, HomeJourneyRefusal> {
        JOURNEY_STEP_IDS
            .get(self.observed.len())
            .copied()
            .ok_or(HomeJourneyRefusal::AlreadyComplete)
    }

    fn push(&mut self, step: &'static str) -> Result<(), HomeJourneyRefusal> {
        if self.next()? != step {
            return Err(HomeJourneyRefusal::OutOfOrder);
        }
        self.observed.push(step);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMS: [&str; 1] = ["Hello"];

    #[test]
    fn deterministic_voice_commands_observe_the_exact_shared_journey() {
        let mut home = HomeModel::new();
        let mut journey = HomeJourney::arrived(&home).unwrap();

        let action = home.submit_text("forms", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        let action = home.submit_text("inspect hello", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        let action = home.submit_text("open prompt", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        let action = home.submit_text("run hello", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        journey.observe_play_completed().unwrap();
        let action = home.submit_text("open patchbay", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        journey.observe_patchbay_opened().unwrap();
        let action = home.submit_text("home", &FORMS);
        journey.observe_action(&home, &action).unwrap();

        assert!(journey.is_complete());
        assert_eq!(journey.observed_step_ids(), JOURNEY_STEP_IDS);
    }

    #[test]
    fn checkpoints_cannot_replace_effect_observation() {
        let mut home = HomeModel::new();
        let mut journey = HomeJourney::arrived(&home).unwrap();
        let action = home.submit_text("forms", &FORMS);
        journey.observe_action(&home, &action).unwrap();
        assert_eq!(
            journey.observe_play_completed(),
            Err(HomeJourneyRefusal::OutOfOrder)
        );
        assert_eq!(
            journey.observe_patchbay_opened(),
            Err(HomeJourneyRefusal::OutOfOrder)
        );
    }
}
