//! Source → selected reviewed offer → production std kernel, with sealed evidence.
use super::{host, installed_std, RecordingTimer};
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
#[test]
fn closed_numeric_graphs_execute_through_installed_std_kernel() {
    for (expression, representation) in [
        ("41 + 1 == 42", "checkedInteger"),
        ("0.1 + 0.2 == 0.3", "rational"),
        ("math/sin(90°) == 1", "symbolic"),
    ] {
        let source = format!(
            r#"plot numeric {{
 operation: math/evaluate(value = {expression}, requirement = {{
            semantics: mathematical(empty), representation: {representation}(empty), rounding: nearestEven(empty), accuracy: exact(empty),
            budget: {{ storageBytes:1048576, workSteps:100000, limbs:128, symbolicNodes:128 }}, parameters:[], reproducible:true,
            exceptions: {{invalid:false,divisionByZero:false,overflow:false,underflow:false,inexact:false}} }})
 show: presentation/bool-value
 operation.result >> show.value
 }}."#
        );
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_mathematical_kinds(&mut startup, &mut profile).unwrap();
        startup
            .insert(conduit_plot::KindSignature {
                kind: "presentation/bool-value".into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile
            .insert_kind(
                conduit_semantic_catalog::bool_value_presentation_contract()
                    .into_semantic_contract(
                        conduit_semantic_catalog::VALUE_PRIMITIVE_CONTRACT_REVISION,
                    ),
            )
            .unwrap();
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_plot(&checked, "numeric", &profile).unwrap();
        let mut advertisement = host("numeric-std-proof").advertisement().clone();
        advertisement
            .capabilities
            .extend(conduit_std_offers::numeric_evaluation_std_offers(&expanded.gears[0]).unwrap());
        advertisement
            .capabilities
            .push(conduit_std_offers::bool_value_presentation_offer());
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
        let plan =
            conduit_planner::plan_expanded_canonical(&expanded, &hosts, &placements, &[]).unwrap();
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement
                    .kind_id
                    .as_str()
                    .starts_with("conduitese/math/evaluate/")
            })
            .unwrap();
        assert_eq!(
            placement.artifact_id,
            conduit_core::numeric_realization::numeric_engine_artifact_id()
        );
        assert_eq!(
            conduit_semantic_catalog::prepare_numeric_evaluation(placement)
                .unwrap()
                .canonical(),
            &[1]
        );
        let mut timer = RecordingTimer { waits: Vec::new() };
        let mut signs = 0;
        let mut output = Vec::with_capacity(2048);
        let report = installed_std::run_fragment(
            installed_std::InstalledRunHost {
                advertisement: &advertisement,
                playback: None,
                midi_input: None,
                midi_output: None,
                keyboard: None,
                local_model: None,
                vector_search: None,
                model_work: None,
                calendar: None,
                body_conversation_context: None,
            },
            &plan.fragments[0],
            0,
            &mut signs,
            &mut output,
            &mut timer,
            &crate::RunControl::default(),
        )
        .unwrap();
        assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("bool value=true"));
    }
}
