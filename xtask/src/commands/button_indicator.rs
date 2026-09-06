//! Canonical Form, ordinary planner/kernel, scripted input, acquired physical output.
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ButtonIndicatorArgs {
    /// Exact CDC 0 path for the separately built indicator-resource firmware.
    #[arg(long)]
    serial_path: PathBuf,
    /// Expected CONDUIT_PICO_APPLIANCE_BUILD_ID from that exact firmware build.
    #[arg(long)]
    firmware_build_id: String,
    /// New evidence file; existing evidence is never overwritten.
    #[arg(long)]
    evidence_out: PathBuf,
    /// Authorize the bounded LED on/off effect. This does not authorize flashing.
    #[arg(long)]
    authorize_led: bool,
}

pub fn run(
    args: ButtonIndicatorArgs,
    opts: &crate::cli::GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if !args.authorize_led {
        return Err("--authorize-led is required".into());
    }
    if args.firmware_build_id.len() > 512
        || !args
            .firmware_build_id
            .contains(":indicator-resource:pico/indicator-resource-firmware@1")
    {
        return Err("expected exact indicator-resource firmware build identity".into());
    }
    if opts.dry_run {
        println!("Would acquire {:?}, plan the canonical button Form, and apply scripted press/release to the Pico LED; no flash", args.serial_path);
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        let mut evidence = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args.evidence_out)?;
        let result = physical::execute(&args);
        let record = match &result {
            Ok(record) => record.clone(),
            Err(error) => serde_json::json!({
                "schema": "conduit.button-indicator/device-output@1",
                "outcome": "failed", "error": error.to_string(),
                "physical_success_claimed": false,
            }),
        };
        serde_json::to_writer_pretty(&mut evidence, &record)?;
        writeln!(evidence)?;
        evidence.sync_all()?;
        result?;
        println!(
            "Device-output receipt: {} (scripted input; no human visibility claim)",
            args.evidence_out.display()
        );
        Ok(())
    }
    #[cfg(not(unix))]
    Err("Pico indicator acquisition requires a POSIX serial environment".into())
}

#[cfg(unix)]
mod physical {
    use super::*;
    use conduit_core::{resource_offer, HostAdvertisement, Plan, INPUT_RESOURCE_CLASS};
    use conduit_form::{
        check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
        StartupCatalog,
    };
    use conduit_std_host::{
        hosted_indicator::HostedIndicatorAdapter,
        hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
        pico_indicator::PicoIndicator,
        HostedRunAdapters, RunControl, StdHost, ThreadTimer,
    };
    use std::time::Duration;

    const SOURCE: &str = include_str!("../../../forms/button-across-room/main.conduit");

