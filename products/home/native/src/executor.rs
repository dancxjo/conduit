use conduit_core::{ObservationKind, TerminalDisposition};

const HELLO_SOURCE: &str = include_str!("../../../../forms/hello/main.conduit");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFormExecution {
    pub form_index: usize,
    pub plan_id: String,
    pub active_play_id: String,
    pub output: Vec<u8>,
}

/// Run a finite installed Form through Conduit's ordinary hosted execution kernel.
///
/// The other installed examples are standing Forms. The native presenter does not
/// yet own a Stop channel or a background execution lifetime for them, so it must
/// refuse those requests instead of blocking the event loop or reporting success.
pub fn execute_installed_form(form_index: usize) -> Result<NativeFormExecution, String> {
    let source = match form_index {
        0 => HELLO_SOURCE,
        1..=3 => {
            return Err(
                "this standing Form needs native Stop and background-lifetime support".into(),
            );
        }
        _ => return Err("installed Form index is outside the native inventory".into()),
    };

    let mut output = Vec::new();
    let execution = conduit::execute_hosted_form(source, &mut output)?;
    let terminal = execution.observations.iter().find(|observation| {
        matches!(
            observation.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )
    });
    let terminal = terminal.ok_or("hosted Form did not complete its Plan")?;
    let active_play_id = terminal
        .active_play_id
        .as_ref()
        .ok_or("completed Plan did not retain its active Play identity")?;

    Ok(NativeFormExecution {
        form_index,
        plan_id: execution.plan.plan_id.as_str().to_owned(),
        active_play_id: active_play_id.as_str().to_owned(),
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_uses_the_real_hosted_form_pipeline() {
        let receipt = execute_installed_form(0).unwrap();
        assert_eq!(receipt.form_index, 0);
        assert!(!receipt.plan_id.is_empty());
        assert!(!receipt.active_play_id.is_empty());
        let output = String::from_utf8(receipt.output).unwrap();
        assert!(output.contains("PRESENTATION-TEXT bytes=13"));
        assert!(output.contains("\nHELLO, WORLD.\n"));
    }

    #[test]
    fn standing_forms_refuse_until_the_native_lifetime_is_admitted() {
        for index in 1..=3 {
            assert!(execute_installed_form(index).is_err());
        }
    }
}
