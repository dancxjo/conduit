//! Submit the original selected prepared workset on its retained carrier.
use super::*;
/// A transport report never proves admission or Start. Preserve owner approval
/// on every outcome, including a lost reply, until receiver retirement is proved.
#[cfg(target_os = "linux")]
pub(in crate::durable_host::source_revision) fn submit_selected_play(
    owner: &mut owner::Owner,
    selected: &UsbPlaySelection,
    carrier: &mut SelectedPlayCarrier,
    // Owned cold selection from the native provider. Neither the incoming
    // challenge nor the serialized File selection can supply these sessions.
    line_sessions: std::collections::BTreeMap<
        conduit_body::ResidentPlot,
        Vec<conduit_embedded_build::SelectedForeSession>,
    >,
) -> Result<(), String> {
    use conduit_std_host::pico_spawn::PreparationCommandBytes;
    let SelectedPlayCarrier { member, socket, .. } = carrier;
    let limits = conduit_embedded_build::BodyFragmentProjectionLimits {
        image: selected.image.bounds(),
        computations: conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: selected.maximum_program_bytes,
            maximum_total_program_bytes: selected.maximum_total_program_bytes,
        },
        assigned: conduit_core::AssignedPlanMaxima {
            encoded_bytes: selected.assigned_encoded_bytes,
            runtime_state_bytes: selected.assigned_runtime_state_bytes,
            counts: selected.assigned_counts,
        },
        maximum_fore_ports: selected.maximum_fore_ports,
        maximum_document_bytes: selected.maximum_document_bytes,
    };
    let mut approved = None;
    let mut refusal = None;
    let exchange = socket.exchange_preparation(
        std::time::Duration::from_millis(selected.wait_milliseconds),
        selected.maximum_document_bytes as usize,
        |challenge| {
            match owner.approve_selected_remote_workset_with_line_fore(
                member,
                challenge,
                limits,
                &line_sessions,
            ) {
                Ok(command) => {
                    approved = Some(command);
                }
                Err(error) => {
                    refusal = Some(error);
                    return Err(());
                }
            }
            let (payload, signature) = approved.as_ref().expect("just approved");
            Ok(PreparationCommandBytes { payload, signature })
        },
    );
    write_json_atomic(
        &selected.report_output,
        &serde_json::json!({
            "schema": "conduit.diagnostic/preparation-carriage@1",
            "member_host": member.host_advertisement.host_id,
            "member_boot": member.host_advertisement.boot_id,
            "reported": format!("{exchange:?}"),
            "owner_refusal": refusal,
            "admission_proved": false,
            "play_proved": false,
            "approval_retained": approved.is_some(),
        }),
    )?;
    exchange.map(|_| ()).map_err(|e| {
        format!("selected preparation carriage: {e:?}; original approval retained if issued")
    })
}
