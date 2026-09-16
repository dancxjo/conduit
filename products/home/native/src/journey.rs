use conduit_home_model::{
    FORM_RUN_STEP_ID, FORM_SELECTED_STEP_ID, FORMS_OPENED_STEP_ID, HOME_ARRIVED_STEP_ID,
    HOME_RETURNED_STEP_ID, HomeDestination, HomeEvent, HomeView, PATCHBAY_OPENED_STEP_ID,
    PLAY_OBSERVED_STEP_ID, PROMPT_OPENED_STEP_ID,
};

use crate::{
    NativeHomeController, NativeHomeRequest, execute_installed_form, open_patchbay_presentation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHomeJourneyReceipt {
    pub schema: &'static str,
    pub host_face: &'static str,
    pub step_ids: Vec<&'static str>,
    pub final_revision: u32,
    pub form_plan_id: String,
    pub form_play_id: String,
    pub patchbay_presentation_id: String,
}

pub fn run_native_home_journey() -> Result<NativeHomeJourneyReceipt, String> {
    let mut home = NativeHomeController::new();
    let mut steps = vec![HOME_ARRIVED_STEP_ID];
    require(
        home.presentation()?.selected_destination() == Some(HomeDestination::Tour),
        "Home did not arrive on Tour",
    )?;

    home.submit_text("forms");
    steps.push(FORMS_OPENED_STEP_ID);
    require(home.model().view() == HomeView::Forms, "Forms did not open")?;
    home.accept(HomeEvent::Next);
    steps.push(FORM_SELECTED_STEP_ID);

    home.accept(HomeEvent::Escape);
    home.submit_text("open prompt");
    steps.push(PROMPT_OPENED_STEP_ID);
    let run = home.submit_text("run hello");
    require(
        run == Some(NativeHomeRequest::RunForm(0)),
        "Prompt did not preserve the exact Form request",
    )?;
    steps.push(FORM_RUN_STEP_ID);
    let execution = execute_installed_form(0)?;
    steps.push(PLAY_OBSERVED_STEP_ID);
    home.report_request(run.as_ref().expect("checked request"), Ok(()));

    home.submit_text("home");
    home.accept(HomeEvent::Next);
    let patchbay = home.accept(HomeEvent::Activate);
    require(
        patchbay == Some(NativeHomeRequest::OpenPatchbay),
        "Patchbay did not preserve the exact Host request",
    )?;
    let patchbay_opening = open_patchbay_presentation()?;
    steps.push(PATCHBAY_OPENED_STEP_ID);
    home.report_request(patchbay.as_ref().expect("checked request"), Ok(()));

    home.accept(HomeEvent::Escape);
    require(
        home.model().view() == HomeView::Launcher,
        "Home did not return to the launcher",
    )?;
    steps.push(HOME_RETURNED_STEP_ID);
    home.presentation()?;

    Ok(NativeHomeJourneyReceipt {
        schema: "conduit.evidence/native-home-journey@1",
        host_face: if cfg!(target_os = "windows") {
            "windows-native"
        } else if cfg!(target_os = "linux") {
            "linux-native"
        } else {
            "hosted-native"
        },
        step_ids: steps,
        final_revision: home.revision(),
        form_plan_id: execution.plan_id,
        form_play_id: execution.active_play_id,
        patchbay_presentation_id: patchbay_opening.presentation_id,
    })
}

fn require(condition: bool, failure: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| failure.to_owned())
}

impl From<crate::NativeHomeRefusal> for String {
    fn from(value: crate::NativeHomeRefusal) -> Self {
        format!("native Home presentation refused: {value:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_home_model::JOURNEY_STEP_IDS;

    #[test]
    fn native_face_enacts_the_shared_home_journey() {
        let receipt = run_native_home_journey().unwrap();
        assert_eq!(receipt.step_ids, JOURNEY_STEP_IDS);
        assert!(receipt.final_revision > 1);
        assert!(!receipt.host_face.is_empty());
        assert!(!receipt.form_plan_id.is_empty());
        assert!(!receipt.form_play_id.is_empty());
        assert!(!receipt.patchbay_presentation_id.is_empty());
    }
}
