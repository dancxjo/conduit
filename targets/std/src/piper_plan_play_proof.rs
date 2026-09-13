//! Explicit proof entrance for real Piper through ordinary std Plan and Play.

use crate::hosted_speech::{PiperDiscovery, PiperLimits};
use crate::{StdHost, StdHostComposition, StdHostConfig, StdRunReport, ThreadTimer};
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use std::collections::BTreeMap;

pub fn run(
    discovery: PiperDiscovery,
    limits: PiperLimits,
    text: &str,
) -> Result<StdRunReport, Box<dyn std::error::Error>> {
    let adapter = discovery.initialize(limits)?;
    let mut host = StdHost::new_with_piper_speech(
        StdHostConfig {
            host_id: HostId::from("std-piper-proof-host"),
            boot_id: BootId::from(crate::boot_identity::fresh_boot_id()),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )?;
    let mut profile = conduit_form::ProfileCatalog::new();
    let mut startup = conduit_form::StartupCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profile)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profile)?;
    crate::installed_std::test_speech_sink::install_catalog(&mut profile);
    let quoted_text = serde_json::to_string(text)?;
    let source = format!(
        "form piper_proof {{\n synthesize: speech/synthesize(maximum-output-bytes = {})\n sink: {}\n {} > synthesize.text\n synthesize.audio > sink.audio\n}}\n",
        conduit_tongues::MAXIMUM_PCM_BYTES,
        crate::installed_std::test_speech_sink::KIND,
        quoted_text,
    );
    let form = conduit_form::parse(&source, &profile)?;
    let advertisements = [host.advertisement().clone()];
    let placements = conduit_planner::default_placements(&form, &advertisements)?;
    let plan = conduit_planner::plan_with_options(
        &form,
        &advertisements,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_offers::PIPER_PCM_BLOCK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .ok_or("Piper proof plan has no local fragment")?;
    let report =
        host.run_fragment_to(fragment, &mut Vec::with_capacity(1_024), &mut ThreadTimer)?;
    if report.speech_synthesis.len() != 1 {
        return Err("Piper Plan/Play proof produced no exact synthesis receipt".into());
    }
    Ok(report)
}
