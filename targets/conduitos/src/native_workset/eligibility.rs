//! Cheap Crèche selection eligibility, separate from exact execution admission.
use super::{NativePlot, WorksetRefusal, profile};
use crate::offer::HostOffer;

/// Inventory and current input presence are sufficient to offer a selection.
/// This does not check Source, construct an advertisement, plan, lower, or
/// reserve resources. The draft obtains checked Source identities separately;
/// Wake plans the selected workset against the current offer and Plan admits
/// its kernel tables. Eligibility must never substitute for either boundary.
/// Recompute these finite facts from the current offer rather than retaining
/// an unbound successful planning result across Host/Boot/offer changes.
pub fn eligibility(plot: NativePlot, offer: &HostOffer<'_>) -> Result<(), WorksetRefusal> {
    if !profile().installed().contains(&plot) {
        return Err(WorksetRefusal::UnknownPlot);
    }
    // All installed candidates currently use the native input delivery profile.
    if offer.keyboard.is_none() {
        return Err(WorksetRefusal::Host);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_workset::{inventory, tests::fixture};

    #[test]
    fn installed_candidates_are_selectable_but_external_source_is_not() {
        let (_, offer) = fixture();
        for plot in inventory() {
            assert_eq!(eligibility(plot, &offer), Ok(()));
        }
        assert_eq!(
            eligibility(NativePlot::Hello, &offer),
            Err(WorksetRefusal::UnknownPlot)
        );
    }

    #[test]
    fn selection_eligibility_does_not_claim_resource_admission() {
        let (ids, mut offer) = fixture();
        for resource in &mut offer.resources {
            if resource.class == "conduit.resource/runtime-memory@1" {
                resource.capacity = 1;
            }
        }
        let plot = NativePlot::KeyboardCanvas;
        assert_eq!(eligibility(plot, &offer), Ok(()));
        assert!(crate::native_workset::review(plot, &ids, &offer, "build").is_err());
        assert!(
            crate::native_workset::prepare(
                &crate::native_workset::tests::wake(&[plot]),
                &ids,
                &offer,
                "build"
            )
            .is_err()
        );
    }

    #[test]
    fn current_input_absence_refuses_installed_candidates() {
        let (_, mut offer) = fixture();
        offer.keyboard = None;
        for plot in inventory() {
            assert_eq!(eligibility(plot, &offer), Err(WorksetRefusal::Host));
        }
        assert_eq!(
            eligibility(NativePlot::Hello, &offer),
            Err(WorksetRefusal::UnknownPlot)
        );
    }
}
