//! History appearance intent is independent of the routed Plan and Body Face.
use super::*;
use conduit_face::*;

pub(in crate::product_front_door) fn history_available(
    prior: &crate::native_guest_face::NativeGuestFace,
) -> bool {
    prior.face().subjects.iter().any(|subject| {
        subject.identity == conduit_environment_history::HISTORY_SUBJECT
            && subject.role == FaceRole::Collection
    })
}

impl FaceArrival {
    pub(in crate::product_front_door) fn history_look_for(
        &self,
        prior: &crate::native_guest_face::NativeGuestFace,
    ) -> Result<LookInfo, &'static str> {
        if !history_available(prior) {
            return Err("native-owner-history-unavailable");
        }
        if let Some(look) = prior.history_look().map_err(|e| e.as_str())? {
            return Ok(look);
        }
        let context = &self.preparation;
        let host = crate::product_bases::native_mask_host_advertisement(
            &context.host_id,
            &context.boot_id,
            context.generation.0,
            &context.provider,
        );
        let (_, configuration) =
            conduit_conduitos_mask_offer::prepare_history_component_stage_from_offer(&host)
                .map_err(|_| "native-owner-history-default-unavailable")?;
        configuration
            .requested_look()
            .map_err(|_| "native-owner-history-default-invalid")
    }

    pub(in crate::product_front_door) fn select_history_key(
        &mut self,
        identities: BootIdentities,
        prior: &crate::native_guest_face::NativeGuestFace,
        route: &mut NativeOwnerReturnRoute,
        usage: u8,
        shift: bool,
        display: &mut impl PixelTarget,
    ) -> Result<crate::native_guest_face::NativeGuestFace, OwnerSelectionError> {
        let look = history_key_look(&self.history_look_for(prior)?, usage, shift)?;
        self.select_owner_history(identities, prior, route, &look, display)
    }
}

fn history_key_look(prior: &LookInfo, usage: u8, shift: bool) -> Result<LookInfo, &'static str> {
    if (usage, shift) == (67, false) {
        let unit = match prior.quantity() {
            LookQuantityFacetOption::Some(facet) => *facet.temperature_unit(),
            LookQuantityFacetOption::None => LookTemperatureUnit::Celsius,
        };
        let quantity = LookQuantityFacetOption::some(match unit {
            LookTemperatureUnit::Celsius => LookTemperatureUnit::Kelvin,
            LookTemperatureUnit::Kelvin => LookTemperatureUnit::Fahrenheit,
            LookTemperatureUnit::Fahrenheit => LookTemperatureUnit::Celsius,
        })
        .map_err(|_| "native-history-unit-invalid")?;
        return LookInfo::new(
            prior.generative().clone(),
            prior.identity().into(),
            quantity,
            prior.spoken().clone(),
            3,
            prior.visual().clone(),
        )
        .map_err(|_| "native-history-look-invalid");
    }
    let LookVisualFacetOption::Some(facet) = prior.visual() else {
        return Err("native-history-visual-absent");
    };
    let expression = facet.expression();
    let mut density = *expression.density();
    let mut palette = *expression.palette();
    let quantity = prior.quantity().clone();
    match (usage, shift) {
        (63, true) => {
            density = match density {
                VisualDensityPreference::Compact => VisualDensityPreference::Spacious,
                VisualDensityPreference::Spacious => VisualDensityPreference::Compact,
            }
        }
        (64, true) => {
            palette = match palette {
                VisualPalettePreference::Editorial => VisualPalettePreference::Phosphor,
                VisualPalettePreference::Phosphor => VisualPalettePreference::Editorial,
            }
        }
        (63, false) => density = VisualDensityPreference::Compact,
        (64, false) => density = VisualDensityPreference::Spacious,
        _ => return Err("native-history-key-unsupported"),
    }
    let palette_changed = palette != *expression.palette();
    let canonical = match palette {
        VisualPalettePreference::Editorial => editorial_look(),
        VisualPalettePreference::Phosphor => phosphor_look(),
    }
    .map_err(|_| "native-history-palette-invalid")?;
    let LookVisualFacetOption::Some(canonical_facet) = canonical.visual() else {
        return Err("native-history-palette-invalid");
    };
    let identity = if palette_changed {
        canonical.identity()
    } else {
        prior.identity()
    };
    let expression = VisualExpressionInfo::new(
        density,
        identity.into(),
        *expression.motion(),
        palette,
        *expression.text_scale_percent(),
        *expression.typography(),
        *expression.version(),
    )
    .map_err(|_| "native-history-expression-invalid")?;
    let tokens = if palette_changed {
        canonical_facet.tokens()
    } else {
        facet.tokens()
    };
    LookInfo::new(
        prior.generative().clone(),
        identity.into(),
        quantity,
        prior.spoken().clone(),
        3,
        LookVisualFacetOption::some(expression, tokens.clone())
            .map_err(|_| "native-history-visual-invalid")?,
    )
    .map_err(|_| "native-history-look-invalid")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantity_choice_survives_an_absent_visual_facet() {
        let base = editorial_look().unwrap();
        let prior = LookInfo::new(
            base.generative().clone(),
            base.identity().into(),
            LookQuantityFacetOption::None,
            base.spoken().clone(),
            3,
            LookVisualFacetOption::None,
        )
        .unwrap();
        let selected = history_key_look(&prior, 67, false).unwrap();
        assert!(
            matches!(selected.quantity(), LookQuantityFacetOption::Some(q)
            if *q.temperature_unit() == LookTemperatureUnit::Kelvin)
        );
        assert_eq!(selected.visual(), prior.visual());
        assert_eq!(selected.spoken(), prior.spoken());
        assert_eq!(selected.generative(), prior.generative());
    }

    #[test]
    fn units_density_and_palette_remain_independent() {
        let mut look = editorial_look().unwrap();
        for unit in [
            LookTemperatureUnit::Kelvin,
            LookTemperatureUnit::Fahrenheit,
            LookTemperatureUnit::Celsius,
        ] {
            look = history_key_look(&look, 67, false).unwrap();
            let quantity = look.quantity().clone();
            assert!(
                matches!(&quantity, LookQuantityFacetOption::Some(facet) if *facet.temperature_unit() == unit)
            );
            for (usage, shift) in [(63, true), (64, true), (63, false), (64, false)] {
                let next = history_key_look(&look, usage, shift).unwrap();
                assert_eq!(next.quantity(), &quantity);
                let (LookVisualFacetOption::Some(before), LookVisualFacetOption::Some(after)) =
                    (look.visual(), next.visual())
                else {
                    panic!("visual fixture");
                };
                if usage == 63 || !shift {
                    assert_eq!(before.expression().palette(), after.expression().palette());
                }
                if usage == 64 && shift {
                    assert_eq!(before.expression().density(), after.expression().density());
                }
                assert_eq!(next.identity(), after.expression().identity());
                look = next;
            }
        }
        assert!(history_key_look(&look, 65, false).is_err());
    }
}
