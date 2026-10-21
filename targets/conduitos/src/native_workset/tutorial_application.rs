//! The same biography-driven resident Tutorial used by browser Workspace.
use super::WorksetRefusal;
use conduit_core::{ConfigurationValue, PlannedGear};
pub use conduitos_application_core::data::TutorialAction;

pub(super) fn selected_application(placement: &PlannedGear) -> Result<&str, WorksetRefusal> {
    let mut entries = placement
        .configuration
        .iter()
        .filter(|entry| entry.key == "application");
    let Some(entry) = entries.next() else {
        return Err(WorksetRefusal::Plan);
    };
    if entries.next().is_some() {
        return Err(WorksetRefusal::Plan);
    }
    match &entry.value {
        ConfigurationValue::Text(value)
            if matches!(value.as_str(), "tutorial" | "tour" | "patchbay") =>
        {
            Ok(value)
        }
        _ => Err(WorksetRefusal::Plan),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_application_is_exact_and_never_defaults_to_legacy_tour() {
        let (ids, offer) = crate::native_workset::tests::fixture();
        let wake = crate::native_workset::tests::wake(&[crate::native_workset::NativePlot::Tour]);
        let prepared = crate::native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
        assert!(matches!(
            crate::native_workset::NativeWorksetPlay::prepare(&prepared),
            Err(WorksetRefusal::Plan)
        ));
        let mut placement = prepared.plan.plots[0].plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == "application/retained")
            .unwrap()
            .clone();
        assert_eq!(selected_application(&placement), Ok("tutorial"));
        let entry = placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "application")
            .unwrap();
        entry.value = ConfigurationValue::Text("tour".into());
        assert_eq!(selected_application(&placement), Ok("tour"));
        placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "application")
            .unwrap()
            .value = ConfigurationValue::Text("unknown".into());
        assert_eq!(selected_application(&placement), Err(WorksetRefusal::Plan));
        placement.configuration.retain(|e| e.key != "application");
        assert_eq!(selected_application(&placement), Err(WorksetRefusal::Plan));
    }
}
