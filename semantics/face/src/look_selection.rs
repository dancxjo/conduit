//! Look data admission, separate from provider selection and Show acknowledgement.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

/// Reports the caller's observed style support. Selected means the facet data
/// is available to that Mask; it does not claim output or acquire a provider.
pub fn look_facet_availability(
    look: &LookInfo,
    request: &LookFacetRequest,
    host_supported: bool,
) -> LookFacetAvailability {
    let present = match request {
        LookFacetRequest::Visual => matches!(look.visual(), LookVisualFacetOption::Some(_)),
        LookFacetRequest::Spoken => matches!(look.spoken(), LookSpokenFacetOption::Some(_)),
        LookFacetRequest::Generative => {
            matches!(look.generative(), LookGenerativeFacetOption::Some(_))
        }
        LookFacetRequest::Unsupported(_) => return LookFacetAvailability::Unsupported,
    };
    if !present {
        LookFacetAvailability::Absent
    } else if !host_supported {
        LookFacetAvailability::HostUnavailable
    } else {
        LookFacetAvailability::Selected
    }
}

#[derive(Debug)]
pub enum LookVisualAdmissionRefusal {
    Facet(LookFacetAvailability),
    Visual(VisualExpressionRefusal),
    Native(NativeBindingRefusal),
}

/// Both immutable resource values remain inspectable when the visual request
/// falls back. Other facets are not selected for another Mask by this value.
pub struct AdmittedVisualLook {
    pub requested: LookInfo,
    pub selected: LookInfo,
    pub visual: AdmittedVisualExpression,
}

/// Reuses existing accessibility and hard Host text-capacity admission. The
/// original Look is never mutated; actual rendering/acknowledgement stays owned
/// by the selected Mask, which also consumes the selected visual token resource.
pub fn admit_visual_look(
    look: &LookInfo,
    user: VisualUserConstraints,
) -> Result<AdmittedVisualLook, LookVisualAdmissionRefusal> {
    let LookVisualFacetOption::Some(facet) = look.visual() else {
        return Err(LookVisualAdmissionRefusal::Facet(
            LookFacetAvailability::Absent,
        ));
    };
    let expression = visual_expression_from_info(
        facet
            .expression()
            .clone()
            .into_structured()
            .map_err(LookVisualAdmissionRefusal::Native)?,
    )
    .map_err(LookVisualAdmissionRefusal::Visual)?;
    let visual = admit_visual_expression(Ok(expression), user)
        .map_err(LookVisualAdmissionRefusal::Visual)?;
    let selected = if matches!(
        visual.outcome,
        VisualExpressionOutcome::PhosphorFallback { .. }
    ) {
        phosphor_look().map_err(LookVisualAdmissionRefusal::Native)?
    } else {
        look.clone()
    };
    Ok(AdmittedVisualLook {
        requested: look.clone(),
        selected,
        visual,
    })
}