    struct ScriptedInput {
        next: u8,
    }
    impl HostedKeyboardAdapter for ScriptedInput {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            if self.next == 2 {
                return HostedKeyboardPoll::Cancelled;
            }
            // This timing makes the output inspectable; it is not timing proof.
            if self.next == 1 {
                std::thread::sleep(Duration::from_millis(500));
            }
            let event = conduit_human::KeyEvent::decode(&[0x2c, self.next, 0])
                .expect("fixed Space press/release");
            self.next += 1;
            HostedKeyboardPoll::Event(event)
        }
    }

    pub(super) fn execute(
        args: &ButtonIndicatorArgs,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut advertisement = StdHost::new().advertisement().clone();
        let digest = conduit_core::semantic_digest(
            "conduit.device/pico-indicator-build@1",
            args.firmware_build_id.as_bytes(),
        );
        let mut provider = PicoIndicator::acquire(
            &args.serial_path,
            &advertisement,
            digest,
            Duration::from_secs(3),
        )
        .map_err(|error| format!("Pico acquisition refused: {error:?}"))?;
        advertisement.capabilities = vec![
            conduit_std_offers::button::offer(),
            conduit_std_offers::button::mapper_offer(),
            conduit_std_offers::indicator_resource::offer(),
        ];
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        advertisement.resources.extend([
            resource_offer("demo/scripted-keyboard", INPUT_RESOURCE_CLASS, 1),
            resource_offer(
                provider.binding().pool_id.as_str(),
                conduit_std_offers::indicator_resource::RESOURCE_CLASS,
                1,
            ),
        ]);
        advertisement.resources.sort();
        let device = provider.device_association();
        conduit_core::validate_device_associations(&advertisement, std::slice::from_ref(&device))
            .map_err(|error| format!("acquired Device provenance refused: {error:?}"))?;
        let plan = plan(&advertisement)?;
        let mut host = StdHost::from_advertisement(advertisement)?;
        let mut input = ScriptedInput { next: 0 };
        let report = host.run_fragment_controlled_with_adapters_to(
            plan.fragments[0].clone(),
            &mut std::io::sink(),
            &mut ThreadTimer,
            &RunControl::default(),
            HostedRunAdapters {
                keyboard: Some(&mut input),
                indicator: Some(&mut provider),
            },
        )?;
        if provider.receipts().len() != 2
            || !provider.receipts()[0].state
            || provider.receipts()[1].state
            || !matches!(
                report.observations.last().map(|event| &event.kind),
                Some(conduit_core::ObservationKind::PlanTerminal {
                    disposition: conduit_core::TerminalDisposition::Completed
                })
            )
        {
            return Err(
                "canonical Play did not prove both acknowledged states and terminal completion"
                    .into(),
            );
        }
        let acknowledgments: Vec<_> = provider.receipts().iter().map(|receipt| serde_json::json!({
            "request": receipt.request, "state": receipt.state, "play_correlation": receipt.play_correlation,
        })).collect();
        Ok(serde_json::json!({
            "schema": "conduit.button-indicator/device-output@1",
            "proof_class": "native-kernel-pico-gpio-acknowledgment",
            "outcome": "completed", "source": SOURCE, "plan": plan,
            "input_proof_class": "scripted-keyboard-adapter",
            "device_path": args.serial_path, "device_identity_basis": "acquired-peripheral-asserted-boot-build",
            "firmware_build_id": args.firmware_build_id,
            "firmware_digest": provider.firmware_digest(), "device_boot": provider.device_boot(),
            "acquired_pool": provider.binding().pool_id,
            "device_association": device,
            "acknowledgments": acknowledgments, "observations": report.observations,
            "human_observed_led": false, "physical_input_claimed": false,
            "conduit_line_claimed": false,
        }))
    }

    fn plan(advertisement: &HostAdvertisement) -> Result<Plan, Box<dyn std::error::Error>> {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
        let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
            .map_err(|e| format!("{e:?}"))?;
        let form = expand_canonical_form(&checked, "button_across_room", &profile)
            .map_err(|e| format!("{e:?}"))?;
        let hosts = [advertisement.clone()];
        let choices = conduit_planner::default_expanded_placements(&form, &hosts)
            .map_err(|e| format!("{e:?}"))?;
        let limits = form
            .connections
            .iter()
            .map(|c| {
                (
                    (
                        c.source_gear_id.clone(),
                        c.source_port_id.clone(),
                        c.sink_gear_id.clone(),
                        c.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: if c.value_kind.as_str() == conduit_core::BOOL_INFO_ID {
                            1
                        } else {
                            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
                        },
                    },
                )
            })
            .collect();
        conduit_planner::plan_expanded_canonical_with_connection_limits(
            &form,
            &hosts,
            &choices,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &Default::default(),
                line_candidates: &Default::default(),
                connection_item_capacity: 1,
                connection_byte_capacity: 1,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits,
        )
        .map_err(|e| format!("{e:?}").into())
    }

    #[test]
    fn entrance_plan_requires_and_selects_the_acquired_resource() {
        let mut advertisement = StdHost::new().advertisement().clone();
        advertisement.capabilities = vec![
            conduit_std_offers::button::offer(),
            conduit_std_offers::button::mapper_offer(),
            conduit_std_offers::indicator_resource::offer(),
        ];
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        advertisement.resources.push(resource_offer(
            "proof/scripted-input",
            INPUT_RESOURCE_CLASS,
            1,
        ));
        advertisement.resources.sort();
        assert!(plan(&advertisement).is_err());
        advertisement.resources.push(resource_offer(
            "proof/acquired-indicator",
            conduit_std_offers::indicator_resource::RESOURCE_CLASS,
            1,
        ));
        advertisement.resources.sort();
        let plan = plan(&advertisement).unwrap();
        assert_eq!(plan.fragments.len(), 1);
        assert_eq!(plan.fragments[0].placements.len(), 3);
        let indicator = plan.fragments[0]
            .placements
            .iter()
            .find(|p| {
                p.implementation_id.as_str()
                    == conduit_std_offers::indicator_resource::IMPLEMENTATION
            })
            .unwrap();
        assert!(indicator
            .resources
            .iter()
            .any(|r| r.pool_id.as_str() == "proof/acquired-indicator"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_dry_run_has_no_device_or_evidence_effects() {
        let opts = crate::cli::GlobalOpts {
            dry_run: true,
            quiet: false,
            json: false,
            locked: true,
        };
        let args = ButtonIndicatorArgs {
            serial_path: "/missing/pico-device".into(),
            firmware_build_id: "conduit-pico-w-signal:proof:clean:thumbv6m-none-eabi:release:indicator-resource:pico/indicator-resource-firmware@1".into(),
            evidence_out: "/missing/evidence/receipt.json".into(),
            authorize_led: true,
        };
        run(args, &opts).unwrap();
    }
}
