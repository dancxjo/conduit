//! Shared executable-tour runner extended across two exact browser fragments.

mod abi;
mod plan;
mod protocol;
mod session;

pub(super) struct ResidentProof {
    pub(super) manifestation: super::protocol::TourEffect,
    pub(super) source_fragment_id: String,
    pub(super) sink_fragment_id: String,
    pub(super) source_active_play_id: String,
    pub(super) sink_active_play_id: String,
    pub(super) line_id: String,
    pub(super) transferred_values: u32,
}

#[inline(never)]
pub(super) fn run_resident(source: &str) -> Result<ResidentProof, String> {
    use protocol::Output;
    use session::{Role, Session};

    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 1)?;
    let source_plan = plan::prepare(
        "browser/resident-tour-a",
        "boot/resident-tour-a",
        "browser/resident-tour-b",
        "boot/resident-tour-b",
        source,
    )?;
    let sink_plan = plan::accept(
        source_plan.plan.clone(),
        "browser/resident-tour-b",
        "boot/resident-tour-b",
    )?;
    let (mut sender, source_output) =
        Session::prepare(Role::Source, source_plan, 1, interaction.clone())?;
    let (mut receiver, sink_output) = Session::prepare(Role::Sink, sink_plan, 1, interaction)?;
    if !matches!(sink_output, Output::Waiting { .. }) {
        return Err("resident Tour second Host did not wait at its exact Line boundary".into());
    }
    let Output::Line {
        frame: value,
        plan_projection: Some(projection),
        receipt: None,
        ..
    } = source_output
    else {
        return Err("resident Tour source Host did not offer its exact Line value".into());
    };
    if projection.hosts.len() != 2
        || !projection.cord.crosses_host
        || projection.cord.maximum_in_flight_items != 1
        || value.line_id != projection.cord.line_id
    {
        return Err("resident Tour two-Host Plan lost its exact bounded Line".into());
    }
    let (manifestation, accepted) = match receiver.ingest(*value)? {
        Output::Manifestation {
            manifestation,
            accepted_frame,
            plan_projection,
            ..
        } if plan_projection.plan_id == projection.plan_id => (*manifestation, accepted_frame),
        _ => return Err("resident Tour sink Host did not manifest the planned value".into()),
    };
    if !matches!(sender.ingest(*accepted)?, Output::Waiting { .. }) {
        return Err("resident Tour source Host did not retain remote acceptance".into());
    }
    let delivered = match receiver.complete_manifestation()? {
        Output::Line { frame, .. } => frame,
        _ => return Err("resident Tour sink Host did not acknowledge presentation".into()),
    };
    let close = match sender.ingest(*delivered)? {
        Output::Line { frame, .. } => frame,
        _ => return Err("resident Tour source Host did not close its planned Cord".into()),
    };
    let (terminal, sink_receipt) = match receiver.ingest(*close)? {
        Output::Line {
            frame,
            receipt: Some(receipt),
            ..
        } => (frame, receipt),
        _ => return Err("resident Tour sink Host did not retain terminal truth".into()),
    };
    let source_receipt = match sender.ingest(*terminal)? {
        Output::Receipt { receipt, .. } => receipt,
        _ => return Err("resident Tour source Host did not retain terminal truth".into()),
    };
    if source_receipt.disposition != "completed"
        || sink_receipt.disposition != "completed"
        || source_receipt.transferred_values != 1
        || sink_receipt.transferred_values != 1
        || source_receipt.fragment_id == sink_receipt.fragment_id
        || source_receipt.active_play_id == sink_receipt.active_play_id
    {
        return Err("resident Tour two-Host receipts lost exact completion truth".into());
    }
    Ok(ResidentProof {
        manifestation,
        source_fragment_id: source_receipt.fragment_id.clone(),
        sink_fragment_id: sink_receipt.fragment_id.clone(),
        source_active_play_id: source_receipt.active_play_id.clone(),
        sink_active_play_id: sink_receipt.active_play_id.clone(),
        line_id: projection.cord.line_id.clone(),
        transferred_values: source_receipt.transferred_values,
    })
}

#[cfg(test)]
mod night_radio_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod button_tests;

#[cfg(test)]
fn submit_ascii_line(
    session: &mut session::Session,
    mut output: protocol::Output,
    text: &str,
) -> protocol::Output {
    for byte in text.bytes().chain(std::iter::once(b'\n')) {
        let (usage, modifiers) = match byte {
            b'a'..=b'z' => (byte - b'a' + 4, 0),
            b'A'..=b'Z' => (
                byte - b'A' + 4,
                conduit_human::KeyModifiers::LEFT_SHIFT.bits(),
            ),
            b' ' => (44, 0),
            b'\n' => (40, 0),
            _ => panic!("unsupported test input byte {byte}"),
        };
        for transition in [0, 1] {
            let protocol::Output::Input { input, .. } = output else {
                if transition == 1 && byte == b'\n' {
                    break;
                }
                panic!("standing keyboard did not request the next key event")
            };
            assert_eq!(input.effect_kind, "key-event");
            let play = input.active_play_id.clone();
            output = session
                .complete_input(
                    &play,
                    input.request_sequence,
                    &[usage, transition, modifiers],
                )
                .unwrap();
        }
    }
    output
}
