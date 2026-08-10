use std::time::Duration;

use conduit_std_host::{StdHost, StdHostConfig, TimerAdapter};
use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
    ordinary_plan::{ORDINARY_FORM_SOURCE, prepare},
};

#[derive(Default)]
struct RecordingTimer(Vec<Duration>);

impl TimerAdapter for RecordingTimer {
    fn wait(&mut self, duration: Duration) {
        self.0.push(duration);
    }
}

#[test]
fn ordinary_semantics_match_the_materially_different_std_realization() {
    let mut catalog = conduit_std_catalog::tick_profile_catalog();
    catalog
        .insert(conduit_std_catalog::tick_presentation_kind_definition())
        .unwrap();
    let form = conduit_form::parse(ORDINARY_FORM_SOURCE, &catalog).unwrap();
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let bare_metal = prepare(&identities, &offer, "build").unwrap();

    let mut std_host = StdHost::new_with_config(StdHostConfig {
        host_id: conduit_core::HostId::from("materially-different-std-host"),
        boot_id: conduit_core::BootId::from("materially-different-std-boot"),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let std_plan = std_host.plan_local(&form, None).unwrap();
    assert_eq!(std_plan.checked_form_id, bare_metal.checked_form_id);
    assert_ne!(std_plan.plan_id, bare_metal.plan_id);
    assert!(
        std_plan.fragments[0]
            .placements
            .iter()
            .all(|placement| { placement.implementation_id.as_str().starts_with("std/") })
    );

    let mut output = Vec::new();
    let mut timer = RecordingTimer::default();
    let report = std_host
        .run_fragment_to(std_plan.fragments[0].clone(), &mut output, &mut timer)
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    assert_eq!(timer.0, vec![Duration::from_millis(1)]);
    assert!(output.contains("tick sequence=0"));
    let kernel = report
        .kernel
        .expect("ordinary std realization uses conduit-kernel");
    assert!(kernel.decisions > 0 && kernel.kernel_events > 0);
}
