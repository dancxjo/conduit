use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_form::parse_with_startup;
use conduit_signal::signal_profile_catalog;

fn form() -> conduit_form::CheckedForm {
    parse_with_startup(
        "form signal-demo {\n pulse: flow/pulse(count = 2, period-ms = 1, initial = false)\n show: presentation/show\n pulse > show\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .unwrap()
}

fn page(sink_boot: &str) -> BrowserSimPage {
    BrowserSimPage::with_hosts([
        BrowserSimConfig {
            host_id: HostId::from("browser/source"),
            boot_id: BootId::from("browser-boot/source"),
            offer_generation: OfferGeneration(1),
        },
        BrowserSimConfig {
            host_id: HostId::from("browser/durable-sink"),
            boot_id: BootId::from(sink_boot),
            offer_generation: OfferGeneration(1),
        },
    ])
}

#[test]
fn stale_plan_never_rebinds_and_explicit_replan_selects_fresh_boot() {
    let source = HostId::from("browser/source");
    let sink = HostId::from("browser/durable-sink");
    let old_page = page("browser-boot/old");
    let old_plan = old_page.plan_pair(&form(), &source, &sink).unwrap();
    let old_plan_id = old_plan.plan_id.clone();
    assert!(old_plan.fragments.iter().any(|fragment| {
        fragment.host_id == sink && fragment.boot_id == BootId::from("browser-boot/old")
    }));

    let mut fresh_page = page("browser-boot/fresh");
    let retained_old_plan = old_plan.clone();
    let refusal = fresh_page.run_plan(old_plan).unwrap_err();
    assert!(refusal.contains("prepare failed"));
    assert_eq!(retained_old_plan.plan_id, old_plan_id);
    assert!(retained_old_plan.fragments.iter().any(|fragment| {
        fragment.host_id == sink && fragment.boot_id == BootId::from("browser-boot/old")
    }));

    let fresh_plan = fresh_page.plan_pair(&form(), &source, &sink).unwrap();
    assert_ne!(fresh_plan.plan_id, old_plan_id);
    assert!(fresh_plan.fragments.iter().any(|fragment| {
        fragment.host_id == sink && fragment.boot_id == BootId::from("browser-boot/fresh")
    }));
    assert_eq!(fresh_page.run_plan(fresh_plan).unwrap().receipts.len(), 2);
}
