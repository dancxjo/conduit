//! Actual checked Source/Host preparation across independent appearance axes.
use super::*;

#[test]
fn engineering_changes_only_look_for_each_creche_structure_and_density_palette() {
    let host = host();
    for recipe in [
        CrecheComponentRecipe::SingleColumn,
        CrecheComponentRecipe::ResponsiveColumns,
    ] {
        for density in [VisualDensity::Compact, VisualDensity::Spacious] {
            for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
                let selection = ComponentVisualSelection { density, palette };
                let (editorial, before) =
                    prepare_selected_creche_stage_from_offer(&host, recipe, selection).unwrap();
                let (engineering, after) = prepare_typographic_creche_stage_from_offer(
                    &host,
                    recipe,
                    selection,
                    VisualTypography::Engineering,
                )
                .unwrap();
                assert_ne!(
                    editorial.planned_mask.plan.plan_id,
                    engineering.planned_mask.plan.plan_id
                );
                assert_eq!(before.component, after.component);
                assert_eq!(before.recipe, after.recipe);
                assert_eq!(before.layout, after.layout);
                assert_eq!(before.implementation_id, after.implementation_id);
                let before = before.requested_look().unwrap();
                let after = after.requested_look().unwrap();
                let LookVisualFacetOption::Some(before) = before.visual() else {
                    panic!("visual facet")
                };
                let LookVisualFacetOption::Some(after) = after.visual() else {
                    panic!("visual facet")
                };
                assert_eq!(before.tokens(), after.tokens());
                let expression = visual_expression_from_info(
                    after.expression().clone().into_structured().unwrap(),
                )
                .unwrap();
                assert_eq!(expression.typography, VisualTypography::Engineering);
                assert_eq!((expression.density, expression.palette), (density, palette));
                assert_eq!(
                    expression.text_scale_percent,
                    if density == VisualDensity::Compact {
                        100
                    } else {
                        125
                    }
                );
            }
        }
    }
}

#[test]
fn navy_roots_supply_exact_complete_look_without_changing_structure() {
    let host = host();
    for recipe in [
        CrecheComponentRecipe::SingleColumn,
        CrecheComponentRecipe::ResponsiveColumns,
    ] {
        for density in [VisualDensity::Compact, VisualDensity::Spacious] {
            for typography in [VisualTypography::Editorial, VisualTypography::Engineering] {
                let selection = ComponentVisualSelection {
                    density,
                    palette: VisualPalette::Phosphor,
                };
                let (_, before) = prepare_typographic_creche_stage_from_offer(
                    &host, recipe, selection, typography,
                )
                .unwrap();
                let (_, after) =
                    prepare_navy_creche_stage_from_offer(&host, recipe, selection, typography)
                        .unwrap();
                assert_eq!(before.component, after.component);
                assert_eq!(before.recipe, after.recipe);
                assert_eq!(before.layout, after.layout);
                assert_eq!(before.implementation_id, after.implementation_id);
                assert_eq!(
                    after.requested_look().unwrap(),
                    navy_look(density, typography).unwrap()
                );
            }
        }
    }
}
