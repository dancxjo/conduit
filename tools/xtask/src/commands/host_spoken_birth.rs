use crate::cli::GlobalOpts;
use conduit_core::SignId;

pub(super) fn prove(
    confirm_birth: bool,
    bootstrap_text: &str,
    body_text: &str,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if !confirm_birth {
        return Err("spoken Birth requires the explicit --confirm-birth action".into());
    }
    if opts.dry_run {
        if opts.json {
            println!(
                "{}",
                serde_json::json!({
                    "schema": conduit_std_host::spoken_birth_journey::SpokenBirthJourneyEvidence::SCHEMA,
                    "speech_library": "conduit-tongues",
                    "dry_run": true,
                    "effects_performed": false,
                    "confirmation_action_id": "action/xtask-confirm-birth",
                })
            );
        } else if !opts.quiet {
            println!("would speak through Tongues as Host, perform Birth, then speak as Body");
        }
        return Ok(());
    }

    let awaiting = conduit_std_host::spoken_birth_journey::begin(bootstrap_text)?;
    let (_, evidence) = awaiting.confirm(
        conduit_std_host::spoken_birth_journey::ConfirmBirthAction::new(
            "action/xtask-confirm-birth",
        )?,
        "source/xtask-tongues-spoken-birth".into(),
        "checked/xtask-tongues-spoken-birth".into(),
        1,
        SignId::from("sign/xtask-tongues-spoken-birth/born"),
        body_text,
    )?;
    if opts.json {
        println!("{}", serde_json::to_string(&evidence)?);
    } else if !opts.quiet {
        println!(
            "TONGUES SPOKEN BIRTH PROVED: host-plan={} body={} body-plan={}",
            evidence.bootstrap.speech.plan_id,
            evidence.body_id.as_str(),
            evidence.body_manifestation.speech.plan_id,
        );
    }
    Ok(())
}
