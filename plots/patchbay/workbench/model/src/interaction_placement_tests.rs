use super::{count_graph, interaction};
use crate::{
    InteractionDisposition, PatchbayEdit, PatchbayEditBasis, PatchbayInteractionRequest,
    PatchbayInvocationOutcome,
};

#[test]
fn placement_names_survive_checked_plot_plan_kernel_and_binary_transport() {
    for (kind, label) in [
        ("face/quantity", Some("Room temperature")),
        ("text/upper", None),
    ] {
        let graph = count_graph();
        let edit = PatchbayEdit::PlaceGear {
            basis: PatchbayEditBasis::new(
                graph.source_document_id.clone(),
                7,
                graph.expanded_plot_id.clone(),
            )
            .unwrap(),
            kind_id: kind.into(),
            data_label: label.map(str::to_owned),
        };
        let mut interaction = interaction();
        let request = PatchbayInteractionRequest::edit(
            interaction.next_request_id("place-gear").unwrap(),
            edit,
        )
        .unwrap();
        let receipt = interaction
            .execute(Some(&graph), request.clone(), |planned| {
                assert_eq!(planned, &request);
                PatchbayInvocationOutcome::Succeeded
            })
            .unwrap();
        assert_eq!(receipt.disposition, InteractionDisposition::Succeeded);
        assert_eq!(receipt.request, request);
    }
}
