//! Add the optional physical Angle codec without replacing legacy alternatives.
use conduit_core::{CapabilityOffer, PortTemporal, StructuredInfoType, StructuredInfoTypeShape};
use conduit_plot::PortableExpressionProgram;

pub(super) fn offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<Option<CapabilityOffer>, String> {
    let angle = |ty: &StructuredInfoType| matches!(ty.shape(), StructuredInfoTypeShape::Leaf(kind) if (kind.as_str() == conduit_core::QUANTITY_INFO_ID || conduit_core::quantity_info_dimension(kind.as_str()).is_some() || conduit_core::parse_quantity_role_info_id(kind.as_str()).is_some()));
    if !angle(&program.input_type) && !angle(&program.output_type) {
        return Ok(None);
    }
    super::reference_validation::check(program)?;
    let Ok(mut offer) = conduit_std_offers::pure_mathematical_angle_std_offer(program, temporal)
    else {
        return Ok(None);
    };
    offer.capability_id = format!(
        "{}mathematical-angle/{}",
        super::OWNED,
        offer.kind_id.as_str()
    )
    .into();
    Ok(Some(offer))
}

pub(super) fn owns(offer: &CapabilityOffer) -> bool {
    offer.implementation.implementation_id.as_str()
        == conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
        && offer.implementation.execution_profile_id.as_str()
            == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
        && offer.implementation.artifact_id == conduit_std_offers::mathematical_angle_std_artifact()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StdHost;
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };

    #[test]
    fn mathematical_angle_batch_replaces_itself_and_preserves_native_alternatives() {
        let source = "plot arithmetic (\n >> value: Angle\n result: Angle >>\n) = (.)\n";
        let expand = |source: &str| {
            let checked =
                check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
                    .unwrap();
            expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new())
                .unwrap()
        };
        let mut host = StdHost::new();
        host.prepare_expression_realizations(&expand(source).expanded)
            .unwrap();
        let offers: Vec<_> = host
            .advertisement()
            .capabilities
            .iter()
            .filter(|o| super::super::owns(o))
            .collect();
        assert_eq!(offers.len(), 2);
        assert_eq!(offers.iter().filter(|o| owns(o)).count(), 1);
        assert!(offers
            .iter()
            .any(|o| o.implementation.execution_profile_id.as_str()
                == conduit_std_offers::PURE_EXPRESSION_STD_PROFILE));
        let original = host.advertisement().clone();
        host.prepare_expression_realizations(&expand(source).expanded)
            .unwrap();
        assert_eq!(host.advertisement(), &original);
        host.prepare_expression_realizations(&expand("plot arithmetic {\n}\n").expanded)
            .unwrap();
        assert!(!host
            .advertisement()
            .capabilities
            .iter()
            .any(super::super::owns));
    }
}
