//! Render the terminal Sign for this exact execution, never infer completion.
use crate::StdRunReport;
use conduit_core::{
    ActivePlayId, BootId, HostId, Observation, ObservationKind, PlanFragment, PlanId,
    TerminalDisposition,
};
use std::io::Write;

pub(super) fn write_terminal(
    output: &mut impl Write,
    fragment: &PlanFragment,
    report: &StdRunReport,
) -> Result<(), String> {
    let play = &report
        .kernel
        .as_ref()
        .ok_or_else(|| "execution report has no kernel Play identity".to_string())?
        .active_play_id;
    let disposition = terminal_for(
        &fragment.plan_id,
        &fragment.host_id,
        &fragment.boot_id,
        play,
        &report.observations,
    )?;
    match disposition {
        TerminalDisposition::Completed => {
            writeln!(output, "plan {} complete", fragment.plan_id.as_str())
        }
        TerminalDisposition::Cancelled { reason } => writeln!(
            output,
            "plan {} cancelled reason={reason:?}",
            fragment.plan_id.as_str()
        ),
        TerminalDisposition::Failed { reason } => writeln!(
            output,
            "plan {} failed reason={reason:?}",
            fragment.plan_id.as_str()
        ),
    }
    .map_err(|error| error.to_string())
}

fn terminal_for(
    plan: &PlanId,
    host: &HostId,
    boot: &BootId,
    play: &ActivePlayId,
    observations: &[Observation],
) -> Result<TerminalDisposition, String> {
    let mut terminals = observations.iter().filter_map(|observation| {
        if observation.plan_id.as_ref() != Some(plan)
            || observation.active_play_id.as_ref() != Some(play)
            || &observation.host_id != host
            || &observation.boot_id != boot
        {
            return None;
        }
        match observation.kind {
            ObservationKind::PlanTerminal { disposition } => Some(disposition),
            _ => None,
        }
    });
    let terminal = terminals.next().ok_or_else(|| {
        "execution report has no terminal Sign for its exact Plan/Play/Host/Boot".to_string()
    })?;
    if terminals.next().is_some() {
        return Err("execution report has multiple terminal Signs for one Play".into());
    }
    Ok(terminal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> Observation {
        Observation {
            sign_id: "terminal".into(),
            active_play_id: Some("play".into()),
            host_id: "host".into(),
            boot_id: "boot".into(),
            plan_id: Some("plan".into()),
            presentation_id: None,
            placement_id: None,
            connection_id: None,
            kind: ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed,
            },
        }
    }

    fn read(observations: &[Observation]) -> Result<TerminalDisposition, String> {
        terminal_for(
            &"plan".into(),
            &"host".into(),
            &"boot".into(),
            &"play".into(),
            observations,
        )
    }

    #[test]
    fn terminal_identity_cannot_be_borrowed_from_another_execution() {
        assert_eq!(
            read(&[observation()]).unwrap(),
            TerminalDisposition::Completed
        );
        let mutations: [fn(&mut Observation); 4] = [
            |o| o.plan_id = Some("other".into()),
            |o| o.active_play_id = Some("other".into()),
            |o| o.host_id = "other".into(),
            |o| o.boot_id = "other".into(),
        ];
        for mutate in mutations {
            let mut changed = observation();
            mutate(&mut changed);
            assert!(read(&[changed]).is_err());
        }
        assert!(read(&[]).is_err());
        assert!(read(&[observation(), observation()]).is_err());
    }
}
